use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_egui::{
    egui::{Layout, OpenUrl, RichText},
    EguiContexts,
};
use std::collections::HashMap;

use crate::client::prelude::*;
use crate::client::l10n;
use crate::{client::client_world::ClientPlayerInfo, ui::prelude::*};

thread_local! {
    static TOUCH_MENU_ICON_TEXTURES: std::cell::RefCell<HashMap<&'static str, egui::TextureId>> =
        std::cell::RefCell::new(HashMap::new());
    static TOUCH_MENU_ICON_HANDLES: std::cell::RefCell<HashMap<&'static str, Handle<Image>>> =
        std::cell::RefCell::new(HashMap::new());
    static TOUCH_MENU_BG_TEXTURES: std::cell::RefCell<HashMap<&'static str, egui::TextureId>> =
        std::cell::RefCell::new(HashMap::new());
    static TOUCH_MENU_BG_HANDLES: std::cell::RefCell<HashMap<&'static str, Handle<Image>>> =
        std::cell::RefCell::new(HashMap::new());
}

// Compute UV rectangle for `object-fit: cover` behaviour.
fn uv_cover_for(img_w: f32, img_h: f32, rect_w: f32, rect_h: f32) -> egui::Rect {
    if img_w <= 0.0 || img_h <= 0.0 {
        return egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }
    let scale = f32::max(rect_w / img_w, rect_h / img_h);
    let src_w = rect_w / scale;
    let src_h = rect_h / scale;
    let ox = (img_w - src_w) * 0.5;
    let oy = (img_h - src_h) * 0.5;
    let u0 = ox / img_w;
    let v0 = oy / img_h;
    let u1 = (ox + src_w) / img_w;
    let v1 = (oy + src_h) / img_h;
    egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1))
}

// Clear cached textures/handles for touch menu (called from settings when refreshing)
pub fn clear_touch_menu_caches(_images: &mut Assets<Image>) {
    TOUCH_MENU_ICON_TEXTURES.with(|s| s.borrow_mut().clear());
    TOUCH_MENU_ICON_HANDLES.with(|s| s.borrow_mut().clear());
    TOUCH_MENU_BG_TEXTURES.with(|s| s.borrow_mut().clear());
    TOUCH_MENU_BG_HANDLES.with(|s| s.borrow_mut().clear());
}

const TOUCH_MENU_ICON_PX: u32 = 96;

#[cfg(not(target_arch = "wasm32"))]
fn rasterize_touch_menu_svg_to_image(icon_name: &str, target_px: u32) -> Option<Image> {
    use resvg::{tiny_skia, usvg};

    let path = format!("assets/ui/touch_main_menu_tiles/{}.svg", icon_name);
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

fn touch_menu_icon_texture_id(
    icon_name: &'static str,
    ctx: &mut EguiContexts,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
) -> Option<egui::TextureId> {
    let existing = TOUCH_MENU_ICON_TEXTURES.with(|store| store.borrow().get(icon_name).copied());
    if existing.is_some() {
        return existing;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let handle = {
        if let Some(img) = rasterize_touch_menu_svg_to_image(icon_name, TOUCH_MENU_ICON_PX) {
            TOUCH_MENU_ICON_HANDLES.with(|store| {
                let mut handles = store.borrow_mut();
                if let Some(existing) = handles.get(icon_name) {
                    if let Some(dst) = images.get_mut(existing.id()) {
                        *dst = img;
                        existing.clone()
                    } else {
                        let h = images.add(img);
                        handles.insert(icon_name, h.clone());
                        h
                    }
                } else {
                    let h = images.add(img);
                    handles.insert(icon_name, h.clone());
                    h
                }
            })
        } else {
            TOUCH_MENU_ICON_HANDLES.with(|store| {
                let mut handles = store.borrow_mut();
                handles
                    .entry(icon_name)
                    .or_insert_with(|| asset_server.load(format!("ui/touch_main_menu_tiles/{}.png", icon_name)))
                    .clone()
            })
        }
    };

    #[cfg(target_arch = "wasm32")]
    let handle = TOUCH_MENU_ICON_HANDLES.with(|store| {
        let mut handles = store.borrow_mut();
        handles
            .entry(icon_name)
            .or_insert_with(|| asset_server.load(format!("ui/touch_main_menu_tiles/{}.png", icon_name)))
            .clone()
    });

    let texture_id = ctx.add_image(bevy_egui::EguiTextureHandle::Strong(handle));
    TOUCH_MENU_ICON_TEXTURES.with(|store| {
        store.borrow_mut().insert(icon_name, texture_id);
    });
    Some(texture_id)
}

fn touch_menu_background_texture_id(
    bg_name: &'static str,
    ctx: &mut EguiContexts,
    asset_server: &AssetServer,
) -> Option<egui::TextureId> {
    let existing = TOUCH_MENU_BG_TEXTURES.with(|store| store.borrow().get(bg_name).copied());
    if existing.is_some() {
        return existing;
    }

    let handle = TOUCH_MENU_BG_HANDLES.with(|store| {
        let mut handles = store.borrow_mut();
        handles
            .entry(bg_name)
            .or_insert_with(|| asset_server.load(format!("ui/touch_main_menu_tiles/{}.jpg", bg_name)))
            .clone()
    });

    let texture_id = ctx.add_image(bevy_egui::EguiTextureHandle::Strong(handle));
    TOUCH_MENU_BG_TEXTURES.with(|store| {
        store.borrow_mut().insert(bg_name, texture_id);
    });
    Some(texture_id)
}

fn build_startup_diagnostic_report(cli: &ClientInfo, cfg: &ClientSettings) -> String {
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let parallelism = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1);
    format!(
        "{}\n{}: {}\n{}: {}\n{}: {}\n{}: {:?}\n{}: {}\n{}: {}\n{}: {}\n{}: ({}, {})\n",
        l10n::tr("ethertia diagnostic"),
        l10n::tr("version"),
        crate::VERSION,
        l10n::tr("platform"),
        platform,
        l10n::tr("parallelism"),
        parallelism,
        l10n::tr("current_ui"),
        cli.curr_ui,
        l10n::tr("server_addr"),
        cli.server_addr,
        l10n::tr("username"),
        cfg.username,
        l10n::tr("vsync"),
        cfg.vsync,
        l10n::tr("chunk_load_distance"),
        cfg.chunks_load_distance.x,
        cfg.chunks_load_distance.y,
    )
}

pub fn ui_main_menu(
    // mut rendered_texture_id: Local<egui::TextureId>,
    // asset_server: Res<AssetServer>,
    mut app_exit_events: EventWriter<AppExit>,
    mut ctx: EguiContexts,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cli: ResMut<ClientInfo>,
    cfg: Res<ClientSettings>,
    mut copied_feedback: Local<f32>,
    time: Res<Time>,
    // cmds: Commands,
    // mut dbg_server_addr: Local<String>,
) {
    // if *rendered_texture_id == egui::TextureId::default() {
    //     *rendered_texture_id = ctx.add_image(asset_server.load("ui/main_menu/1.png"));
    // }
    // let img = ctx.add_image(asset_server.load("proto.png"));

    if cfg.touch_ui {
        let icon_singleplayer = touch_menu_icon_texture_id("singleplayer", &mut ctx, &mut images, &asset_server);
        let icon_multiplayer = touch_menu_icon_texture_id("multiplayer", &mut ctx, &mut images, &asset_server);
        let icon_settings = touch_menu_icon_texture_id("settings", &mut ctx, &mut images, &asset_server);
        let icon_terminate = touch_menu_icon_texture_id("terminate", &mut ctx, &mut images, &asset_server);
        let bg_singleplayer = touch_menu_background_texture_id("singleplayer_bg", &mut ctx, &asset_server);
        let bg_multiplayer = touch_menu_background_texture_id("multiplayer_bg", &mut ctx, &asset_server);
        let bg_settings = touch_menu_background_texture_id("settings_bg", &mut ctx, &asset_server);
        let bg_terminate = touch_menu_background_texture_id("terminate_bg", &mut ctx, &asset_server);

        // 平台类磁贴（统一底图和风格，icon_svg 可替换）
        let platform_tiles: [(&str, &str, Option<&str>, &str); 5] = [
            ("Windows", "Win64/Win32", None, "windows"),
            ("Linux", "x86_64/aarch64", None, "linux"),
            ("macOS", "Intel/AppleSilicon", None, "macos"),
            ("Android", "Mobile", None, "android"),
            ("Web", "WASM", None, "web"),
        ];
        // 信息类磁贴（统一底图和风格，icon_svg 可替换）
        let info_tiles: [(&str, &str, Option<&str>, &str); 7] = [
            ("GitHub", "github.com/Dreamtowards/Ethertum", Some("https://github.com/Dreamtowards/Ethertum"), "github"),
            ("Steam", "steam(coming soon)", Some("https://store.steampowered.com/"), "steam"),
            ("YouTube", "youtube.com", Some("https://www.youtube.com/@Dreamtowards"), "youtube"),
            ("Docs", "docs.ethertia.com", Some("https://docs.ethertia.com"), "docs"),
            ("诊断", "复制诊断信息", None, "diagnostic"),
            ("模组", &format!("{} mods loaded.", 0), None, "mods"),
            ("版权", "Copyright © nil. Do distribute!", None, "copyright"),
        ];
        // 底图资源名，可后续替换
        // 底图资源名，可后续替换（SVG/PNG均可，建议SVG）
        let platform_bg_svg = "platform_bg";
        let info_bg_svg = "info_bg";

        // 动态加载SVG/PNG为TextureId（如无则降级为纯色）
        // 资源路径约定：assets/ui/touch_main_menu_tiles/{name}
        fn load_tile_texture(ctx: &mut EguiContexts, asset_server: &AssetServer, images: &mut Assets<Image>, name: &str) -> Option<egui::TextureId> {
            // 优先SVG，找不到则PNG
            let svg_path = format!("ui/touch_main_menu_tiles/{}", name);
            let png_path = format!("ui/touch_main_menu_tiles/{}.png", name.trim_end_matches(".svg"));
            let handle = asset_server.get_handle(svg_path.clone()).or_else(|| asset_server.get_handle(png_path.clone()));
            if let Some(h) = handle {
                Some(ctx.add_image(bevy_egui::EguiTextureHandle::Strong(h)))
            } else {
                None
            }
        }

        // 预先加载平台/信息类磁贴的底图与图标纹理，避免在 egui 闭包内再次对 `ctx` 做可变借用
        let platform_bg_tex = touch_menu_background_texture_id(platform_bg_svg, &mut ctx, &asset_server);
        let platform_icon_textures: Vec<Option<egui::TextureId>> = platform_tiles
            .iter()
            .map(|(_, _, _, icon_name)| touch_menu_icon_texture_id(icon_name, &mut ctx, &mut images, &asset_server))
            .collect();

        let info_bg_tex = touch_menu_background_texture_id(info_bg_svg, &mut ctx, &asset_server);
        let info_icon_textures: Vec<Option<egui::TextureId>> = info_tiles
            .iter()
            .map(|(_, _, _, icon_name)| touch_menu_icon_texture_id(icon_name, &mut ctx, &mut images, &asset_server))
            .collect();

        let Ok(ctx_mut) = ctx.ctx_mut() else {
            return;
        };

        let safe_top = crate::ui::ui_safe_top();

        egui::CentralPanel::default().show(ctx_mut, |ui| {
            ui.add_space((safe_top + 18.0).max(18.0));
            ui.vertical_centered(|ui| {
                ui.add(egui::Label::new(RichText::new("ethertia").heading().color(Color32::WHITE)));
                ui.add_space(8.0);
                ui.label(l10n::tr("Touch UI mode enabled"));
            });

            ui.add_space(20.0);

            let width = ui.available_width();
            let tile_size = if width > 1100.0 {
                egui::vec2(320.0, 132.0)
            } else if width > 760.0 {
                egui::vec2(280.0, 124.0)
            } else {
                egui::vec2((width - 20.0).max(220.0), 112.0)
            };

            let columns = if width > 1100.0 {
                3
            } else if width > 760.0 {
                2
            } else {
                1
            };

            let main_tiles = [
                (
                    l10n::tr("Singleplayer"),
                    l10n::tr("Local worlds and offline play"),
                    CurrentUI::LocalWorldList,
                    false,
                    icon_singleplayer,
                    bg_singleplayer,
                    true,
                ),
                (
                    l10n::tr("Multiplayer"),
                    l10n::tr("Join community servers"),
                    CurrentUI::ServerList,
                    false,
                    icon_multiplayer,
                    bg_multiplayer,
                    true,
                ),
                (
                    l10n::tr("Settings"),
                    l10n::tr("Graphics, controls and language"),
                    CurrentUI::Settings,
                    false,
                    icon_settings,
                    bg_settings,
                    true,
                ),
                (
                    l10n::tr("Terminate"),
                    l10n::tr("Exit the game"),
                    CurrentUI::MainMenu,
                    true,
                    icon_terminate,
                    bg_terminate,
                    true,
                ),
            ];

            ui.vertical_centered(|ui| {
                egui::Grid::new("touch_main_menu_tiles")
                    .num_columns(columns)
                    .spacing([14.0, 14.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (idx, (title, subtitle, target_ui, is_exit, icon_texture_id, bg_texture_id, icon_bottom_right)) in
                            main_tiles.iter().enumerate()
                        {
                            let (rect, response) = ui.allocate_exact_size(tile_size, egui::Sense::click());
                            let visuals = ui.style().interact(&response);

                            ui.painter().rect_filled(rect, 12.0, visuals.bg_fill);

                            if let Some(bg) = *bg_texture_id {
                                // Try to find the original image handle/name so we can read its pixel size
                                let mut uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                let bg_name_opt: Option<&'static str> = TOUCH_MENU_BG_TEXTURES.with(|store| {
                                    store
                                        .borrow()
                                        .iter()
                                        .find(|(_, &id)| id == bg)
                                        .map(|(k, _)| *k)
                                });

                                if let Some(bg_name) = bg_name_opt {
                                    let handle_opt = TOUCH_MENU_BG_HANDLES.with(|h| h.borrow().get(bg_name).cloned());
                                    if let Some(handle) = handle_opt {
                                        if let Some(img) = images.get(handle.id()) {
                                            let img_w = img.texture_descriptor.size.width as f32;
                                            let img_h = img.texture_descriptor.size.height as f32;
                                            uv = uv_cover_for(img_w, img_h, rect.width(), rect.height());
                                        }
                                    }
                                }

                                ui.painter().image(bg, rect, uv, egui::Color32::WHITE);
                                let alpha = (cfg.touch_menu_tile_overlay_strength * 255.0).round() as u8;
                                ui.painter().rect_filled(rect, 12.0, egui::Color32::from_black_alpha(alpha));
                            }

                            let title_pos = rect.left_top() + egui::vec2(16.0, 14.0);
                            let subtitle_pos = rect.left_top() + egui::vec2(16.0, 50.0);
                            ui.painter().text(
                                title_pos,
                                egui::Align2::LEFT_TOP,
                                title,
                                egui::FontId::proportional(26.0),
                                egui::Color32::WHITE,
                            );
                            ui.painter().text(
                                subtitle_pos,
                                egui::Align2::LEFT_TOP,
                                subtitle,
                                egui::FontId::proportional(18.0),
                                egui::Color32::from_white_alpha(230),
                            );

                            if let Some(icon) = *icon_texture_id {
                                if *icon_bottom_right {
                                    let size = egui::vec2(42.0, 42.0);
                                    let min = rect.right_bottom() - size - egui::vec2(10.0, 10.0);
                                    let icon_rect = egui::Rect::from_min_size(min, size);
                                    ui.painter().image(
                                        icon,
                                        icon_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE,
                                    );
                                } else {
                                    let size = egui::vec2(36.0, 36.0);
                                    let min = rect.left_bottom() - egui::vec2(0.0, size.y + 10.0) + egui::vec2(16.0, 0.0);
                                    let icon_rect = egui::Rect::from_min_size(min, size);
                                    ui.painter().image(
                                        icon,
                                        icon_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE,
                                    );
                                }
                            }

                            if response.clicked() {
                                if *is_exit {
                                    app_exit_events.write(AppExit::Success);
                                } else {
                                    cli.curr_ui = target_ui.clone();
                                }
                            }

                            if (idx + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });

                // 平台类磁贴区
                let platform_tile_size = egui::vec2(tile_size.x.min(180.0), 64.0);
                let platform_columns = if width > 1100.0 { 5 } else if width > 760.0 { 3 } else { 2 };
                egui::Grid::new("touch_main_menu_platform_tiles")
                    .num_columns(platform_columns)
                    .spacing([10.0, 10.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (idx, (title, subtitle, _url, icon_svg)) in platform_tiles.iter().enumerate() {
                            let (rect, response) = ui.allocate_exact_size(platform_tile_size, egui::Sense::click());
                            let visuals = ui.style().interact(&response);
                            // 底图（已预加载）
                            if let Some(tex) = platform_bg_tex {
                                ui.painter().image(tex, rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().rect_filled(rect, 10.0, egui::Color32::from_rgb(40, 60, 120));
                            }
                            // 图标区域
                            let icon_rect = egui::Rect::from_min_size(rect.left_top() + egui::vec2(10.0, 10.0), egui::vec2(36.0, 36.0));
                            let icon_tex = platform_icon_textures.get(idx).and_then(|v| *v);
                            if let Some(tex) = icon_tex {
                                ui.painter().image(tex, icon_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().rect_filled(icon_rect, 6.0, egui::Color32::from_rgb(180, 200, 255));
                            }
                            // 标题/副标题
                            let title_pos = rect.left_top() + egui::vec2(56.0, 12.0);
                            let subtitle_pos = rect.left_top() + egui::vec2(56.0, 34.0);
                            ui.painter().text(
                                title_pos,
                                egui::Align2::LEFT_TOP,
                                *title,
                                egui::FontId::proportional(18.0),
                                egui::Color32::WHITE,
                            );
                            ui.painter().text(
                                subtitle_pos,
                                egui::Align2::LEFT_TOP,
                                *subtitle,
                                egui::FontId::proportional(13.0),
                                egui::Color32::from_white_alpha(210),
                            );
                            // 点击事件预留
                            if response.clicked() {
                                // 可扩展
                            }
                            if (idx + 1) % platform_columns == 0 {
                                ui.end_row();
                            }
                        }
                    });

                // 信息类磁贴区
                let info_tile_size = egui::vec2(tile_size.x.min(180.0), 64.0);
                let info_columns = if width > 1100.0 { 5 } else if width > 760.0 { 3 } else { 2 };
                egui::Grid::new("touch_main_menu_info_tiles")
                    .num_columns(info_columns)
                    .spacing([10.0, 10.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (idx, (title, subtitle, url, icon_svg)) in info_tiles.iter().enumerate() {
                            let (rect, response) = ui.allocate_exact_size(info_tile_size, egui::Sense::click());
                            let visuals = ui.style().interact(&response);
                            // 底图（已预加载）
                            if let Some(tex) = info_bg_tex {
                                ui.painter().image(tex, rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().rect_filled(rect, 10.0, egui::Color32::from_rgb(60, 80, 60));
                            }
                            // 图标区域
                            let icon_rect = egui::Rect::from_min_size(rect.left_top() + egui::vec2(10.0, 10.0), egui::vec2(36.0, 36.0));
                            let icon_tex = info_icon_textures.get(idx).and_then(|v| *v);
                            if let Some(tex) = icon_tex {
                                ui.painter().image(tex, icon_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().rect_filled(icon_rect, 6.0, egui::Color32::from_rgb(200, 220, 180));
                            }
                            let title_pos = rect.left_top() + egui::vec2(56.0, 12.0);
                            let subtitle_pos = rect.left_top() + egui::vec2(56.0, 34.0);
                            ui.painter().text(
                                title_pos,
                                egui::Align2::LEFT_TOP,
                                *title,
                                egui::FontId::proportional(18.0),
                                egui::Color32::WHITE,
                            );
                            ui.painter().text(
                                subtitle_pos,
                                egui::Align2::LEFT_TOP,
                                *subtitle,
                                egui::FontId::proportional(13.0),
                                egui::Color32::from_white_alpha(210),
                            );
                            if response.clicked() {
                                if let Some(url) = url {
                                    ui.ctx().open_url(OpenUrl::new_tab(url));
                                } else if *title == "诊断" {
                                    let report = build_startup_diagnostic_report(&cli, &cfg);
                                    ui.ctx().copy_text(report);
                                } else if *title == "模组" {
                                    // 未来可弹出模组列表
                                }
                            }
                            if (idx + 1) % info_columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
        });
    } else {
        let Ok(ctx_mut) = ctx.ctx_mut() else {
            return;
        };

        egui::CentralPanel::default().show(ctx_mut, |ui| {
            let h = ui.available_height();
        //     img,
        //     Rect::from_min_size(pos2(100., 100.), vec2(200., 200.)),
        //     Rect::from_min_size(pos2(0., 0.), vec2(1., 1.)),
        //     Color32::WHITE
        // );

        // ui.painter().image(*rendered_texture_id, ui.max_rect(), Rect::from_min_max([0.0, 0.0].into(), [1.0, 1.0].into()), Color32::WHITE);

        ui.vertical_centered(|ui| {
            ui.add_space(h * 0.16);
            ui.add(egui::Label::new(RichText::new("ethertia").heading().color(Color32::WHITE)));
            ui.add_space(h * 0.24);

            // if dbg_server_addr.is_empty() {
            //     *dbg_server_addr = "127.0.0.1:4000".into();
            // }
            // ui.add_sized(siz, egui::TextEdit::singleline(&mut *dbg_server_addr));
            // if ui.add_sized(siz, egui::Button::new(l10n::tr("Connect to Server"))).clicked() {
            //     // 连接服务�?这两个操作会不会有点松散
            //     next_ui.set(CurrentUI::ConnectingServer);
            //     cli.connect_server(dbg_server_addr.clone());
            // }
            // // if ui.add_sized(siz, egui::Button::new(l10n::tr("Debug Local"))).clicked() {
            // //     // 临时的单人版方法 直接进入世界而不管网�?
            // //     next_ui.set(CurrentUI::None);
            // //     commands.insert_resource(WorldInfo::default());
            // // }
            // ui.label(l10n::tr("·"));

            // if ui.add_sized(siz, egui::Button::new(l10n::tr("Singleplayer"))).clicked() {
            //     next_ui.set(CurrentUI::LocalSaves);
            // }
            if ui.btn_normal(l10n::tr("Singleplayer")).clicked() {
                cli.curr_ui = CurrentUI::LocalWorldList;
            }
            if ui.btn_normal(l10n::tr("Multiplayer")).clicked() {
                cli.curr_ui = CurrentUI::ServerList;
            }
            if ui.btn_normal(l10n::tr("Settings")).clicked() {
                cli.curr_ui = CurrentUI::Settings;
            }
            if ui.btn_normal(l10n::tr("Terminate")).clicked() {
                app_exit_events.write(AppExit::Success);
            }

            ui.add_space(12.);
            if ui.btn_normal(l10n::tr("Copy Diagnostic Info")).clicked() {
                let report = build_startup_diagnostic_report(&cli, &cfg);
                ui.ctx().copy_text(report);
                *copied_feedback = time.elapsed_secs();
            }
            if *copied_feedback > 0.0 && time.elapsed_secs() - *copied_feedback < 3.0 {
                ui.small(l10n::tr("Copied to clipboard"));
            }

            let report_preview = build_startup_diagnostic_report(&cli, &cfg);
            ui.collapsing(l10n::tr("Diagnostic Preview"), |ui| {
                ui.code(report_preview);
            });
        });

        ui.with_layout(Layout::bottom_up(egui::Align::RIGHT), |ui| {
            ui.label(l10n::tr("Copyright © nil. Do distribute!"));
        });

        ui.with_layout(Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.horizontal(|ui| {
                #[cfg(not(target_os = "android"))]
                {
                    if sfx_play(ui.selectable_label(false, l10n::tr("GitHub"))).on_hover_text(l10n::tr("Github Repository")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("https://github.com/Dreamtowards/Ethertum"));
                    }
                    if sfx_play(ui.selectable_label(false, l10n::tr("Steam"))).on_hover_text(l10n::tr("Steam")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("https://github.com/Dreamtowards/Ethertum"));
                    }
                    if sfx_play(ui.selectable_label(false, l10n::tr("YouTube"))).on_hover_text(l10n::tr("YouTube")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("https://github.com/Dreamtowards/Ethertum"));
                    }
                    if sfx_play(ui.selectable_label(false, l10n::tr("Docs"))).on_hover_text(l10n::tr("Wiki & Documentations")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("https://docs.ethertia.com"));
                    }
                }
                ui.label(l10n::tr("|"));
                sfx_play(ui.selectable_label(false, l10n::tr("Windows")));
                sfx_play(ui.selectable_label(false, l10n::tr("Linux")));
                sfx_play(ui.selectable_label(false, l10n::tr("macOS")));
                sfx_play(ui.selectable_label(false, l10n::tr("Android")));
                ui.label(l10n::tr("·"));
                // ui.selectable_label(false, l10n::tr("Texture"));
                sfx_play(ui.selectable_label(false, l10n::tr("Web")));
                sfx_play(ui.selectable_label(false, l10n::tr("WASM")));
                sfx_play(ui.selectable_label(false, l10n::tr("Disk")));
                // ui.selectable_label(false, l10n::tr("Cloud"));
                sfx_play(ui.selectable_label(false, l10n::tr("Network")));
            });
            ui.label(format!("v{}\n{}", crate::VERSION, l10n::tr("0 mods loaded.")));
        });
    });
    }
}

pub fn ui_pause_menu(
    mut ctx: EguiContexts,
    mut cli: EthertiaClient,
    mut player: ResMut<ClientPlayerInfo>,
    mut inv_ui_state: ResMut<super::items::InventoryUiState>,
    mut vox_brush: ResMut<crate::voxel::VoxelBrush>,
    items: Option<Res<crate::item::Items>>,
    time: Res<Time>,
    mut last_save_feedback: Local<f32>,
    // mut net_client: ResMut<RenetClient>,
) {
    let Some(items) = items else {
        return;
    };

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    egui::Window::new(l10n::tr("Inventory")).show(ctx_mut, |ui| {
        super::items::ui_inventory_operation_first(ui, &mut player.inventory, &items, &mut inv_ui_state, Some(vox_brush));
    });

    super::new_egui_window(l10n::tr("Pause"))
        .anchor(Align2::CENTER_TOP, [0., 32.])
        .show(ctx_mut, |ui| {
            ui.horizontal(|ui| {
                if ui.add_sized([140.0, 42.0], egui::Button::new(l10n::tr("Resume"))).clicked() {
                    cli.data().curr_ui = CurrentUI::None;
                }
                ui.label(l10n::tr("Press ESC to return to game"));
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.toggle_value(&mut false, l10n::tr("Map"));
                ui.toggle_value(&mut false, l10n::tr("Inventory"));
                ui.toggle_value(&mut false, l10n::tr("Team"));
                ui.toggle_value(&mut false, l10n::tr("Abilities"));
                ui.toggle_value(&mut false, l10n::tr("Quests"));
                ui.separator();

                if ui.toggle_value(&mut false, l10n::tr("Settings")).clicked() {
                    cli.data().curr_ui = CurrentUI::Settings;
                }

                if cli.data().is_admin && ui.toggle_value(&mut false, l10n::tr("Admin")).clicked() {
                    let state = &mut cli.data().admin_panel_open;
                    *state = !*state;
                }

                if cli.data().is_admin && ui.toggle_value(&mut false, l10n::tr("World Editor")).clicked() {
                    let data = cli.data();
                    data.curr_ui = CurrentUI::WorldEditor;
                    data.global_editor_view = true;
                    data.enable_cursor_look = false;
                }

                if ui.toggle_value(&mut false, l10n::tr("Save World")).clicked() {
                    cli.request_save_world();
                    *last_save_feedback = time.elapsed_secs();
                }

                if ui.toggle_value(&mut false, l10n::tr("Quit")).clicked() {
                    cli.exit_world();
                }
            });

            if *last_save_feedback > 0.0 && time.elapsed_secs() - *last_save_feedback < 2.0 {
                ui.small(l10n::tr("World save requested"));
            }
            if cli.data().is_admin {
                ui.small(l10n::tr("Tip: Press F10 to toggle World Editor mode quickly."));
            }
        });

    // return;
    // egui::CentralPanel::default()
    //     .frame(Frame::default().fill(Color32::from_black_alpha(190)))
    //     .show(ctx.ctx_mut(), |ui| {
    //         let w = ui.available_width();

    //         let head_y = 75.;
    //         ui.painter().rect_filled(
    //             ui.max_rect().with_max_y(head_y),
    //             Rounding::ZERO,
    //             Color32::from_rgba_premultiplied(35, 35, 35, 210),
    //         );
    //         ui.painter().rect_filled(
    //             ui.max_rect().with_max_y(head_y).with_min_y(head_y - 2.),
    //             Rounding::ZERO,
    //             Color32::from_white_alpha(80),
    //         );

    //         ui.add_space(head_y - 27.);

    //         ui.horizontal(|ui| {
    //             ui.add_space((w - 420.) / 2.);

    //             ui.style_mut().spacing.button_padding.x = 10.;

    //             ui.toggle_value(&mut false, "Map");
    //             ui.toggle_value(&mut false, "Inventory");
    //             ui.toggle_value(&mut false, "Team");
    //             ui.toggle_value(&mut false, "Abilities");
    //             ui.toggle_value(&mut false, "Quests");
    //             ui.separator();

    //             if ui.toggle_value(&mut false, "Settings").clicked() {
    //                 cli.data().curr_ui = CurrentUI::Settings;
    //             }

    //             if ui.toggle_value(&mut false, "Quit").clicked() {
    //                 cli.exit_world();
    //             }
    //         });

    //         // let h = ui.available_height();
    //         // ui.add_space(h * 0.2);

    //         // ui.vertical_centered(|ui| {

    //         //     if ui.add_sized([200., 20.], egui::Button::new(l10n::tr("Continue"))).clicked() {
    //         //         next_state_ingame.set(GameInput::Controlling);
    //         //     }
    //         //     if ui.add_sized([200., 20.], egui::Button::new(l10n::tr("Back to Title"))).clicked() {
    //         //     }
    //         // });
}
