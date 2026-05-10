mod debug;
pub mod hud;
mod items;
mod main_menu;
pub mod serverlist;
mod settings;

use std::{collections::HashMap, sync::{LazyLock, Mutex}};

static UI_WINDOW_MAXIMIZED: LazyLock<Mutex<HashMap<String, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub mod prelude {
    pub use super::items::{ui_inventory, ui_item_stack};
    pub use super::sfx_play;
    pub use super::CurrentUI;
    pub use super::UiExtra;
    pub use bevy_egui::egui::{self, pos2, vec2, Align2, Color32, InnerResponse, Rect};
    pub use bevy_egui::EguiContexts;
}

use bevy::{
    diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::PrimaryWindow,
};
use bevy::post_process::bloom::Bloom;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::Skybox;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{ScreenSpaceReflections};
use bevy_egui::{egui::{
    self, style::HandleShape, Align2, Color32, FontData, FontDefinitions, FontFamily, Layout, Pos2, Response, Rounding, Stroke, Ui, WidgetText,
}, EguiContextSettings, EguiContexts, EguiGlobalSettings, EguiMultipassSchedule, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, PrimaryEguiContext};
use egui_extras::{Size, StripBuilder};
use rand::Rng;

use crate::client::prelude::*;
use crate::client::l10n;

pub struct UiPlugin;

#[derive(Default, Resource)]
struct UiState {
    is_window_open: bool,
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>();
        app.init_resource::<items::InventoryUiState>();
        app.insert_resource(hud::ChatHistory::default());
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        
        {
            app
            .add_systems(
                PreStartup,
                setup_camera_system.before(EguiStartupSet::InitContexts),
            )
            .add_systems(
                Startup,
                (configure_visuals_system, configure_ui_state_system, init_ui_scale_factor_system),
            )
            .add_systems(PreUpdate, sync_ui_window_metrics_system)
            .add_systems(Update, ensure_world_camera_system.run_if(condition::in_world))
            .add_systems(Update, sync_camera_render_effects_system)
            .add_systems(Update, sync_l10n_language_system)
            .add_systems(Update, refresh_fonts_on_language_change)
            .add_systems(Update, items::flush_inventory_ui_ops.run_if(condition::in_world))
            .add_systems(
                EguiPrimaryContextPass,
                (
                    /* test */
                    ui_example_system,
                    /* debug */
                    debug::ui_menu_panel
                        .run_if(|cli: Res<ClientInfo>| cli.dbg_menubar)
                        .run_if(|cli: Res<ClientInfo>| cli.curr_ui != CurrentUI::WorldEditor),
                    debug::hud_debug_text.run_if(|cli: Res<ClientInfo>| cli.dbg_text).before(debug::ui_menu_panel),
                    debug::ui_admin_panel.run_if(condition::in_world),
                    debug::ui_world_editor_panel
                        .run_if(condition::in_world)
                        .run_if(condition::in_ui(CurrentUI::WorldEditor)),
                    /* hud */
                    (
                        hud::hud_hotbar,
                        hud::hud_attitude_indicators,
                        hud::hud_chat,
                        hud::hud_playerlist.run_if(condition::manipulating),
                        hud::hud_touch_sticks,
                    )
                        .run_if(condition::in_world)
                        .run_if(|cli: Res<ClientInfo>| cli.curr_ui != CurrentUI::WorldEditor),
                    items::draw_ui_holding_item,
                    /* menu */
                    (
                        settings::ui_settings.run_if(condition::in_ui(CurrentUI::Settings)),
                        main_menu::ui_pause_menu.run_if(condition::in_ui(CurrentUI::PauseMenu)),
                        // Menus
                        main_menu::ui_main_menu.run_if(condition::in_ui(CurrentUI::MainMenu)),
                        settings::ui_touch_tile_style_overlay.run_if(|cli: Res<ClientInfo>| cli.curr_ui == CurrentUI::MainMenu || cli.curr_ui == CurrentUI::Settings),
                        serverlist::ui_localsaves.run_if(condition::in_ui(CurrentUI::LocalWorldList)),
                        serverlist::ui_create_world.run_if(condition::in_ui(CurrentUI::LocalWorldNew)),
                        serverlist::ui_serverlist.run_if(condition::in_ui(CurrentUI::ServerList)),
                        //serverlist::ui_connecting_server.run_if(condition::in_ui(CurrentUI::ConnectingServer)),
                        serverlist::ui_disconnected_reason.run_if(condition::in_ui(CurrentUI::DisconnectedReason)),
                    )
                ),
            );
        }

        app.add_systems(First, play_bgm);

        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            // SystemInformationDiagnosticsPlugin,
        ));

        /*
        // Debug UI
        {
            app.add_systems(Update, debug::ui_menu_panel.run_if(|cli: Res<ClientInfo>| cli.dbg_menubar)); // Debug MenuBar. before CentralPanel
            app.add_systems(
                Update,
                debug::hud_debug_text
                    .run_if(|cli: Res<ClientInfo>| cli.dbg_text)
                    .before(debug::ui_menu_panel),
            );

            app.add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                EntityCountDiagnosticsPlugin,
                // SystemInformationDiagnosticsPlugin,
            ));
        }

        // HUDs
        {
            app.add_systems(
                Update,
                (hud::hud_hotbar, hud::hud_chat, hud::hud_playerlist.run_if(condition::manipulating), hud::hud_touch_sticks).run_if(condition::in_world),
            );
            app.insert_resource(hud::ChatHistory::default());

            app.add_systems(Update, items::draw_ui_holding_item);
        }

        app.add_systems(
            Update,
            (
                settings::ui_settings.run_if(condition::in_ui(CurrentUI::Settings)),
                main_menu::ui_pause_menu.run_if(condition::in_ui(CurrentUI::PauseMenu)),
                // Menus
                main_menu::ui_main_menu.run_if(condition::in_ui(CurrentUI::MainMenu)),
                serverlist::ui_localsaves.run_if(condition::in_ui(CurrentUI::LocalWorldList)),
                serverlist::ui_create_world.run_if(condition::in_ui(CurrentUI::LocalWorldNew)),
                serverlist::ui_serverlist.run_if(condition::in_ui(CurrentUI::ServerList)),
                serverlist::ui_connecting_server.run_if(condition::in_ui(CurrentUI::ConnectingServer)),
                serverlist::ui_disconnected_reason.run_if(condition::in_ui(CurrentUI::DisconnectedReason)),
            ), //.chain()
               //.before(debug::ui_menu_panel)
        );
        */
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub enum CurrentUI {
    None,
    #[default]
    MainMenu,
    PauseMenu,
    WorldEditor,
    Settings,
    ServerList,
    ConnectingServer,
    DisconnectedReason,
    ChatInput,
    LocalWorldList,
    LocalWorldNew,
}

// Shared UI runtime state that may be touched from multiple systems.
static UI_WINDOW_SIZE: Mutex<Vec2> = Mutex::new(Vec2::ZERO);
static UI_SAFE_TOP: Mutex<f32> = Mutex::new(0.0);

struct UiSfxState {
    hovered_id: egui::Id,
    last_hovered_id: egui::Id,
    clicked: bool,
    back_requested: bool,
}

impl Default for UiSfxState {
    fn default() -> Self {
        Self {
            hovered_id: egui::Id::NULL,
            last_hovered_id: egui::Id::NULL,
            clicked: false,
            back_requested: false,
        }
    }
}

static UI_SFX_STATE: Mutex<UiSfxState> = Mutex::new(UiSfxState {
    hovered_id: egui::Id::NULL,
    last_hovered_id: egui::Id::NULL,
    clicked: false,
    back_requested: false,
});

pub fn set_window_size(size: Vec2) {
    if let Ok(mut v) = UI_WINDOW_SIZE.lock() {
        *v = size;
    }
}

#[derive(Default)]
struct WindowResizeTracker {
    resizing: bool,
    last_size: Option<Vec2>,
}

pub fn set_ui_safe_top(v: f32) {
    if let Ok(mut top) = UI_SAFE_TOP.lock() {
        *top = v.max(0.0);
    }
}

pub fn ui_safe_top() -> f32 {
    if cfg!(target_os = "android") {
        UI_SAFE_TOP.lock().map(|v| *v).unwrap_or(42.0).max(32.0)
    } else {
        0.0
    }
}

pub(crate) struct UiWindow<'a> {
    window: egui::Window<'a>,
    title: &'a str,
}

pub(crate) fn new_egui_window(title: &str) -> UiWindow<'_> {
    UiWindow {
        window: egui::Window::new(title),
        title,
    }
}

impl<'a> UiWindow<'a> {
    pub fn anchor(mut self, anchor: Align2, offset: impl Into<egui::Vec2>) -> Self {
        self.window = self.window.anchor(anchor, offset);
        self
    }

    pub fn show(self, ctx: &egui::Context, add_contents: impl FnOnce(&mut egui::Ui)) -> Option<egui::InnerResponse<Option<()>>> {
        let title = self.title;
        let window_size = UI_WINDOW_SIZE.lock().map(|v| *v).unwrap_or(Vec2::ZERO);
        let window_margin = 16.0;
        let title_bar_margin = 28.0;

        let maximized = UI_WINDOW_MAXIMIZED
            .lock()
            .ok()
            .and_then(|store| store.get(title).copied())
            .unwrap_or(false);
        let window_id = egui::Id::new((title, maximized));

        let mut window = self.window
            .id(window_id)
            .default_size([680., 420.])
            .resizable(true)
            .title_bar(false)
            .collapsible(false);

        if cfg!(target_os = "android") {
            let safe_top = ui_safe_top();
            let width = (window_size.x - window_margin).max(320.0);
            let height = (window_size.y - safe_top - window_margin).max(220.0);
            window = window
                .fixed_rect(egui::Rect::from_min_size(
                    egui::pos2(0.0, safe_top),
                    egui::vec2(width, height),
                ))
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::LEFT_TOP, [0., safe_top]);
        } else if maximized {
            let width = (window_size.x - window_margin).max(320.0);
            let height = (window_size.y - window_margin).max(240.0);
            window = window
                .fixed_rect(egui::Rect::from_min_size(
                    egui::pos2(window_margin * 0.5, window_margin * 0.5),
                    egui::vec2(width, height),
                ))
                .resizable(false)
                .anchor(Align2::LEFT_TOP, [window_margin * 0.5, window_margin * 0.5]);
        } else if window_size.x - 680. < 100. || window_size.y - 420. < 100. {
            let width = (window_size.x - window_margin).max(320.0);
            let height = (window_size.y - window_margin - title_bar_margin).max(240.0);
            window = window.fixed_size([width, height]).resizable(false);
        }

        window.show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(title);
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if cfg!(not(target_os = "android")) {
                        let maximize_label = if maximized { l10n::tr("Restore") } else { l10n::tr("Maximize") };
                        let toggle_clicked = ui.button(maximize_label).on_hover_text(l10n::tr("Toggle maximize")).clicked();
                        if toggle_clicked {
                            if let Ok(mut map) = UI_WINDOW_MAXIMIZED.lock() {
                                let entry = map.entry(title.to_string()).or_insert(false);
                                *entry = !*entry;
                            }
                        }
                    }
                });
            });
            ui.separator();
            add_contents(ui);
        })
    }
}

pub fn color32_of(c: Srgba) -> Color32 {
    Color32::from_rgba_premultiplied((c.red*255.) as u8, (c.green*255.) as u8, (c.blue*255.) as u8, (c.alpha*255.) as u8)
}

pub fn color32_gray_alpha(gray: f32, alpha: f32) -> Color32 {
    let g = (gray * 255.) as u8;
    let a = (alpha * 255.) as u8;
    Color32::from_rgba_premultiplied(g, g, g, a)
}

fn setup_camera_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cli: Res<ClientInfo>,
) {
    spawn_main_camera(&mut commands, &asset_server, &cli);
}

fn ensure_world_camera_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cli: Res<ClientInfo>,
    query_cam: Query<Entity, With<CharacterControllerCamera>>,
) {
    if !query_cam.is_empty() {
        return;
    }

    error!("No CharacterControllerCamera found while world is loaded. Recreating fallback camera.");
    spawn_main_camera(&mut commands, &asset_server, &cli);
}

fn spawn_main_camera(commands: &mut Commands, asset_server: &AssetServer, cli: &ClientInfo) {
    // Spawn a minimal fallback Camera for menus/UI only. Heavy world resources
    // (cubemap skybox, envmap) are created when a world loads in
    // `client_world::on_world_init` to avoid loading GPU-heavy textures while in menus.
    let mut camera_entity = commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        CharacterControllerCamera,
        Name::new("Camera"),
        Msaa::Off,
    ));

    // Post-process / effects will be applied when the world initializes. Keep the
    // menu camera lightweight to prevent unnecessary GPU allocation when in UI.
}

fn sync_camera_render_effects_system(
    mut commands: Commands,
    cli: Res<ClientInfo>,
    skybox_cubemap: Option<Res<crate::client::client_world::SkyboxCubemap>>,
    query_cam: Query<(
        Entity,
        Option<&Skybox>,
        Option<&EnvironmentMapLight>,
        Option<&Fxaa>,
        Option<&Tonemapping>,
        Option<&Bloom>,
        Option<&ScreenSpaceReflections>,
        Option<&bevy::light::VolumetricFog>,
    ), With<CharacterControllerCamera>>,
) {
    for (
        camera_entity,
        has_skybox,
        has_envmap,
        has_fxaa,
        has_tonemapping,
        has_bloom,
        has_ssr,
        has_vol_fog,
    ) in query_cam.iter()
    {
        let mut ent = commands.entity(camera_entity);

        if cli.render_fxaa {
            if has_fxaa.is_none() {
                ent.insert(Fxaa::default());
            }
        } else if has_fxaa.is_some() {
            ent.remove::<Fxaa>();
        }

        if cli.render_tonemapping {
            if has_tonemapping.is_none() {
                ent.insert(Tonemapping::TonyMcMapface);
            }
        } else if has_tonemapping.is_some() {
            ent.remove::<Tonemapping>();
        }

        if cli.render_bloom {
            if has_bloom.is_none() {
                ent.insert(Bloom::default());
            }
        } else if has_bloom.is_some() {
            ent.remove::<Bloom>();
        }

        if cli.render_ssr {
            if has_ssr.is_none() {
                ent.insert(ScreenSpaceReflections::default());
            }
        } else if has_ssr.is_some() {
            ent.remove::<ScreenSpaceReflections>();
        }

        if cli.render_volumetric_fog {
            if has_vol_fog.is_none() {
                ent.insert(bevy::light::VolumetricFog {
                    ambient_color: Color::linear_rgb(
                        cli.volumetric_fog_color.x.clamp(0.0, 1.0),
                        cli.volumetric_fog_color.y.clamp(0.0, 1.0),
                        cli.volumetric_fog_color.z.clamp(0.0, 1.0),
                    ),
                    ambient_intensity: crate::client::client_world::volumetric_fog_intensity_from_density(
                        cli.volumetric_fog_density,
                    ),
                    ..default()
                });
            }
        } else if has_vol_fog.is_some() {
            ent.remove::<bevy::light::VolumetricFog>();
        }

        if cli.render_skybox {
            if let Some(cubemap) = skybox_cubemap.as_ref() {
                if has_skybox.is_none() {
                    ent.insert(Skybox {
                        image: cubemap.image_handle.clone(),
                        brightness: 1000.0,
                        ..Default::default()
                    });
                }
                if has_envmap.is_none() {
                    ent.insert(EnvironmentMapLight {
                        diffuse_map: cubemap.image_handle.clone(),
                        specular_map: cubemap.image_handle.clone(),
                        intensity: 1000.0,
                        ..Default::default()
                    });
                }
            }
        } else {
            if has_skybox.is_some() {
                ent.remove::<Skybox>();
            }
            if has_envmap.is_some() {
                ent.remove::<EnvironmentMapLight>();
            }
        }
    }
}

fn configure_visuals_system(mut contexts: EguiContexts, cfg: Res<ClientSettings>) -> Result {
    /*
    contexts.ctx_mut()?.style_mut(|style| {
        let visuals = &mut style.visuals;
        let round = Rounding::from(2.);
        
        visuals.window_rounding = round;
        visuals.widgets.noninteractive.rounding = round;
        visuals.widgets.inactive.rounding = round;
        visuals.widgets.hovered.rounding = round;
        visuals.widgets.active.rounding = round;
        visuals.widgets.open.rounding = round;
        visuals.window_rounding = round;
        visuals.menu_rounding = round;

        visuals.collapsing_header_frame = true;
        visuals.handle_shape = HandleShape::Rect { aspect_ratio: 0.5 };
        visuals.slider_trailing_fill = true;

        visuals.widgets.hovered.bg_stroke = Stroke::new(2.0, Color32::from_white_alpha(180));
        visuals.widgets.active.bg_stroke = Stroke::new(3.0, Color32::WHITE);

        visuals.widgets.inactive.weak_bg_fill = Color32::from_white_alpha(10); // button
        visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(20); // button hovered
        visuals.widgets.active.weak_bg_fill = Color32::from_white_alpha(60); // button pressed

        visuals.selection.bg_fill = Color32::from_rgb(27, 76, 201);
        visuals.selection.stroke = Stroke::new(2.0, color32_gray_alpha(1., 0.78)); // visuals.selection.bg_fill

        visuals.extreme_bg_color = color32_gray_alpha(0.02, 0.66); // TextEdit, ProgressBar, ScrollBar Bg, Plot Bg

        visuals.window_fill = color32_gray_alpha(0.1, 0.99);
        visuals.window_shadow = egui::epaint::Shadow {
            blur: 204,
            color: Color32::from_black_alpha(45),
            ..default()
        };
        visuals.popup_shadow = visuals.window_shadow;
    });
    */

    let language = crate::client::l10n::normalize_language(&cfg.language);
    apply_fonts_for_language(contexts.ctx_mut()?, language)?;
    Ok(())
}

fn refresh_fonts_on_language_change(
    mut contexts: EguiContexts,
    cfg: Res<ClientSettings>,
    mut last_language: Local<String>,
) -> Result {
    let normalized = crate::client::l10n::normalize_language(&cfg.language);
    if !cfg.is_changed() && !last_language.is_empty() {
        return Ok(());
    }

    if *last_language != normalized {
        *last_language = normalized.to_string();
        apply_fonts_for_language(contexts.ctx_mut()?, normalized)?;
    }

    Ok(())
}

fn apply_fonts_for_language(ctx: &egui::Context, language: &str) -> Result {
    let fonts = build_font_definitions(language)?;
    ctx.set_fonts(fonts);
    Ok(())
}

fn build_font_definitions(language: &str) -> Result<FontDefinitions> {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "ui_base".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/menlo.ttf"))),
    );

    let fallback_fonts = load_system_fonts_for_language(language, 3);
    if fallback_fonts.is_empty() {
        log::warn!(
            "[UI] No system fallback font found for language {}; non-Latin glyphs may be incomplete.",
            language
        );
    } else {
        for (idx, (source_path, bytes)) in fallback_fonts.into_iter().enumerate() {
            let key = format!("ui_fallback_{}", idx);
            log::info!("[UI] System fallback font loaded: {} ({} bytes)", source_path, bytes.len());
            fonts
                .font_data
                .insert(key, std::sync::Arc::new(FontData::from_owned(bytes)));
        }
    }

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .ok_or(crate::err_opt_is_none!())?
        .insert(0, "ui_base".to_owned());
    let fallback_keys: Vec<String> = fonts
        .font_data
        .keys()
        .filter(|k| k.starts_with("ui_fallback_"))
        .cloned()
        .collect();
    for key in &fallback_keys {
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .ok_or(crate::err_opt_is_none!())?
            .push(key.clone());
    }

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .ok_or(crate::err_opt_is_none!())?
        .insert(0, "ui_base".to_owned());
    for key in &fallback_keys {
        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .ok_or(crate::err_opt_is_none!())?
            .push(key.clone());
    }

    Ok(fonts)
}

fn load_system_fonts_for_language(language: &str, max_fonts: usize) -> Vec<(String, Vec<u8>)> {
    let mut loaded = Vec::new();
    for path in font_candidate_paths_for_language(language) {
        if loaded.len() >= max_fonts {
            break;
        }
        if loaded.iter().any(|(loaded_path, _)| loaded_path == path) {
            continue;
        }
        match std::fs::read(path) {
            Ok(bytes) => loaded.push((path.to_string(), bytes)),
            Err(_) => continue,
        }
    }
    loaded
}

fn font_candidate_paths_for_language(language: &str) -> Vec<&'static str> {
    let mut candidates = Vec::new();
    let script = language_script_group(language);

    #[cfg(target_os = "android")]
    {
        match script {
            "cjk" => candidates.extend([
                "/system/fonts/NotoSansCJK-Regular.ttc",
                "/system/fonts/NotoSansSC-Regular.otf",
                "/system/fonts/NotoSansJP-Regular.otf",
                "/system/fonts/NotoSansKR-Regular.otf",
            ]),
            "arabic" => candidates.extend([
                "/system/fonts/NotoNaskhArabic-Regular.ttf",
                "/system/fonts/NotoSansArabic-Regular.ttf",
            ]),
            "devanagari" => candidates.extend(["/system/fonts/NotoSansDevanagari-Regular.ttf"]),
            "bengali" => candidates.extend(["/system/fonts/NotoSansBengali-Regular.ttf"]),
            "thai" => candidates.extend(["/system/fonts/NotoSansThai-Regular.ttf"]),
            "hebrew" => candidates.extend(["/system/fonts/NotoSansHebrew-Regular.ttf"]),
            "cyrillic" | "latin" | "greek" => {}
            _ => {}
        }
        candidates.push("/system/fonts/DroidSansFallback.ttf");
    }

    #[cfg(target_os = "windows")]
    {
        match script {
            "cjk" => candidates.extend([
                "C:/Windows/Fonts/msyh.ttc",
                "C:/Windows/Fonts/msyh.ttf",
                "C:/Windows/Fonts/msjh.ttc",
                "C:/Windows/Fonts/meiryo.ttc",
                "C:/Windows/Fonts/msgothic.ttc",
                "C:/Windows/Fonts/malgun.ttf",
                "C:/Windows/Fonts/simsun.ttc",
                "C:/Windows/Fonts/simhei.ttf",
            ]),
            "arabic" => candidates.extend([
                "C:/Windows/Fonts/segoeui.ttf",
                "C:/Windows/Fonts/tradbdo.ttf",
                "C:/Windows/Fonts/arial.ttf",
            ]),
            "devanagari" => candidates.extend([
                "C:/Windows/Fonts/Nirmala.ttf",
                "C:/Windows/Fonts/mangal.ttf",
            ]),
            "bengali" => candidates.extend([
                "C:/Windows/Fonts/Nirmala.ttf",
                "C:/Windows/Fonts/vrinda.ttf",
            ]),
            "thai" => candidates.extend([
                "C:/Windows/Fonts/LeelawUI.ttf",
                "C:/Windows/Fonts/tahoma.ttf",
            ]),
            "hebrew" => candidates.extend([
                "C:/Windows/Fonts/arial.ttf",
                "C:/Windows/Fonts/segoeui.ttf",
            ]),
            "cyrillic" | "greek" | "latin" => {}
            _ => {}
        }
        candidates.extend([
            "C:/Windows/Fonts/segoeui.ttf",
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/seguisym.ttf",
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        match script {
            "cjk" => candidates.extend([
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            ]),
            "arabic" => candidates.extend([
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
                "/System/Library/Fonts/Supplemental/Geeza Pro.ttc",
            ]),
            "devanagari" => candidates.extend(["/System/Library/Fonts/Supplemental/Devanagari Sangam MN.ttc"]),
            "thai" => candidates.extend(["/System/Library/Fonts/Supplemental/Thonburi.ttc"]),
            "hebrew" => candidates.extend(["/System/Library/Fonts/Supplemental/Arial Hebrew.ttf"]),
            "bengali" | "cyrillic" | "greek" | "latin" => {}
            _ => {}
        }
        candidates.push("/System/Library/Fonts/Supplemental/Arial Unicode.ttf");
    }

    #[cfg(target_os = "linux")]
    {
        match script {
            "cjk" => candidates.extend([
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf",
                "/usr/share/fonts/opentype/noto/NotoSansJP-Regular.otf",
                "/usr/share/fonts/opentype/noto/NotoSansKR-Regular.otf",
            ]),
            "arabic" => candidates.extend([
                "/usr/share/fonts/truetype/noto/NotoNaskhArabic-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
            ]),
            "devanagari" => candidates.extend(["/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf"]),
            "bengali" => candidates.extend(["/usr/share/fonts/truetype/noto/NotoSansBengali-Regular.ttf"]),
            "thai" => candidates.extend(["/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf"]),
            "hebrew" => candidates.extend(["/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf"]),
            "cyrillic" | "greek" | "latin" => {}
            _ => {}
        }
        candidates.extend([
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ]);
    }

    candidates
}

fn language_script_group(language: &str) -> &'static str {
    match crate::client::l10n::normalize_language(language) {
        "zh-Hans" | "zh-Hant" | "lzh" | "ja-JP" | "ko-KR" => "cjk",
        "ru-RU" | "uk-UA" => "cyrillic",
        "ar-SA" | "ur-PK" | "fa-IR" => "arabic",
        "he-IL" => "hebrew",
        "hi-IN" => "devanagari",
        "bn-BD" => "bengali",
        "th-TH" => "thai",
        "el-GR" => "greek",
        _ => "latin",
    }
}

fn configure_ui_state_system(mut ui_state: ResMut<UiState>) {
    ui_state.is_window_open = true;
}

fn init_ui_scale_factor_system(
    mut query_egui_camera: Query<(&mut EguiContextSettings, &Camera)>,
) {
    let Ok((mut egui_settings, _camera)) = query_egui_camera.single_mut() else {
        return;
    };

    // Keep touch UI readable and avoid inverse-DPI shrinking at startup.
    egui_settings.scale_factor = if cfg!(target_os = "android") { 1.2 } else { 1.0 };
}

fn sync_ui_window_metrics_system(
    mut resize_start: EventReader<bevy::window::WindowResizeStart>,
    mut resize_events: EventReader<bevy::window::WindowResized>,
    mut resize_end: EventReader<bevy::window::WindowResizeEnd>,
    query_window: Query<&Window, With<PrimaryWindow>>,
    mut tracker: Local<WindowResizeTracker>,
) {
    // On resize start: enter resize/drag mode and clear pending size.
    if resize_start.iter().next().is_some() {
        tracker.resizing = true;
        tracker.last_size = None;
    }

    // Process WindowResized events: if currently resizing, remember the last size
    // but do not call `set_window_size`. If not resizing, update immediately.
    for _ev in resize_events.iter() {
        if let Ok(window) = query_window.single() {
            let size = Vec2::new(window.resolution.width(), window.resolution.height());
            if tracker.resizing {
                tracker.last_size = Some(size);
            } else {
                set_window_size(size);
                if cfg!(target_os = "android") {
                    let safe_top = (window.resolution.height() * 0.045).clamp(32.0, 72.0);
                    set_ui_safe_top(safe_top);
                }
            }
        }
    }

    // On resize end: apply the last seen size (if any) once and leave resize mode.
    if resize_end.iter().next().is_some() {
        let final_size = tracker.last_size.or_else(|| {
            query_window.single().ok().map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        });
        if let Some(size) = final_size {
            set_window_size(size);
            if cfg!(target_os = "android") {
                if let Ok(window) = query_window.single() {
                    let safe_top = (window.resolution.height() * 0.045).clamp(32.0, 72.0);
                    set_ui_safe_top(safe_top);
                }
            }
        }
        tracker.resizing = false;
        tracker.last_size = None;
    }
}

fn sync_l10n_language_system(cfg: Res<ClientSettings>) {
    l10n::set_current_language(&cfg.language);
}

fn ui_example_system(
    mut ui_state: ResMut<UiState>,
    mut is_initialized: Local<bool>,
    mut contexts: EguiContexts,
) -> Result {
    if !*is_initialized {
        *is_initialized = true;
    }
    Ok(())
}

fn play_bgm(asset_server: Res<AssetServer>, mut cmds: Commands, mut limbo_played: Local<bool>, mut cli: ResMut<ClientInfo>) {
    if let Ok(mut sfx_state) = UI_SFX_STATE.lock() {
        if sfx_state.back_requested {
            sfx_state.back_requested = false;
            cli.curr_ui = CurrentUI::MainMenu;
        }
    }

    #[cfg(target_os = "android")]
    {
        return;
    }

    // if !*limbo_played {
    //     *limbo_played = true;

    //     let ls = [
    //         "sounds/music/limbo.ogg",
    //         "sounds/music/dead_voxel.ogg",
    //         // "sounds/music/milky_way_wishes.ogg",
    //         // "sounds/music/gion.ogg",
    //         "sounds/music/radiance.ogg",
    //     ];

    //     cmds.spawn(AudioBundle {
    //         source: asset_server.load(ls[rand::thread_rng().gen_range(0..ls.len())]),
    //         settings: PlaybackSettings::DESPAWN,
    //     });
    // }

    if let Ok(mut sfx_state) = UI_SFX_STATE.lock() {
        if sfx_state.hovered_id != egui::Id::NULL && sfx_state.hovered_id != sfx_state.last_hovered_id {
            cmds.spawn(
                AudioPlayer::<AudioSource>(asset_server.load("sounds/ui/button.ogg"))
                //.with_settings(PlaybackSettings::DESPAWN),
            );
        }
        sfx_state.last_hovered_id = sfx_state.hovered_id;
        sfx_state.hovered_id = egui::Id::NULL;

        if sfx_state.clicked {
            cmds.spawn(
                AudioPlayer::<AudioSource>(asset_server.load("sounds/ui/button_large.ogg"))
                //.with_settings(PlaybackSettings::DESPAWN),
            );
        }
        sfx_state.clicked = false;

    }
}

// UI Panel: Left-Navs and Right-Content
pub fn ui_lr_panel(ui: &mut Ui, separator: bool, mut add_nav: impl FnMut(&mut Ui), mut add_main: impl FnMut(&mut Ui)) {
    let nav_width = if cfg!(target_os = "android") { 180.0 } else { 120.0 };
    let mut builder = StripBuilder::new(ui).size(Size::exact(nav_width)); // Left
    if separator {
        builder = builder.size(Size::exact(0.0)); // Separator
    }
    builder
        .size(Size::remainder().at_least(300.0)) // Right
        .horizontal(|mut strip| {
            strip.strip(|builder| {
                builder.size(Size::remainder()).size(Size::exact(40.)).vertical(|mut strip| {
                    strip.cell(|ui| {
                        ui.add_space(8.);
                        ui.style_mut().spacing.item_spacing.y = 7.;
                        ui.style_mut().spacing.button_padding.y = 3.;

                        ui.with_layout(Layout::top_down_justified(egui::Align::Min), |ui| {
                            add_nav(ui);
                        });
                    });
                    strip.cell(|ui| {
                        ui.with_layout(Layout::bottom_up(egui::Align::Min), |ui| {
                            let back_resp = if cfg!(target_os = "android") {
                                sfx_play(ui.add_sized([140.0, 46.0], egui::Button::new(l10n::tr("Back"))))
                            } else {
                                sfx_play(ui.selectable_label(false, l10n::tr("⬅Back")))
                            };
                            if back_resp.clicked() {
                                if let Ok(mut sfx_state) = UI_SFX_STATE.lock() {
                                    sfx_state.back_requested = true;
                                }
                            }
                        });
                    });
                });
            });
            if separator {
                strip.cell(|_ui| {});
            }
            strip.cell(|ui| {
                if separator {
                    let p = ui.cursor().left_top() + egui::Vec2::new(-ui.style().spacing.item_spacing.x, 0.);
                    let p2 = Pos2::new(p.x, p.y + ui.available_height());
                    ui.painter().line_segment([p, p2], ui.visuals().widgets.noninteractive.bg_stroke);
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    add_main(ui);
                });
            });
        });
}

pub trait UiExtra {
    fn btn(&mut self, text: impl Into<WidgetText>) -> Response;

    fn btn_normal(&mut self, text: impl Into<WidgetText>) -> Response;

    fn btn_borderless(&mut self, text: impl Into<WidgetText>) -> Response;
}

pub fn sfx_play(resp: Response) -> Response {
    if let Ok(mut sfx_state) = UI_SFX_STATE.lock() {
        if resp.hovered() || resp.gained_focus() {
            sfx_state.hovered_id = resp.id;
        }
        if resp.clicked() {
            sfx_state.clicked = true;
        }
    }
    resp
}

impl UiExtra for Ui {
    fn btn(&mut self, text: impl Into<WidgetText>) -> Response {
        sfx_play(self.add(egui::Button::new(text)))
    }
    fn btn_normal(&mut self, text: impl Into<WidgetText>) -> Response {
        self.add_space(4.);
        if cfg!(target_os = "android") {
            sfx_play(self.add_sized([320., 56.], egui::Button::new(text)))
        } else {
            sfx_play(self.add_sized([220., 24.], egui::Button::new(text)))
        }
    }
    fn btn_borderless(&mut self, text: impl Into<WidgetText>) -> Response {
        sfx_play(self.add(egui::Button::selectable(false, text)))
    }
}
