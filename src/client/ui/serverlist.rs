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
                        if task.is_finished() {
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
        let local_world_supported = serv_cfg.is_some() || cfg!(target_arch = "wasm32");
        let mut do_refresh = false;
        let mut do_delete: Option<String> = None;
        let mut do_play: Option<crate::voxel::WorldMeta> = None;

        if !local_world_supported {
            ui.colored_label(Color32::YELLOW, "Local worlds are unavailable on this platform/runtime.");
            ui.small("Integrated server is not active. Use Multiplayer to connect to a remote server.");
            ui.add_space(8.0);
        }

        if !last_error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", *last_error));
            ui.add_space(6.0);
        }

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
                        let terrain_label = match world.config.terrain_mode {
                            crate::voxel::WorldTerrainMode::Planet => "Spherical",
                            crate::voxel::WorldTerrainMode::Flat => "Flat",
                            crate::voxel::WorldTerrainMode::SuperFlat => "SuperFlat",
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::WHITE, &world.name);
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                ui.label(format!("{} · seed {}", format_age_secs(world.last_played), world.seed));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(format!("{} · Persistent local world", terrain_label));
                            ui.with_layout(Layout::right_to_left(egui::Align::Max), |ui| {
                                if local_world_supported {
                                    if ui.btn("🗑").on_hover_text("Delete world").clicked() {
                                        do_delete = Some(world.name.clone());
                                    }
                                    if ui.btn("▶").on_hover_text("Play world").clicked() {
                                        do_play = Some(crate::voxel::WorldMeta {
                                            schema_version: world.schema_version,
                                            name: world.name.clone(),
                                            seed: world.seed,
                                            created: 0,
                                            last_played: world.last_played,
                                            config: world.config.clone(),
                                            owner_username: None,
                                            admin_usernames: Vec::new(),
                                        });
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

        if let Some(mut meta) = do_play {
            if meta.schema_version == 0 {
                match crate::voxel::migrate_world_meta(&meta.name, cli.cfg.terrain_mode) {
                    Ok(migrated) => {
                        meta = migrated;
                    }
                    Err(err) => {
                        *last_error = err.to_string();
                        return;
                    }
                }
            }
            if cfg!(target_arch = "wasm32") {
                cli.connect_local_world(meta, 0);
            } else if let Some(serv_cfg) = &serv_cfg {
                cli.connect_local_world(meta, serv_cfg.port);
            } else {
                *last_error = "Integrated server unavailable on this runtime".to_string();
            }
        }
    });
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedInputMode {
    Number,
    Hex,
    #[default]
    Text,
    Random,
    WorldNameHash,
    Daily,
    NameAndText,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionTag {
    Performance,
    Fun,
    Dangerous,
}

impl OptionTag {
    fn color(self) -> Color32 {
        match self {
            OptionTag::Performance => Color32::from_rgb(70, 140, 255),
            OptionTag::Fun => Color32::from_rgb(180, 90, 255),
            OptionTag::Dangerous => Color32::from_rgb(255, 70, 70),
        }
    }

    fn label(self) -> &'static str {
        match self {
            OptionTag::Performance => "Performance",
            OptionTag::Fun => "Fun",
            OptionTag::Dangerous => "Dangerous",
        }
    }
}

fn ui_option_row(ui: &mut egui::Ui, label: &str, tags: &[OptionTag], add_widget: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        for (idx, tag) in tags.iter().enumerate() {
            let (strip_rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 22.0), egui::Sense::hover());
            ui.painter().rect_filled(strip_rect, 1.0, tag.color());
            if idx + 1 < tags.len() {
                ui.add_space(2.0);
            }
        }
        ui.add_space(8.0);
        ui.label(label);
        add_widget(ui);
    });
}

fn next_random_seed(seed_source: &str) -> u64 {
    crate::util::hashcode(&format!("{}:{}", seed_source, crate::util::current_timestamp_millis()))
}

fn parse_hex_seed(seed_text: &str) -> Option<u64> {
    let trimmed = seed_text.trim();
    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if raw.is_empty() {
        return None;
    }
    u64::from_str_radix(raw, 16).ok()
}

fn daily_seed() -> u64 {
    let day = crate::util::current_timestamp().as_secs() / 86_400;
    crate::util::hashcode(&format!("daily:{day}"))
}

fn resolve_seed(final_name: &str, seed_text: &str, mode: SeedInputMode, random_seed: u64) -> u64 {
    let text = seed_text.trim();
    match mode {
        SeedInputMode::Number => text.parse::<u64>().unwrap_or_else(|_| crate::util::hashcode(final_name)),
        SeedInputMode::Hex => parse_hex_seed(text).unwrap_or_else(|| crate::util::hashcode(final_name)),
        SeedInputMode::Text => {
            if text.is_empty() {
                crate::util::hashcode(final_name)
            } else {
                crate::util::hashcode(text)
            }
        }
        SeedInputMode::Random => random_seed.max(1),
        SeedInputMode::WorldNameHash => crate::util::hashcode(final_name),
        SeedInputMode::Daily => daily_seed(),
        SeedInputMode::NameAndText => {
            if text.is_empty() {
                crate::util::hashcode(&format!("{}:default", final_name))
            } else {
                crate::util::hashcode(&format!("{}:{}", final_name, text))
            }
        }
    }
}

pub fn ui_create_world(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
    serv_cfg: Option<Res<ServerSettings>>,
    mut tx_world_name: Local<String>,
    mut tx_world_seed: Local<String>,
    mut seed_mode: Local<SeedInputMode>,
    mut random_seed: Local<u64>,
    mut world_config: Local<crate::voxel::WorldGenConfig>,
    mut advanced_open: Local<bool>,
    mut initialized: Local<bool>,
    mut create_error: Local<String>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };
    let local_play_supported = serv_cfg.is_some() || cfg!(target_arch = "wasm32");

    if !*initialized {
        *world_config = crate::voxel::WorldGenConfig::default();
        world_config.terrain_mode = cli.cfg.terrain_mode;
        if tx_world_name.trim().is_empty() {
            *tx_world_name = format!("world_{}", crate::util::current_timestamp_millis());
        }
        *random_seed = next_random_seed(tx_world_name.as_str());
        *initialized = true;
    }

    new_egui_window("New World").show(ctx_mut, |ui| {
        let space = 14.;
        let final_name = if tx_world_name.trim().is_empty() {
            format!("world_{}", crate::util::current_timestamp_millis())
        } else {
            tx_world_name.trim().to_string()
        };

        if *seed_mode == SeedInputMode::Random && *random_seed == 0 {
            *random_seed = next_random_seed(&final_name);
        }

        let resolved_seed = resolve_seed(&final_name, tx_world_seed.as_str(), *seed_mode, *random_seed);

        ui.horizontal_wrapped(|ui| {
            ui.label("Legend:");
            for tag in [OptionTag::Performance, OptionTag::Fun, OptionTag::Dangerous] {
                ui.colored_label(tag.color(), format!("| {}", tag.label()));
            }
        });
        ui.small("One option may have multiple tags. Multiple bars mean mixed traits.");
        ui.small("Dangerous options can dramatically change generation style or compatibility.");
        ui.add_space(8.0);

        ui_option_row(ui, "Name:", &[OptionTag::Fun], |ui| {
            sfx_play(ui.text_edit_singleline(&mut *tx_world_name));
        });
        ui.add_space(space);

        ui_option_row(ui, "World Type:", &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
            let is_planet = world_config.terrain_mode == crate::voxel::WorldTerrainMode::Planet;
            let is_flat = world_config.terrain_mode == crate::voxel::WorldTerrainMode::Flat;
            let is_superflat = world_config.terrain_mode == crate::voxel::WorldTerrainMode::SuperFlat;

            if sfx_play(ui.radio(is_planet, "Spherical Planet")).clicked() {
                world_config.terrain_mode = crate::voxel::WorldTerrainMode::Planet;
                cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::Planet;
            }
            if sfx_play(ui.radio(is_flat, "Flat World")).clicked() {
                world_config.terrain_mode = crate::voxel::WorldTerrainMode::Flat;
                cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::Flat;
            }
            if sfx_play(ui.radio(is_superflat, "SuperFlat World")).clicked() {
                world_config.terrain_mode = crate::voxel::WorldTerrainMode::SuperFlat;
                cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::SuperFlat;
            }
        });
        ui.add_space(space);

        ui_option_row(ui, "Seed Mode:", &[OptionTag::Fun], |ui| {
            egui::ComboBox::from_id_source("world_seed_mode")
                .selected_text(match *seed_mode {
                    SeedInputMode::Number => "Numeric (u64)",
                    SeedInputMode::Hex => "Hexadecimal (u64)",
                    SeedInputMode::Text => "Text Hash",
                    SeedInputMode::Random => "Random",
                    SeedInputMode::WorldNameHash => "World Name Hash",
                    SeedInputMode::Daily => "Daily Seed",
                    SeedInputMode::NameAndText => "Name + Text Hash",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::Number, "Numeric (u64)");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::Hex, "Hexadecimal (u64)");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::Text, "Text Hash");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::Random, "Random");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::WorldNameHash, "World Name Hash");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::Daily, "Daily Seed");
                    ui.selectable_value(&mut *seed_mode, SeedInputMode::NameAndText, "Name + Text Hash");
                });
        });

        match *seed_mode {
            SeedInputMode::Number => {
                ui_option_row(ui, "Seed Number (u64):", &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small("Use a decimal integer. Empty or invalid values fall back to world-name hashing.\n");
            }
            SeedInputMode::Hex => {
                ui_option_row(ui, "Seed Hex (u64):", &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small("Supports forms like 0x1A2B or 1A2B. Empty or invalid values fall back to world-name hashing.\n");
            }
            SeedInputMode::Text => {
                ui_option_row(ui, "Seed Text:", &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small("Text is hashed into a reproducible u64 seed. Empty text falls back to world-name hashing.\n");
            }
            SeedInputMode::Random => {
                ui_option_row(ui, "Random Seed:", &[OptionTag::Fun], |ui| {
                    ui.label(format!("Random Seed: {}", *random_seed));
                    if sfx_play(ui.button("Regenerate")).clicked() {
                        *random_seed = next_random_seed(&final_name);
                    }
                });
                ui.small("A random seed is generated once and stays stable until you regenerate it.\n");
            }
            SeedInputMode::WorldNameHash => {
                ui.label(format!("World Name Hash Seed: {}", crate::util::hashcode(&final_name)));
                ui.small("Seed is derived only from the world name. Renaming the world changes the seed.\n");
            }
            SeedInputMode::Daily => {
                let day = crate::util::current_timestamp().as_secs() / 86_400;
                ui.label(format!("Daily Seed: {}", daily_seed()));
                ui.small(format!("Daily seed rotates once per UTC day (day bucket: {}).\n", day));
            }
            SeedInputMode::NameAndText => {
                ui_option_row(ui, "Seed Text (combined with world name):", &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small("Seed is hashed from world name + text. Useful for themed variants under the same world name.\n");
            }
        }
        ui.colored_label(Color32::LIGHT_GREEN, format!("Resolved Seed: {}", resolved_seed));
        ui.small(format!(
            "Selected terrain: {}",
            match world_config.terrain_mode {
                crate::voxel::WorldTerrainMode::Planet => "Spherical Planet",
                crate::voxel::WorldTerrainMode::Flat => "Flat World",
                crate::voxel::WorldTerrainMode::SuperFlat => "SuperFlat World",
            }
        ));

        ui.add_space(8.0);
        egui::CollapsingHeader::new("Advanced Generation Parameters")
            .default_open(*advanced_open)
            .show(ui, |ui| {
                ui.label("General Parameters:");

                let mut oct = world_config.fbm_octaves as i32;
                if ui
                    .add(egui::Slider::new(&mut oct, 1..=12).text("FBM Octaves"))
                    .changed()
                {
                    world_config.fbm_octaves = oct as u8;
                }

                ui_option_row(ui, "Noise Scale 2D", &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                    ui.add(egui::Slider::new(&mut world_config.noise_scale_2d, 8.0..=2048.0));
                });
                ui_option_row(ui, "Noise Scale 3D", &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                    ui.add(egui::Slider::new(&mut world_config.noise_scale_3d, 8.0..=2048.0));
                });
                ui_option_row(ui, "Gravity (m/s²)", &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                    ui.add(egui::Slider::new(&mut world_config.gravity_acceleration, 0.0..=60.0));
                });
                ui_option_row(ui, "Spawn Surface Offset", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                    ui.add(egui::Slider::new(&mut world_config.spawn_surface_offset, 0.0..=128.0));
                });
                ui_option_row(ui, "Generation Backend", &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                    egui::ComboBox::from_id_source("world_gen_backend_pref")
                        .selected_text(match world_config.worldgen_backend {
                            crate::voxel::WorldGenBackendPreference::Auto => "Auto (Follow Client Setting)",
                            crate::voxel::WorldGenBackendPreference::CpuCompatible => "CPU Compatible (Stable Shape)",
                            crate::voxel::WorldGenBackendPreference::GpuFast => "GPU Fast (May Differ)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut world_config.worldgen_backend,
                                crate::voxel::WorldGenBackendPreference::Auto,
                                "Auto (Follow Client Setting)",
                            );
                            ui.selectable_value(
                                &mut world_config.worldgen_backend,
                                crate::voxel::WorldGenBackendPreference::CpuCompatible,
                                "CPU Compatible (Stable Shape)",
                            );
                            ui.selectable_value(
                                &mut world_config.worldgen_backend,
                                crate::voxel::WorldGenBackendPreference::GpuFast,
                                "GPU Fast (May Differ)",
                            );
                        });
                });

                egui::CollapsingHeader::new("Flat World Parameters")
                    .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::Flat)
                    .show(ui, |ui| {
                        ui_option_row(ui, "Height Divisor", &[OptionTag::Performance, OptionTag::Fun], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.flat_height_divisor, 1.0..=128.0));
                        });
                        ui_option_row(ui, "3D Noise Strength", &[OptionTag::Fun, OptionTag::Performance], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.flat_3d_noise_strength, 0.0..=16.0));
                        });
                        ui_option_row(ui, "Water Level (Y)", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.flat_water_level, -128..=128));
                        });
                    });

                egui::CollapsingHeader::new("SuperFlat Parameters")
                    .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::SuperFlat)
                    .show(ui, |ui| {
                        ui_option_row(ui, "Ground Level (Y)", &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.superflat_ground_level, -128..=256));
                        });
                        ui_option_row(ui, "Dirt Depth", &[OptionTag::Fun, OptionTag::Performance], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.superflat_dirt_depth, 1..=16));
                        });
                        ui_option_row(ui, "Water Level (Y)", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.superflat_water_level, -256..=256));
                        });
                        ui_option_row(ui, "Generate Trees", &[OptionTag::Fun], |ui| {
                            ui.checkbox(&mut world_config.superflat_generate_trees, "");
                        });
                    });

                egui::CollapsingHeader::new("Planet Parameters")
                    .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::Planet)
                    .show(ui, |ui| {
                        ui_option_row(ui, "Planet Radius", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.planet_radius, 32.0..=20_000.0).text("Radius"));
                            if sfx_play(ui.button("Default")).clicked() {
                                world_config.planet_radius = crate::voxel::WorldGenConfig::default().planet_radius;
                            }
                        });
                        ui_option_row(ui, "Planet Center", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                            ui.add(egui::DragValue::new(&mut world_config.planet_center.x).speed(1.0).prefix("x="));
                            ui.add(egui::DragValue::new(&mut world_config.planet_center.y).speed(1.0).prefix("y="));
                            ui.add(egui::DragValue::new(&mut world_config.planet_center.z).speed(1.0).prefix("z="));
                        });
                        ui_option_row(ui, "Shell Thickness", &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.planet_shell_thickness, 8.0..=512.0));
                        });
                        ui_option_row(ui, "Planet 3D Noise Strength", &[OptionTag::Fun, OptionTag::Performance], |ui| {
                            ui.add(egui::Slider::new(&mut world_config.planet_3d_noise_strength, 0.0..=8.0));
                        });
                        ui_option_row(ui, "Planet Inner Water", &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                            ui.checkbox(&mut world_config.planet_inner_water, "");
                        });
                    });
            });

        ui.add_space(22.);

        if !create_error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, create_error.as_str());
            ui.add_space(6.0);
        }

        let mut sanitized_config = (*world_config).clone();
        sanitized_config.sanitize();

        if sfx_play(ui.add_sized([290., 26.], egui::Button::new("Create World").fill(Color32::DARK_GREEN))).clicked() {
            match crate::voxel::create_world_with_config(&final_name, resolved_seed, sanitized_config.clone()) {
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
            match crate::voxel::create_world_with_config(&final_name, resolved_seed, sanitized_config.clone()) {
                Ok(meta) => {
                    create_error.clear();
                    if cfg!(target_arch = "wasm32") {
                        cli.connect_local_world(meta, 0);
                    } else if let Some(serv_cfg) = &serv_cfg {
                        cli.connect_local_world(meta, serv_cfg.port);
                    } else {
                        *create_error = "Integrated server unavailable on this runtime".to_string();
                    }
                }
                Err(err) => {
                    *create_error = err.to_string();
                }
            }
        }

        ui.add_space(4.);
        if sfx_play(ui.add_sized([290., 20.], egui::Button::new("Cancel"))).clicked() {
            cli.data().curr_ui = CurrentUI::LocalWorldList;
        }
    });
}
