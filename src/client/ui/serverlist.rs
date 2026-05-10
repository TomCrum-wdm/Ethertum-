use crate::{
    client::l10n,
    client::prelude::*,
    server::{dedicated_server::rcon::Motd, prelude::ServerSettings},
    util,
};
use bevy::{
    asset::RenderAssetUsages,
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task},
};
use bevy_egui::{
    egui::{self, Color32, Layout},
    EguiContexts,
};
use bevy_renet::renet::RenetClient;

use super::{sfx_play, ui_lr_panel, CurrentUI, UiExtra};

use super::new_egui_window;

thread_local! {
    static WORLDGEN_OPTION_ICON_TEXTURES: std::cell::RefCell<HashMap<String, egui::TextureId>> =
        std::cell::RefCell::new(HashMap::new());
    static WORLDGEN_OPTION_ICON_RASTER_PX: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
    static WORLDGEN_OPTION_ICON_IMAGE_HANDLES: std::cell::RefCell<HashMap<String, Handle<Image>>> =
        std::cell::RefCell::new(HashMap::new());
    static WORLDGEN_OPTION_UI_STYLE: std::cell::RefCell<OptionUiStyle> =
        std::cell::RefCell::new(OptionUiStyle::default());
}

const GRID_ICON_SIZE_MAX: f32 = 96.0;

const ICON_RASTER_MIN: u32 = 64;
const ICON_RASTER_MAX: u32 = 1024;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum OptionLayoutMode {
    #[default]
    List,
    Grid,
}

#[derive(Clone, Copy)]
struct OptionUiStyle {
    layout: OptionLayoutMode,
    icon_size: f32,
    text_scale: f32,
}

impl Default for OptionUiStyle {
    fn default() -> Self {
        Self {
            layout: OptionLayoutMode::List,
            icon_size: 14.0,
            text_scale: 1.0,
        }
    }
}

const WORLDGEN_OPTION_ICON_LABELS: &[&str] = &[
    "Name:",
    "World Type:",
    "Seed Mode:",
    "Seed Number (u64):",
    "Seed Hex (u64):",
    "Seed Text:",
    "Random Seed:",
    "Seed Text (combined with world name):",
    "FBM Octaves",
    "Noise Scale 2D",
    "Noise Scale 3D",
    "Gravity (m/s²)",
    "Spawn Surface Offset",
    "Generation Backend",
    "Base Terrain Voxel Style",
    "Height Divisor",
    "3D Noise Strength",
    "Water Level (Y)",
    "Ground Level (Y)",
    "Dirt Depth",
    "Generate Trees",
    "Planet Radius",
    "Planet Center",
    "Shell Thickness",
    "Planet 3D Noise Strength",
    "Planet Inner Water",
    "Enable Surface Decoration",
    "Surface Air Scan Depth",
    "Beach Max Y",
    "Beach Noise Scale",
    "Beach Noise Threshold",
    "Flora Noise Scale",
    "Bush Threshold",
    "Fern Threshold",
    "Rose Threshold",
    "Vine Spawn (/256)",
    "Vine Length Factor",
    "Tree Spawn (/256)",
    "Tree Trunk Height Base",
    "Tree Trunk Height Variance",
    "Tree Leaf Radius Base",
    "Tree Leaf Radius Variance",
    "Tree Local Height Cap",
];

fn option_icon_slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut prev_underscore = false;
    for ch in label.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "option".to_string()
    } else {
        out
    }
}

fn option_icon_asset_path(label: &str) -> String {
    format!("ui/worldgen_option_icons/{}.png", option_icon_slug(label))
}

fn option_icon_svg_disk_path(label: &str) -> String {
    format!("assets/ui/worldgen_option_icons/{}.svg", option_icon_slug(label))
}

fn icon_target_raster_px(icon_size_ui: f32, pixels_per_point: f32) -> u32 {
    let wanted = (icon_size_ui * pixels_per_point * 2.0).ceil() as u32;
    wanted.clamp(ICON_RASTER_MIN, ICON_RASTER_MAX).next_power_of_two().min(ICON_RASTER_MAX)
}

#[cfg(not(target_arch = "wasm32"))]
fn rasterize_option_svg_to_image(label: &str, target_px: u32) -> Option<Image> {
    use resvg::{tiny_skia, usvg};

    let path = option_icon_svg_disk_path(label);
    let data = std::fs::read(path).ok()?;

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).ok()?;

    let svg_size = tree.size();
    let sx = target_px as f32 / svg_size.width();
    let sy = target_px as f32 / svg_size.height();
    let scale = sx.min(sy).max(0.001);
    let tx = (target_px as f32 - svg_size.width() * scale) * 0.5;
    let ty = (target_px as f32 - svg_size.height() * scale) * 0.5;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);

    let mut pixmap = tiny_skia::Pixmap::new(target_px, target_px)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Some(Image::new_fill(
        Extent3d {
            width: target_px,
            height: target_px,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixmap.data(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ))
}

fn option_card_size(label: &str, icon_size: f32, text_scale: f32) -> egui::Vec2 {
    let text_w = (label.chars().count() as f32 * 7.2 * text_scale).clamp(90.0, 260.0);
    let width = (icon_size * 2.3 + text_w + 38.0).clamp(190.0, 420.0);
    let height = (icon_size + 80.0 + text_scale * 14.0).clamp(96.0, 180.0);
    egui::vec2(width, height)
}

fn with_option_layout(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let style = WORLDGEN_OPTION_UI_STYLE.with(|s| *s.borrow());
    if style.layout == OptionLayoutMode::Grid {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), content);
    } else {
        content(ui);
    }
}

fn ensure_option_icon_texture(
    label: &str,
    asset_server: &AssetServer,
    ctx: &mut EguiContexts,
    images: &mut Assets<Image>,
    target_raster_px: u32,
) {
    let already_loaded = WORLDGEN_OPTION_ICON_TEXTURES.with(|store| store.borrow().contains_key(label));
    let already_resolution_matched = WORLDGEN_OPTION_ICON_RASTER_PX
        .with(|store| store.borrow().get(label).is_some_and(|v| *v == target_raster_px));
    if already_loaded && already_resolution_matched {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let handle = {
        if let Some(svg_image) = rasterize_option_svg_to_image(label, target_raster_px) {
            WORLDGEN_OPTION_ICON_IMAGE_HANDLES.with(|store| {
                let mut handles = store.borrow_mut();
                if let Some(existing) = handles.get(label) {
                    if let Some(dst) = images.get_mut(existing.id()) {
                        *dst = svg_image;
                        existing.clone()
                    } else {
                        let h = images.add(svg_image);
                        handles.insert(label.to_string(), h.clone());
                        h
                    }
                } else {
                    let h = images.add(svg_image);
                    handles.insert(label.to_string(), h.clone());
                    h
                }
            })
        } else {
            WORLDGEN_OPTION_ICON_IMAGE_HANDLES.with(|store| {
                let mut handles = store.borrow_mut();
                handles
                    .entry(label.to_string())
                    .or_insert_with(|| asset_server.load(option_icon_asset_path(label)))
                    .clone()
            })
        }
    };

    #[cfg(target_arch = "wasm32")]
    let handle = WORLDGEN_OPTION_ICON_IMAGE_HANDLES.with(|store| {
        let mut handles = store.borrow_mut();
        handles
            .entry(label.to_string())
            .or_insert_with(|| asset_server.load(option_icon_asset_path(label)))
            .clone()
    });

    let texture = ctx.add_image(bevy_egui::EguiTextureHandle::Strong(handle));
    WORLDGEN_OPTION_ICON_TEXTURES.with(|store| {
        store.borrow_mut().insert(label.to_string(), texture);
    });
    WORLDGEN_OPTION_ICON_RASTER_PX.with(|store| {
        store.borrow_mut().insert(label.to_string(), target_raster_px);
    });
}

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

    new_egui_window(l10n::tr("Server List")).show(ctx_mut, |ui| {
        let h = ui.available_height();

        ui.vertical_centered(|ui| {
            ui.add_space(h * 0.2);

            if net_client.is_some_and(|e| e.is_connected()) {
                ui.label(l10n::tr("Authenticating & Logging in..."));
            } else {
                ui.label(l10n::tr("Connecting to the server..."));
            }
            ui.add_space(38.);
            ui.spinner();

            ui.add_space(h * 0.3);

            if ui.btn_normal(l10n::tr("Cancel")).clicked() {
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

    new_egui_window(l10n::tr("Disconnected Reason")).show(ctx_mut, |ui| {
        let h = ui.available_height();

        ui.vertical_centered(|ui| {
            ui.add_space(h * 0.2);

            ui.label(l10n::tr("Disconnected:"));
            ui.colored_label(Color32::WHITE, cli.disconnected_reason.as_str());

            ui.add_space(h * 0.3);

            if ui.btn_normal(l10n::tr("Back to title")).clicked() {
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

    new_egui_window(l10n::tr("Server List")).show(ctx_mut, |ui| {
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
                if ui.btn_borderless(l10n::tr("Add Server")).clicked() {
                    do_new_server.set(true);
                }
                if ui.btn_borderless(l10n::tr("Refresh All")).clicked() {
                    do_refresh_all.set(true);
                }
                if show_btn_stop_refresh && ui.btn_borderless(l10n::tr("Stop Refresh")).clicked() {
                    do_stop_refreshing.set(true);
                }
                ui.separator();
                if ui.btn_borderless(l10n::tr("Acquire List")).on_hover_text(l10n::tr("Get Official Server List")).clicked() {
                    do_acquire_list = true;
                }
                if ui.btn_borderless(l10n::tr("Direct Connect")).clicked() {}
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
                                ui.colored_label(Color32::DARK_RED, l10n::tr("Inaccessible")).on_hover_text(&ui_server_info.motd);
                            }

                            // Right: Ops
                            ui.with_layout(Layout::right_to_left(egui::Align::Max), |ui| {
                                if is_editing {
                                    if ui.btn(l10n::tr("Save")).clicked() {
                                        ui_server_info.is_editing = false;
                                    }
                                } else {
                                    if ui.btn(l10n::tr("Delete")).on_hover_text(l10n::tr("Delete")).clicked() {
                                        do_del_idx = Some(idx);
                                    }
                                    if ui.btn(l10n::tr("Edit")).on_hover_text(l10n::tr("Edit")).clicked() {
                                        ui_server_info.is_editing = true;
                                    }
                                    if is_refreshing {
                                        if ui.btn(l10n::tr("Stop")).on_hover_text(l10n::tr("Stop Refreshing")).clicked() {
                                            is_refreshing = false;
                                        }
                                    } else if ui.btn(l10n::tr("Refresh")).on_hover_text(l10n::tr("Refresh Server Status")).clicked() {
                                        is_refreshing = true;
                                    }
                                    if ui.btn(l10n::tr("Play")).on_hover_text(l10n::tr("Join & Play")).clicked() {
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
                        name: l10n::tr("Server Name").into(),
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

    new_egui_window(l10n::tr("Local Worlds")).show(ctx_mut, |ui| {
        let local_world_supported = serv_cfg.is_some() || cfg!(target_arch = "wasm32");
        let mut do_refresh = false;
        let mut do_delete: Option<String> = None;
        let mut do_play: Option<crate::voxel::WorldMeta> = None;

        if !local_world_supported {
            ui.colored_label(Color32::YELLOW, "Local worlds are unavailable on this platform/runtime.");
            ui.small(l10n::tr("Integrated server is not active. Use Multiplayer to connect to a remote server."));
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
                if ui.btn_borderless(l10n::tr("New World")).clicked() {
                    cli.data().curr_ui = CurrentUI::LocalWorldNew;
                }
                if ui.btn_borderless(l10n::tr("Refresh")).clicked() {
                    do_refresh = true;
                }
                if ui.btn_borderless(l10n::tr("Back")).clicked() {
                    cli.data().curr_ui = CurrentUI::MainMenu;
                }
            },
            |ui| {
                if worlds.is_empty() {
                    ui.label(l10n::tr("No local worlds yet. Click New World to create one."));
                }

                for world in worlds.iter() {
                    ui.group(|ui| {
                        let terrain_label = match world.config.terrain_mode {
                            crate::voxel::WorldTerrainMode::Planet => l10n::tr("Spherical"),
                            crate::voxel::WorldTerrainMode::Flat => l10n::tr("Flat"),
                            crate::voxel::WorldTerrainMode::SuperFlat => l10n::tr("SuperFlat"),
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::WHITE, &world.name);
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                ui.label(format!(
                                    "{} · {} {}",
                                    format_age_secs(world.last_played),
                                    l10n::tr("seed"),
                                    world.seed
                                ));
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.label(format!("{} · {}", terrain_label, l10n::tr("Persistent local world")));
                            ui.with_layout(Layout::right_to_left(egui::Align::Max), |ui| {
                                if local_world_supported {
                                    if ui.btn(l10n::tr("Delete")).on_hover_text(l10n::tr("Delete world")).clicked() {
                                        do_delete = Some(world.name.clone());
                                    }
                                    if ui.btn(l10n::tr("Play")).on_hover_text(l10n::tr("Play world")).clicked() {
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
                                    if ui.btn(l10n::tr("Delete")).on_hover_text(l10n::tr("Delete world")).clicked() {
                                        do_delete = Some(world.name.clone());
                                    }
                                    ui.add_enabled(false, egui::Button::new(l10n::tr("Play")))
                                        .on_hover_text(l10n::tr("Play is unavailable on this runtime"));
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
    let style = WORLDGEN_OPTION_UI_STYLE.with(|s| *s.borrow());
    if style.layout == OptionLayoutMode::Grid {
        let size = option_card_size(label, style.icon_size, style.text_scale);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_size(size);
            ui.set_max_width(size.x);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    WORLDGEN_OPTION_ICON_TEXTURES.with(|store| {
                        if let Some(icon) = store.borrow().get(label).copied() {
                            ui.image((icon, egui::vec2(style.icon_size, style.icon_size)));
                        }
                    });

                    ui.vertical(|ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Button);
                        ui.label(egui::RichText::new(label).size(13.0 * style.text_scale));
                        ui.horizontal_wrapped(|ui| {
                            for tag in tags {
                                ui.colored_label(tag.color(), format!("{}", tag.label()));
                            }
                        });
                    });
                });
                ui.add_space(6.0);
                add_widget(ui);
            });
        });
        ui.add_space(8.0);
        return;
    }

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
        WORLDGEN_OPTION_ICON_TEXTURES.with(|store| {
            if let Some(icon) = store.borrow().get(label).copied() {
                ui.add_space(4.0);
                ui.image((icon, egui::vec2(style.icon_size, style.icon_size)));
            }
        });
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

pub(crate) fn ui_create_world(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    serv_cfg: Option<Res<ServerSettings>>,
    mut tx_world_name: Local<String>,
    mut tx_world_seed: Local<String>,
    mut seed_mode: Local<SeedInputMode>,
    mut random_seed: Local<u64>,
    mut world_config: Local<crate::voxel::WorldGenConfig>,
    mut advanced_open: Local<bool>,
    mut initialized: Local<bool>,
    mut create_error: Local<String>,
    mut option_layout_mode: Local<OptionLayoutMode>,
    mut option_icon_size: Local<f32>,
    mut option_text_scale: Local<f32>,
) {
    let pixels_per_point = match ctx.ctx_mut() {
        Ok(c) => c.pixels_per_point(),
        Err(_) => 1.0,
    };
    let icon_size_ui = if *option_layout_mode == OptionLayoutMode::Grid {
        GRID_ICON_SIZE_MAX
    } else {
        option_icon_size.clamp(12.0, GRID_ICON_SIZE_MAX)
    };
    let target_raster_px = icon_target_raster_px(icon_size_ui, pixels_per_point);

    for label in WORLDGEN_OPTION_ICON_LABELS {
        ensure_option_icon_texture(
            label,
            &asset_server,
            &mut ctx,
            &mut images,
            target_raster_px,
        );
    }

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
        *option_layout_mode = OptionLayoutMode::List;
        *option_icon_size = 14.0;
        *option_text_scale = 1.0;
        *initialized = true;
    }

    WORLDGEN_OPTION_UI_STYLE.with(|s| {
        let mut icon_size = option_icon_size.clamp(12.0, GRID_ICON_SIZE_MAX);
        if *option_layout_mode == OptionLayoutMode::Grid {
            icon_size = GRID_ICON_SIZE_MAX;
        }
        *s.borrow_mut() = OptionUiStyle {
            layout: *option_layout_mode,
            icon_size,
            text_scale: option_text_scale.clamp(0.8, 1.4),
        };
    });

    new_egui_window(l10n::tr("New World")).show(ctx_mut, |ui| {
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
            ui.label(l10n::tr("Legend:"));
            for tag in [OptionTag::Performance, OptionTag::Fun, OptionTag::Dangerous] {
                ui.colored_label(tag.color(), format!("| {}", tag.label()));
            }
        });
        ui.small(l10n::tr("One option may have multiple tags. Multiple bars mean mixed traits."));
        ui.small(l10n::tr("Dangerous options can dramatically change generation style or compatibility."));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(l10n::tr("Option Layout:"));
            if ui.selectable_label(*option_layout_mode == OptionLayoutMode::List, l10n::tr("List")).clicked() {
                *option_layout_mode = OptionLayoutMode::List;
            }
            if ui.selectable_label(*option_layout_mode == OptionLayoutMode::Grid, l10n::tr("Icon Grid")).clicked() {
                *option_layout_mode = OptionLayoutMode::Grid;
                *option_icon_size = GRID_ICON_SIZE_MAX;
            }
            ui.add_space(10.0);
            ui.label(l10n::tr("Icon"));
            ui.add_enabled(
                *option_layout_mode != OptionLayoutMode::Grid,
                egui::Slider::new(&mut *option_icon_size, 12.0..=GRID_ICON_SIZE_MAX),
            );
            ui.label(l10n::tr("Text"));
            ui.add(egui::Slider::new(&mut *option_text_scale, 0.8..=1.4));
            if *option_layout_mode == OptionLayoutMode::Grid {
                ui.small(l10n::tr("Grid mode locks icon size to maximum."));
            }
        });
        ui.add_space(8.0);

        with_option_layout(ui, |ui| {
            ui_option_row(ui, l10n::tr("Name:"), &[OptionTag::Fun], |ui| {
                sfx_play(ui.text_edit_singleline(&mut *tx_world_name));
            });
            ui_option_row(ui, l10n::tr("World Type:"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                let is_planet = world_config.terrain_mode == crate::voxel::WorldTerrainMode::Planet;
                let is_flat = world_config.terrain_mode == crate::voxel::WorldTerrainMode::Flat;
                let is_superflat = world_config.terrain_mode == crate::voxel::WorldTerrainMode::SuperFlat;

                if sfx_play(ui.radio(is_planet, l10n::tr("Spherical Planet"))).clicked() {
                    world_config.terrain_mode = crate::voxel::WorldTerrainMode::Planet;
                    cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::Planet;
                }
                if sfx_play(ui.radio(is_flat, l10n::tr("Flat World"))).clicked() {
                    world_config.terrain_mode = crate::voxel::WorldTerrainMode::Flat;
                    cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::Flat;
                }
                if sfx_play(ui.radio(is_superflat, l10n::tr("SuperFlat World"))).clicked() {
                    world_config.terrain_mode = crate::voxel::WorldTerrainMode::SuperFlat;
                    cli.cfg.terrain_mode = crate::voxel::WorldTerrainMode::SuperFlat;
                }
            });

            ui_option_row(ui, l10n::tr("Seed Mode:"), &[OptionTag::Fun], |ui| {
                egui::ComboBox::from_id_source("world_seed_mode")
                    .selected_text(match *seed_mode {
                        SeedInputMode::Number => l10n::tr("Numeric (u64)"),
                        SeedInputMode::Hex => l10n::tr("Hexadecimal (u64)"),
                        SeedInputMode::Text => l10n::tr("Text Hash"),
                        SeedInputMode::Random => l10n::tr("Random"),
                        SeedInputMode::WorldNameHash => l10n::tr("World Name Hash"),
                        SeedInputMode::Daily => l10n::tr("Daily Seed"),
                        SeedInputMode::NameAndText => l10n::tr("Name + Text Hash"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::Number, l10n::tr("Numeric (u64)"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::Hex, l10n::tr("Hexadecimal (u64)"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::Text, l10n::tr("Text Hash"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::Random, l10n::tr("Random"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::WorldNameHash, l10n::tr("World Name Hash"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::Daily, l10n::tr("Daily Seed"));
                        ui.selectable_value(&mut *seed_mode, SeedInputMode::NameAndText, l10n::tr("Name + Text Hash"));
                    });
            });
        });
        ui.add_space(space);

        match *seed_mode {
            SeedInputMode::Number => {
                ui_option_row(ui, l10n::tr("Seed Number (u64):"), &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small(l10n::tr("Use a decimal integer. Empty or invalid values fall back to world-name hashing.\n"));
            }
            SeedInputMode::Hex => {
                ui_option_row(ui, l10n::tr("Seed Hex (u64):"), &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small(l10n::tr("Supports forms like 0x1A2B or 1A2B. Empty or invalid values fall back to world-name hashing.\n"));
            }
            SeedInputMode::Text => {
                ui_option_row(ui, l10n::tr("Seed Text:"), &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small(l10n::tr("Text is hashed into a reproducible u64 seed. Empty text falls back to world-name hashing.\n"));
            }
            SeedInputMode::Random => {
                ui_option_row(ui, l10n::tr("Random Seed:"), &[OptionTag::Fun], |ui| {
                    ui.label(format!("{}: {}", l10n::tr("Random Seed"), *random_seed));
                    if sfx_play(ui.button(l10n::tr("Regenerate"))).clicked() {
                        *random_seed = next_random_seed(&final_name);
                    }
                });
                ui.small(l10n::tr("A random seed is generated once and stays stable until you regenerate it.\n"));
            }
            SeedInputMode::WorldNameHash => {
                ui.label(format!("{}: {}", l10n::tr("World Name Hash Seed"), crate::util::hashcode(&final_name)));
                ui.small(l10n::tr("Seed is derived only from the world name. Renaming the world changes the seed.\n"));
            }
            SeedInputMode::Daily => {
                let day = crate::util::current_timestamp().as_secs() / 86_400;
                ui.label(format!("{}: {}", l10n::tr("Daily Seed"), daily_seed()));
                ui.small(format!("{} ({}: {}).\n", l10n::tr("Daily seed rotates once per UTC day"), l10n::tr("day bucket"), day));
            }
            SeedInputMode::NameAndText => {
                ui_option_row(ui, l10n::tr("Seed Text (combined with world name):"), &[OptionTag::Fun], |ui| {
                    sfx_play(ui.text_edit_singleline(&mut *tx_world_seed));
                });
                ui.small(l10n::tr("Seed is hashed from world name + text. Useful for themed variants under the same world name.\n"));
            }
        }
        ui.colored_label(Color32::LIGHT_GREEN, format!("{}: {}", l10n::tr("Resolved Seed"), resolved_seed));
        ui.small(format!(
            "{}: {}",
            l10n::tr("Selected terrain"),
            match world_config.terrain_mode {
                crate::voxel::WorldTerrainMode::Planet => l10n::tr("Spherical Planet"),
                crate::voxel::WorldTerrainMode::Flat => l10n::tr("Flat World"),
                crate::voxel::WorldTerrainMode::SuperFlat => l10n::tr("SuperFlat World"),
            }
        ));

        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::CollapsingHeader::new(l10n::tr("Advanced Generation Parameters"))
                    .default_open(*advanced_open)
                    .show(ui, |ui| {
                        ui.label(l10n::tr("General Parameters:"));

                        let mut oct = world_config.fbm_octaves as i32;
                        ui_option_row(ui, l10n::tr("FBM Octaves"), &[OptionTag::Performance, OptionTag::Fun], |ui| {
                            if ui
                                .add(egui::Slider::new(&mut oct, 1..=12).text(l10n::tr("FBM Octaves")))
                                .changed()
                            {
                                world_config.fbm_octaves = oct as u8;
                            }
                        });

                        with_option_layout(ui, |ui| {
                            ui_option_row(ui, l10n::tr("Noise Scale 2D"), &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                                ui.add(egui::Slider::new(&mut world_config.noise_scale_2d, 8.0..=2048.0));
                            });
                            ui_option_row(ui, l10n::tr("Noise Scale 3D"), &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                                ui.add(egui::Slider::new(&mut world_config.noise_scale_3d, 8.0..=2048.0));
                            });
                            ui_option_row(ui, l10n::tr("Gravity (m/s²)"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                ui.add(egui::Slider::new(&mut world_config.gravity_acceleration, 0.0..=60.0));
                            });
                            ui_option_row(ui, l10n::tr("Spawn Surface Offset"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                ui.add(egui::Slider::new(&mut world_config.spawn_surface_offset, 0.0..=128.0));
                            });
                            ui_option_row(ui, l10n::tr("Generation Backend"), &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                                egui::ComboBox::from_id_source("world_gen_backend_pref")
                                    .selected_text(match world_config.worldgen_backend {
                                        crate::voxel::WorldGenBackendPreference::Auto => l10n::tr("Auto (Follow Client Setting)"),
                                        crate::voxel::WorldGenBackendPreference::CpuCompatible => l10n::tr("CPU Compatible (Stable Shape)"),
                                        crate::voxel::WorldGenBackendPreference::GpuFast => l10n::tr("GPU Fast (May Differ)"),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut world_config.worldgen_backend,
                                            crate::voxel::WorldGenBackendPreference::Auto,
                                            l10n::tr("Auto (Follow Client Setting)"),
                                        );
                                        ui.selectable_value(
                                            &mut world_config.worldgen_backend,
                                            crate::voxel::WorldGenBackendPreference::CpuCompatible,
                                            l10n::tr("CPU Compatible (Stable Shape)"),
                                        );
                                        ui.selectable_value(
                                            &mut world_config.worldgen_backend,
                                            crate::voxel::WorldGenBackendPreference::GpuFast,
                                            l10n::tr("GPU Fast (May Differ)"),
                                        );
                                    });
                            });
                            ui_option_row(ui, l10n::tr("Base Terrain Voxel Style"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                egui::ComboBox::from_id_source("world_base_voxel_style")
                                    .selected_text(match world_config.base_voxel_style {
                                        crate::voxel::WorldBaseVoxelStyle::SmoothIsosurface => l10n::tr("Smooth Isosurface"),
                                        crate::voxel::WorldBaseVoxelStyle::BlockyCube => l10n::tr("Blocky Cube"),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut world_config.base_voxel_style,
                                            crate::voxel::WorldBaseVoxelStyle::SmoothIsosurface,
                                            l10n::tr("Smooth Isosurface"),
                                        );
                                        ui.selectable_value(
                                            &mut world_config.base_voxel_style,
                                            crate::voxel::WorldBaseVoxelStyle::BlockyCube,
                                            l10n::tr("Blocky Cube"),
                                        );
                                    });
                            });
                        });

                        egui::CollapsingHeader::new(l10n::tr("Flat World Parameters"))
                            .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::Flat)
                            .show(ui, |ui| {
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Height Divisor"), &[OptionTag::Performance, OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flat_height_divisor, 1.0..=128.0));
                                    });
                                    ui_option_row(ui, l10n::tr("3D Noise Strength"), &[OptionTag::Fun, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flat_3d_noise_strength, 0.0..=16.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Water Level (Y)"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flat_water_level, -128..=128));
                                    });
                                });
                            });

                        egui::CollapsingHeader::new(l10n::tr("SuperFlat Parameters"))
                            .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::SuperFlat)
                            .show(ui, |ui| {
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Ground Level (Y)"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.superflat_ground_level, -128..=256));
                                    });
                                    ui_option_row(ui, l10n::tr("Dirt Depth"), &[OptionTag::Fun, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.superflat_dirt_depth, 1..=16));
                                    });
                                    ui_option_row(ui, l10n::tr("Water Level (Y)"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.superflat_water_level, -256..=256));
                                    });
                                    ui_option_row(ui, l10n::tr("Generate Trees"), &[OptionTag::Fun], |ui| {
                                        ui.checkbox(&mut world_config.superflat_generate_trees, "");
                                    });
                                });
                            });

                        egui::CollapsingHeader::new(l10n::tr("Planet Parameters"))
                            .default_open(world_config.terrain_mode == crate::voxel::WorldTerrainMode::Planet)
                            .show(ui, |ui| {
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Planet Radius"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.planet_radius, 32.0..=20_000.0).text(l10n::tr("Radius")));
                                        if sfx_play(ui.button(l10n::tr("Default"))).clicked() {
                                            world_config.planet_radius = crate::voxel::WorldGenConfig::default().planet_radius;
                                        }
                                    });
                                    ui_option_row(ui, l10n::tr("Planet Center"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                        ui.add(egui::DragValue::new(&mut world_config.planet_center.x).speed(1.0).prefix(l10n::tr("x=")));
                                        ui.add(egui::DragValue::new(&mut world_config.planet_center.y).speed(1.0).prefix(l10n::tr("y=")));
                                        ui.add(egui::DragValue::new(&mut world_config.planet_center.z).speed(1.0).prefix(l10n::tr("z=")));
                                    });
                                    ui_option_row(ui, l10n::tr("Shell Thickness"), &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.planet_shell_thickness, 8.0..=512.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Planet 3D Noise Strength"), &[OptionTag::Fun, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.planet_3d_noise_strength, 0.0..=8.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Planet Inner Water"), &[OptionTag::Dangerous, OptionTag::Fun], |ui| {
                                        ui.checkbox(&mut world_config.planet_inner_water, "");
                                    });
                                });
                            });

                        egui::CollapsingHeader::new(l10n::tr("Surface Decoration / Flora (Hidden Engine Controls)"))
                            .default_open(false)
                            .show(ui, |ui| {
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Enable Surface Decoration"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.checkbox(&mut world_config.surface_decoration_enabled, "");
                                    });
                                    ui_option_row(ui, l10n::tr("Surface Air Scan Depth"), &[OptionTag::Performance, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.surface_air_scan_depth, 1..=16));
                                    });
                                });

                                ui.separator();
                                ui.label(l10n::tr("Beach Conversion (Stone -> Sand)"));
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Beach Max Y"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.beach_max_y, -256..=256));
                                    });
                                    ui_option_row(ui, l10n::tr("Beach Noise Scale"), &[OptionTag::Fun, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.beach_noise_scale, 1.0..=512.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Beach Noise Threshold"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.beach_noise_threshold, -1.0..=1.0));
                                    });
                                });

                                ui.separator();
                                ui.label(l10n::tr("Flora Placement"));
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Flora Noise Scale"), &[OptionTag::Fun, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flora_noise_scale, 1.0..=512.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Bush Threshold"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flora_bush_threshold, -1.0..=1.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Fern Threshold"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flora_fern_threshold, -1.0..=1.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Rose Threshold"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.flora_rose_threshold, -1.0..=1.0));
                                    });
                                });

                                ui.separator();
                                ui.label(l10n::tr("Vines"));
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Vine Spawn (/256)"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.vine_spawn_per_256, 0.0..=256.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Vine Length Factor"), &[OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.vine_length_factor, 0.0..=64.0));
                                    });
                                });

                                ui.separator();
                                ui.label(l10n::tr("Trees"));
                                with_option_layout(ui, |ui| {
                                    ui_option_row(ui, l10n::tr("Tree Spawn (/256)"), &[OptionTag::Fun, OptionTag::Dangerous], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_spawn_per_256, 0.0..=256.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Tree Trunk Height Base"), &[OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_trunk_height_base, 1..=32));
                                    });
                                    ui_option_row(ui, l10n::tr("Tree Trunk Height Variance"), &[OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_trunk_height_var, 0.0..=32.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Tree Leaf Radius Base"), &[OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_leaves_radius_base, 1..=16));
                                    });
                                    ui_option_row(ui, l10n::tr("Tree Leaf Radius Variance"), &[OptionTag::Fun], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_leaves_radius_var, 0.0..=16.0));
                                    });
                                    ui_option_row(ui, l10n::tr("Tree Local Height Cap"), &[OptionTag::Dangerous, OptionTag::Performance], |ui| {
                                        ui.add(egui::Slider::new(&mut world_config.tree_local_height_cap, 1..=64));
                                    });
                                });
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

        if sfx_play(ui.add_sized([290., 26.], egui::Button::new(l10n::tr("Create World")).fill(Color32::DARK_GREEN))).clicked() {
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
            if sfx_play(ui.add_sized([290., 20.], egui::Button::new(l10n::tr("Create & Play")))).clicked() {
                create_and_play_clicked = true;
            }
        });
        if !local_play_supported {
            ui.small(l10n::tr("Create & Play is unavailable on this runtime."));
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
        if sfx_play(ui.add_sized([290., 20.], egui::Button::new(l10n::tr("Cancel")))).clicked() {
            cli.data().curr_ui = CurrentUI::LocalWorldList;
        }
    });
}
