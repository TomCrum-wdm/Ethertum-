use crate::{
    client::prelude::*,
    server::{dedicated_server::rcon::Motd, prelude::ServerSettings},
    util,
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use bevy_egui::{
    egui::{self, Color32, Layout},
    EguiContexts,
};
use bevy_renet::renet::RenetClient;

use super::{sfx_play, ui_lr_panel, CurrentUI, UiExtra};

use super::new_egui_window;

fn format_age_secs(ts: i64) -> String {
    let now = crate::util::current_timestamp().as_secs() as i64;
    let dt = (now - ts).max(0);
    if dt < 60 {
        format!("{}s ago", dt)
    } else if dt < 3600 {
        format!("{}m ago", dt / 60)
    } else if dt < 86400 {
        format!("{}h ago", dt / 3600)
    } else {
        format!("{}d ago", dt / 86400)
    }
}

pub fn ui_connecting_server(mut ctx: EguiContexts, mut cli: EthertiaClient, net_client: Option<ResMut<RenetClient>>) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    new_egui_window("Server List").show(ctx_mut, |ui| {
    // Local state for export background task and result
        let h = ui.available_height();

        ui.vertical_centered(|ui| {
            ui.add_space(h * 0.2);

            if net_client.is_some_and(|e| e.is_connected()) {
                ui.label("Authenticating & Logging in...");
            } else {
                ui.label("Connecting to the server...");
            }
            ui.add_space(38.);
            ui.spinner();

            ui.add_space(h * 0.3);

            if ui.btn_normal("Cancel").clicked() {
                cli.exit_world();
            }
        });
    });
}

pub fn ui_disconnected_reason(
    mut ctx: EguiContexts,
    mut cli: ResMut<ClientInfo>, // readonly. mut only for curr_ui.
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    new_egui_window("Disconnected Reason").show(ctx_mut, |ui| {
        let h = ui.available_height();

        ui.vertical_centered(|ui| {
            ui.add_space(h * 0.2);

            ui.label("Disconnected:");
            ui.colored_label(Color32::WHITE, cli.disconnected_reason.as_str());

            ui.add_space(h * 0.3);

            if ui.btn_normal("Back to title").clicked() {
                cli.curr_ui = CurrentUI::MainMenu;
            }
        });
    });
}

#[derive(Default, Debug)]
pub struct UiServerInfo {
    pub motd: String,
    pub num_players_online: u32,
    pub num_players_limit: u32,
    pub ping: u32,
    pub gameplay_addr: String,

    pub is_editing: bool,
    pub refreshing_task: Option<(Task<anyhow::Result<Motd>>, u64)>,
}

pub fn ui_serverlist(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
    // mut refreshing_indices: Local<HashMap<usize, (Task<anyhow::Result<Motd>>, u64)>>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    new_egui_window("Server List").show(ctx_mut, |ui| {
        let serverlist = &mut cli.cfg.serverlist;

        // all access defer to one closure.
        let do_new_server = std::cell::Cell::new(false);
        let do_refresh_all = std::cell::Cell::new(false);
        let do_stop_refreshing = std::cell::Cell::new(false);
        let mut do_acquire_list = false;
        let mut do_join_addr = None;
        let mut do_del_idx = None;

        let show_btn_stop_refresh = serverlist.iter().any(|e| e.ui.refreshing_task.is_some());
        ui_lr_panel(
            ui,
            true,
            |ui| {
                if ui.btn_borderless("Add Server").clicked() {
                    do_new_server.set(true);
                }
                if ui.btn_borderless("Refresh All").clicked() {
                    do_refresh_all.set(true);
                }
                if show_btn_stop_refresh && ui.btn_borderless("Stop Refresh").clicked() {
                    do_stop_refreshing.set(true);
                }
                ui.separator();
                if ui.btn_borderless("Aquire List").on_hover_text("Get Official Server List").clicked() {
                    do_acquire_list = true;
                }
                if ui.btn_borderless("Direct Connect").clicked() {}
            },
            |ui| {
                for (idx, server_item) in serverlist.iter_mut().enumerate() {
                    let ui_server_info = &mut server_item.ui;

                    let is_editing = ui_server_info.is_editing;
                    let is_accessable = ui_server_info.ping != 0;
                    let mut is_refreshing = ui_server_info.refreshing_task.is_some() || do_refresh_all.get();

                    ui.group(|ui| {
                        // First Line
                        ui.horizontal(|ui| {
                            if is_editing {
                                ui.text_edit_singleline(&mut server_item.name);
                            } else {
                                // Left: Name
                                ui.colored_label(Color32::WHITE, server_item.name.clone())
                                    .on_hover_text(server_item.addr.clone());
                                ui.small(&server_item.addr);

                                // Right: Status
                                if is_accessable {
                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        ui.label(format!(
                                            "{}ms · {}/{}",
                                            ui_server_info.ping, ui_server_info.num_players_online, ui_server_info.num_players_limit
                                        ));
                                    });
                                }
                            }
                        });
                        // Line2
                        ui.horizontal(|ui| {
                            // Left: Description/Motd
                            if is_editing {
                                ui.text_edit_singleline(&mut server_item.addr);
                            } else if is_refreshing {
                                ui.spinner();
                            } else if is_accessable {
                                ui.label(&ui_server_info.motd);
                            } else {
                                ui.colored_label(Color32::DARK_RED, "Inaccessible 🚫").on_hover_text(&ui_server_info.motd);
                            }

                            // Right: Ops
                            ui.with_layout(Layout::right_to_left(egui::Align::Max), |ui| {
                                if is_editing {
                                    if ui.btn("✅").clicked() {
                                        ui_server_info.is_editing = false;
                                    }
                                } else {
                                    if ui.btn("🗑").on_hover_text("Delete").clicked() {
                                        do_del_idx = Some(idx);
                                    }
                                    if ui.btn("⛭").on_hover_text("Edit").clicked() {
                                        ui_server_info.is_editing = true;
                                    }
                                    if is_refreshing {
                                        if ui.btn("❌").on_hover_text("Stop Refreshing").clicked() {
                                            is_refreshing = false;
                                        }
                                    } else if ui.btn("⟲").on_hover_text("Refresh Server Status").clicked() {
                                        is_refreshing = true;
                                    }
                                    if ui.btn("▶").on_hover_text("Join & Play").clicked() {
                                        do_join_addr = Some(if ui_server_info.gameplay_addr.is_empty() {
                                            server_item.addr.clone()
                                        } else if ui_server_info.gameplay_addr.starts_with(":") {
                                            // Concat: same ip but different port.
                                            let host = server_item
                                                .addr
                                                .find(':')
                                                .and_then(|i| server_item.addr.get(0..i))
                                                .unwrap_or(&server_item.addr);
                                            format!("{}{}", host, ui_server_info.gameplay_addr)
                                        } else {
                                            ui_server_info.gameplay_addr.clone()
                                        });
                                    }
                                }
                            });
                        });
                    });

                    // ServerStatus Process
                    if is_refreshing {
                        let addr = server_item.addr.clone(); // opt
                        let (task, time) = ui_server_info.refreshing_task.get_or_insert_with(|| {
                            (
                                AsyncComputeTaskPool::get().spawn(async move { util::http_get_json::<Motd>(&format!("http://{}", addr)) }),
                                util::current_timestamp_millis(),
                            )
                        });
                        if let Some(polled) = futures_lite::future::block_on(futures_lite::future::poll_once(task)) {
                            match polled {
                                Ok(r) => {
                                    ui_server_info.motd = r.motd;
                                    ui_server_info.num_players_limit = r.num_player_limit;
                                    ui_server_info.num_players_online = r.num_player_online;
                                    ui_server_info.gameplay_addr = r.game_addr;
                                    ui_server_info.ping = (util::current_timestamp_millis() - *time) as u32;
                                }
                                Err(err) => {
                                    info!("Failed to access server status: {}", err);
                                    ui_server_info.ping = 0;
                                    ui_server_info.motd = err.to_string();
                                }
                            }
                            is_refreshing = false;
                        }
                    }
                    if do_stop_refreshing.get() || !is_refreshing {
                        ui_server_info.refreshing_task = None;
                    }
                }

                if do_new_server.get() {
                    serverlist.push(ServerListItem {
                        name: "Server Name".into(),
                        addr: "0.0.0.0:4000".into(),
                        ..default()
                    });
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            },
        );

        if do_acquire_list {
            match crate::util::http_get_json("https://ethertia.com/server-info.json") {
                Ok(ret) => *serverlist = ret,
                Err(err) => info!("{}", err),
            }
        }

        if let Some(idx) = do_del_idx {
            serverlist.remove(idx);
        }

        if let Some(addr) = do_join_addr {
            cli.connect_server(addr);
        }
    });
}

pub fn ui_localsaves(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
    serv_cfg: Option<Res<ServerSettings>>,
    mut worlds: Local<Vec<crate::voxel::LocalWorldInfo>>,
    mut last_error: Local<String>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    match crate::voxel::list_worlds() {
        Ok(list) => {
            *worlds = list;
            last_error.clear();
        }
        Err(err) => {
            if worlds.is_empty() {
                *last_error = err.to_string();
            }
        }
    }

    new_egui_window("Local Worlds").show(ctx_mut, |ui| {
        let local_world_supported = serv_cfg.is_some();
        let mut do_refresh = false;
        let mut do_delete: Option<String> = None;
        let mut do_play: Option<(String, u64)> = None;

        if !local_world_supported {
            ui.colored_label(Color32::YELLOW, "Local worlds are unavailable on this platform/runtime.");
            ui.small("Integrated server is not active. Use Multiplayer to connect to a remote server.");
            ui.add_space(8.0);
        }

<<<<<<< HEAD
        // Gather saves from disk — do this in background to avoid blocking UI/startup.
        let saves_root = crate::util::saves_root();
        // If we have a cached result, use it. Otherwise ensure a background task is running.
        if saves_cache.is_none() {
            if saves_loading_task.is_none() {
                let root = saves_root.clone();
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    let mut list: Vec<(String, Option<serde_json::Value>)> = Vec::new();
                    if let Ok(entries) = std::fs::read_dir(&root) {
                        for e in entries.flatten() {
                            let p = e.path();
                            if p.is_dir() {
                                let folder = e.file_name().to_string_lossy().into_owned();
                                let meta_path = p.join("meta.json");
                                let meta = meta_path
                                    .exists()
                                    .then(|| std::fs::read_to_string(&meta_path).ok())
                                    .flatten()
                                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
                                list.push((folder, meta));
                            }
                        }
                    }
                    list
                });
                *saves_loading_task = Some(task);
            }
        }

        // If loading task finished, collect result into cache
        if let Some(task) = saves_loading_task.as_mut() {
            if let Some(polled) = futures_lite::future::block_on(futures_lite::future::poll_once(task)) {
                *saves_cache = Some(polled);
                *saves_loading_task = None;
            }
        }

        let mut saves: Vec<(String, Option<serde_json::Value>)> = saves_cache.clone().unwrap_or_default();

        // Prepare deferred actions to avoid multiple mutable borrows of `cli` inside closures
        let create_requested = std::cell::Cell::new(false);
        let mut enter_request: Option<(String, Option<u64>)> = None;
        let mut export_request: Option<(String, bool)> = None; // (save_name, include_cache)

=======
        if !last_error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", *last_error));
            ui.add_space(6.0);
        }

>>>>>>> feature/world-persistence-8073199
        ui_lr_panel(
            ui,
            false,
            |ui| {
                if ui.btn_borderless("New World").clicked() {
<<<<<<< HEAD
                    create_requested.set(true);
=======
                    cli.data().curr_ui = CurrentUI::LocalWorldNew;
                }
                if ui.btn_borderless("Refresh").clicked() {
                    do_refresh = true;
                }
                if ui.btn_borderless("Back").clicked() {
                    cli.data().curr_ui = CurrentUI::MainMenu;
>>>>>>> feature/world-persistence-8073199
                }
            },
            |ui| {
<<<<<<< HEAD
                // two-column layout: left = saves list, right = generation console
                ui.columns(2, |cols| {
                            // Left: saves list
                    cols[0].vertical(|ui| {
                                // Prominent Create World button
                                ui.centered_and_justified(|ui| {
                                    ui.add_space(6.0);
                                    if sfx_play(ui.add_sized([320., 52.], egui::Button::new("Create World").fill(Color32::DARK_GREEN))).clicked() {
                                        create_requested.set(true);
                                    }
                                    ui.add_space(6.0);
                                });

                                ui.separator();
                        if saves.is_empty() {
                            ui.label("No local saves found.");
                            ui.small("Create a world using the generation console on the right.");
                            return;
                        }

                        for (idx, (folder, meta)) in saves.iter().enumerate() {
                            let is_editing = idx_editing.is_some_and(|i| i == idx);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    let display_name = meta
                                        .as_ref()
                                        .and_then(|m| m.get("name"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| folder.clone());
                                    ui.colored_label(Color32::WHITE, display_name.clone()).on_hover_text(folder.clone());
                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        if let Some(m) = meta.as_ref() {
                                            if let Some(seed_v) = m.get("seed").and_then(|v| v.as_u64()) {
                                                ui.label(format!("seed: {:016x}", seed_v));
                                            }
                                        }
                                    });
                                });
        ui_lr_panel(
            ui,
            false,
            |ui| {
                if ui.btn_borderless("New World").clicked() {
                    cli.data().curr_ui = CurrentUI::LocalWorldNew;
                }
                if ui.btn_borderless("Refresh").clicked() {
                    do_refresh = true;
                }
                if ui.btn_borderless("Back").clicked() {
                    cli.data().curr_ui = CurrentUI::MainMenu;
                }
            },
            |ui| {
                if worlds.is_empty() {
                    ui.label("No local worlds yet. Click New World to create one.");
                }

                for world in worlds.iter() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::WHITE, &world.name);
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                ui.label(format!("{} · seed {}", format_age_secs(world.last_played), world.seed));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label("Persistent local world");
                            ui.with_layout(Layout::right_to_left(egui::Align::Max), |ui| {
                                if local_world_supported {
                                    if ui.btn("🗑").on_hover_text("Delete world").clicked() {
                                        do_delete = Some(world.name.clone());
                                    }
                                    if ui.btn("▶").on_hover_text("Play world").clicked() {
                                        do_play = Some((world.name.clone(), world.seed));
                                    }
                                } else {
                                    if ui.btn("🗑").on_hover_text("Delete world").clicked() {
                                        do_delete = Some(world.name.clone());
                                    }
                                    ui.add_enabled(false, egui::Button::new("▶"))
                                        .on_hover_text("Play is unavailable on this runtime");
                                }
                            });
                        });
                    });
                }
            },
        );

        if do_refresh {
            match crate::voxel::list_worlds() {
                Ok(list) => {
                    *worlds = list;
                    last_error.clear();
                }
                Err(err) => {
                    worlds.clear();
                    *last_error = err.to_string();
                }
            }
        }

        if let Some(name) = do_delete {
            match crate::voxel::delete_world(&name) {
                Ok(()) => {
                    match crate::voxel::list_worlds() {
                        Ok(list) => {
                            *worlds = list;
                            last_error.clear();
                        }
                        Err(err) => {
                            worlds.clear();
                            *last_error = err.to_string();
                        }
                    }
                }
                Err(err) => *last_error = err.to_string(),
            }
        }

        if let Some((name, seed)) = do_play {
            if let Some(serv_cfg) = &serv_cfg {
                cli.connect_local_world(name, seed, serv_cfg.port);
            } else {
                *last_error = "Integrated server unavailable on this runtime".to_string();
            }
        }
    });
                                } else {
                                    if ui.btn("🗑").on_hover_text("Delete world").clicked() {
                                        do_delete = Some(world.name.clone());
>>>>>>> feature/world-persistence-8073199
                                    }
                                    ui.add_enabled(false, egui::Button::new("▶"))
                                        .on_hover_text("Play is unavailable on this runtime");
                                }
                                enter_request = Some((name, seed_val));
                            }
                        });
                    });
                });
            },
        );

<<<<<<< HEAD
        // Execute deferred actions now with exclusive mutable access to `cli`.
        if create_requested.get() {
            cli.data().curr_ui = CurrentUI::LocalWorldNew;
        }
        if let Some((name, seed)) = enter_request {
            cli.enter_world_with_save(Some(name), seed);
        }
        if let Some((save_name, include_cache)) = export_request {
            match crate::voxel::chunk_storage::export_save_as_zip(&save_name, include_cache) {
                Ok(path) => info!("Exported save {} -> {:?}", save_name, path),
                Err(err) => warn!("Failed to export save {}: {}", save_name, err),
=======
        if do_refresh {
            match crate::voxel::list_worlds() {
                Ok(list) => {
                    *worlds = list;
                    last_error.clear();
                }
                Err(err) => {
                    worlds.clear();
                    *last_error = err.to_string();
                }
            }
        }

        if let Some(name) = do_delete {
            match crate::voxel::delete_world(&name) {
                Ok(()) => {
                    match crate::voxel::list_worlds() {
                        Ok(list) => {
                            *worlds = list;
                            last_error.clear();
                        }
                        Err(err) => {
                            worlds.clear();
                            *last_error = err.to_string();
                        }
                    }
                }
                Err(err) => *last_error = err.to_string(),
            }
        }

        if let Some((name, seed)) = do_play {
            if let Some(serv_cfg) = &serv_cfg {
                cli.connect_local_world(name, seed, serv_cfg.port);
            } else {
                *last_error = "Integrated server unavailable on this runtime".to_string();
>>>>>>> feature/world-persistence-8073199
            }
        }
    });
}

#[derive(Default, Debug, PartialEq)]
pub enum Difficulty {
    Peace,
    #[default]
    Normal,
    Hard,
}

pub fn ui_create_world(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
<<<<<<< HEAD
    mut tx_world_name: Local<String>,
    mut tx_world_seed: Local<String>,
    mut _difficulty: Local<Difficulty>,
    mut tx_terrain_is_planet: Local<bool>,
    mut tx_planet_x: Local<String>,
    mut tx_planet_y: Local<String>,
    mut tx_planet_z: Local<String>,
    mut tx_planet_radius: Local<String>,
    mut tx_planet_thickness: Local<String>,
    mut tx_gravity: Local<String>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else { return };
=======
    serv_cfg: Option<Res<ServerSettings>>,
    mut tx_world_name: Local<String>,
    mut tx_world_seed: Local<String>,
    mut _difficulty: Local<Difficulty>,
    mut create_error: Local<String>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };
    let local_play_supported = serv_cfg.is_some();
>>>>>>> feature/world-persistence-8073199

    new_egui_window("New World").show(ctx_mut, |ui| {
        let space = 14.;

        ui.label("Name:");
        sfx_play(ui.text_edit_singleline(&mut *tx_world_name));
        ui.add_space(space);

        ui.label("Seed:");
        sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
        ui.add_space(space);

        ui.label("Gamemode:");
        ui.horizontal(|ui| {
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Peace, "Survival"));
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Normal, "Creative"));
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Hard, "Spectator"));
        });
        ui.add_space(space);

        ui.label("Difficulty:");
        ui.horizontal(|ui| {
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Peace, "Peace"));
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Normal, "Normal"));
            sfx_play(ui.radio_value(&mut *_difficulty, Difficulty::Hard, "Hard"));
        });
        ui.add_space(space);

        egui::ComboBox::from_id_source("Difficulty")
            .selected_text(format!("{:?}", *_difficulty))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut *_difficulty, Difficulty::Peace, "Peace");
                ui.selectable_value(&mut *_difficulty, Difficulty::Normal, "Normal");
                ui.selectable_value(&mut *_difficulty, Difficulty::Hard, "Hard");
            });

        ui.add_space(space);

        ui.separator();
        ui.heading("Generator Parameters");
        ui.label("Terrain Mode:");
        egui::ComboBox::from_id_source("terrain_mode")
            .selected_text(if *tx_terrain_is_planet { "Planet" } else { "Flat" })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut *tx_terrain_is_planet, true, "Planet");
                ui.selectable_value(&mut *tx_terrain_is_planet, false, "Flat");
            });

        ui.horizontal(|ui| {
            ui.label("Planet Center X:");
            ui.add(egui::TextEdit::singleline(&mut *tx_planet_x));
        });
        ui.horizontal(|ui| {
            ui.label("Planet Center Y:");
            ui.add(egui::TextEdit::singleline(&mut *tx_planet_y));
        });
        ui.horizontal(|ui| {
            ui.label("Planet Center Z:");
            ui.add(egui::TextEdit::singleline(&mut *tx_planet_z));
        });

        ui.horizontal(|ui| {
            ui.label("Planet Radius:");
            ui.add(egui::TextEdit::singleline(&mut *tx_planet_radius));
        });
        ui.horizontal(|ui| {
            ui.label("Shell Thickness:");
            ui.add(egui::TextEdit::singleline(&mut *tx_planet_thickness));
        });
        ui.horizontal(|ui| {
            ui.label("Gravity:");
            ui.add(egui::TextEdit::singleline(&mut *tx_gravity));
        });

        ui.add_space(22.);

<<<<<<< HEAD
        if sfx_play(ui.add_sized([290., 26.], egui::Button::new("Create World").fill(Color32::DARK_GREEN))).clicked() {
            // Apply settings to client config and enter world
            let seed_val = tx_world_seed.parse::<u64>().ok().unwrap_or_else(|| util::current_timestamp_millis() as u64);
            let mut name = tx_world_name.clone();
            if name.trim().is_empty() {
                name = format!("world_{:016x}", seed_val);
            }

            // Parse generator params
            let px = tx_planet_x.parse::<f32>().unwrap_or(cli.cfg.planet_center[0]);
            let py = tx_planet_y.parse::<f32>().unwrap_or(cli.cfg.planet_center[1]);
            let pz = tx_planet_z.parse::<f32>().unwrap_or(cli.cfg.planet_center[2]);
            let pr = tx_planet_radius.parse::<f32>().unwrap_or(cli.cfg.planet_radius);
            let pt = tx_planet_thickness.parse::<f32>().unwrap_or(cli.cfg.planet_shell_thickness);
            let g = tx_gravity.parse::<f32>().unwrap_or(cli.cfg.gravity_accel);

            cli.cfg.terrain_mode = if *tx_terrain_is_planet { crate::client::settings::TerrainMode::Planet } else { crate::client::settings::TerrainMode::Flat };
            cli.cfg.planet_center = [px, py, pz];
            cli.cfg.planet_radius = pr;
            cli.cfg.planet_shell_thickness = pt;
            cli.cfg.gravity_accel = g;

            cli.enter_world_with_save(Some(name), Some(seed_val));
        }
=======
        if !create_error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, create_error.as_str());
            ui.add_space(6.0);
        }

        if sfx_play(ui.add_sized([290., 26.], egui::Button::new("Create World").fill(Color32::DARK_GREEN))).clicked() {
            let final_name = if tx_world_name.trim().is_empty() {
                format!("world_{}", crate::util::current_timestamp_millis())
            } else {
                tx_world_name.trim().to_string()
            };

            let seed = tx_world_seed
                .trim()
                .parse::<u64>()
                .unwrap_or_else(|_| crate::util::hashcode(&final_name));

            match crate::voxel::create_world(&final_name, seed) {
                Ok(_) => {
                    create_error.clear();
                    cli.data().curr_ui = CurrentUI::LocalWorldList;
                }
                Err(err) => {
                    *create_error = err.to_string();
                }
            }
        }

        ui.add_space(4.);
        let mut create_and_play_clicked = false;
        ui.add_enabled_ui(local_play_supported, |ui| {
            if sfx_play(ui.add_sized([290., 20.], egui::Button::new("Create & Play"))).clicked() {
                create_and_play_clicked = true;
            }
        });
        if !local_play_supported {
            ui.small("Create & Play is unavailable on this runtime.");
        }
        if create_and_play_clicked {
            let final_name = if tx_world_name.trim().is_empty() {
                format!("world_{}", crate::util::current_timestamp_millis())
            } else {
                tx_world_name.trim().to_string()
            };

            let seed = tx_world_seed
                .trim()
                .parse::<u64>()
                .unwrap_or_else(|_| crate::util::hashcode(&final_name));

            match crate::voxel::create_world(&final_name, seed) {
                Ok(meta) => {
                    create_error.clear();
                    if let Some(serv_cfg) = &serv_cfg {
                        cli.connect_local_world(meta.name, meta.seed, serv_cfg.port);
                    } else {
                        *create_error = "Integrated server unavailable on this runtime".to_string();
                    }
                }
                Err(err) => {
                    *create_error = err.to_string();
                }
            }
        }

>>>>>>> feature/world-persistence-8073199
        ui.add_space(4.);
        if sfx_play(ui.add_sized([290., 20.], egui::Button::new("Cancel"))).clicked() {
            cli.data().curr_ui = CurrentUI::LocalWorldList;
        }
    });
}
