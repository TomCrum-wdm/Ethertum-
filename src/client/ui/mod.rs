mod debug;
pub mod hud;
mod items;
mod main_menu;
pub mod serverlist;
mod settings;

use std::sync::Mutex;

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
            .add_systems(Update, items::flush_inventory_ui_ops.run_if(condition::in_world))
            .add_systems(
                EguiPrimaryContextPass,
                (
                    /* test */
                    ui_example_system,
                    /* debug */
                    debug::ui_menu_panel.run_if(|cli: Res<ClientInfo>| cli.dbg_menubar),
                    debug::hud_debug_text.run_if(|cli: Res<ClientInfo>| cli.dbg_text).before(debug::ui_menu_panel),
                    /* hud */
                    (hud::hud_hotbar, hud::hud_chat, hud::hud_playerlist.run_if(condition::manipulating), hud::hud_touch_sticks).run_if(condition::in_world),
                    items::draw_ui_holding_item,
                    /* menu */
                    (
                        settings::ui_settings.run_if(condition::in_ui(CurrentUI::Settings)),
                        main_menu::ui_pause_menu.run_if(condition::in_ui(CurrentUI::PauseMenu)),
                        // Menus
                        main_menu::ui_main_menu.run_if(condition::in_ui(CurrentUI::MainMenu)),
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

pub fn new_egui_window(title: &str) -> egui::Window {
    let size = [680., 420.];

    let mut w = egui::Window::new(title)
        .default_size(size)
        .resizable(true)
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .collapsible(false);

    let window_size = UI_WINDOW_SIZE.lock().map(|v| *v).unwrap_or(Vec2::ZERO);

    if cfg!(target_os = "android") {
        let safe_top = ui_safe_top();
        let width = window_size.x.max(320.0);
        let height = (window_size.y - safe_top).max(220.0);
        return egui::Window::new(title)
            .fixed_size([width, height])
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, [0., safe_top]);
    }

    if window_size.x - size[0] < 100. || window_size.y - size[1] < 100. {
        w = w.fixed_size([window_size.x - 12., window_size.y - 12.]).resizable(false);
    }

    w
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
) {
    spawn_main_camera(&mut commands, &asset_server);
}

fn ensure_world_camera_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query_cam: Query<Entity, With<CharacterControllerCamera>>,
) {
    if !query_cam.is_empty() {
        return;
    }

    error!("No CharacterControllerCamera found while world is loaded. Recreating fallback camera.");
    spawn_main_camera(&mut commands, &asset_server);
}

fn spawn_main_camera(commands: &mut Commands, asset_server: &AssetServer) {
    // WARNING: 不应该产生多个Camera 否则SSR不支持 很多东西也会非预期的绘制多次如gizmos
    // commands.spawn((
    //     Camera2d::default(),
    //     Camera {
    //         order: 10,
    //         hdr: true,  // Sync with Camera3d!
    //         ..default()
    //     }
    // ));

    #[cfg(not(target_os = "android"))]
    {
        // NOTE: 也许应该放在通用系统里初始化camera而不是ui里, 但毕竟依赖egui的初始化时序 先暂时放这吧
        let skybox_image = asset_server.load("table_mountain_2_puresky_4k_cubemap.jpg");
        commands.insert_resource(crate::client::client_world::SkyboxCubemap {
            is_loaded: false,
            image_handle: skybox_image.clone(),
        });

        // Desktop-quality path
        let mut camera_entity = commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            bevy::render::view::Hdr,
            bevy::core_pipeline::prepass::DepthPrepass,
            bevy::core_pipeline::prepass::DeferredPrepass,
            bevy::core_pipeline::prepass::NormalPrepass,
            DistanceFog {
                ..default()
            },
            Skybox {
                image: skybox_image.clone(),
                brightness: 1000.0,
                ..Default::default()
            },
            EnvironmentMapLight {
                diffuse_map: skybox_image.clone(),
                specular_map: skybox_image.clone(),
                intensity: 1000.0,
                ..Default::default()
            },
            CharacterControllerCamera,
            Name::new("Camera"),
            Msaa::Off,
        ));

        camera_entity
            .insert(ScreenSpaceReflections::default())
            .insert(Fxaa::default())
            .insert(Tonemapping::TonyMcMapface)
            .insert(Bloom::default())
            .insert(bevy::light::VolumetricFog {
                ambient_intensity: 0.,
                ..default()
            });
    }

    #[cfg(target_os = "android")]
    {
        // Android path: keep deferred features aligned with material shaders.
        let skybox_image = asset_server.load("table_mountain_2_puresky_4k_cubemap.jpg");
        commands.insert_resource(crate::client::client_world::SkyboxCubemap {
            is_loaded: false,
            image_handle: skybox_image.clone(),
        });

        let mut camera_entity = commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            bevy::render::view::Hdr,
            bevy::core_pipeline::prepass::DepthPrepass,
            bevy::core_pipeline::prepass::DeferredPrepass,
            bevy::core_pipeline::prepass::NormalPrepass,
            DistanceFog {
                color: Color::srgb(0.62, 0.72, 0.84),
                ..default()
            },
            Skybox {
                image: skybox_image.clone(),
                brightness: 1000.0,
                ..Default::default()
            },
            EnvironmentMapLight {
                diffuse_map: skybox_image.clone(),
                specular_map: skybox_image.clone(),
                intensity: 1000.0,
                ..Default::default()
            },
            CharacterControllerCamera,
            Name::new("Camera"),
            Msaa::Off,
        ));

        camera_entity
            .insert(ScreenSpaceReflections::default())
            .insert(Fxaa::default())
            .insert(Tonemapping::TonyMcMapface)
            .insert(Bloom::default())
            .insert(bevy::light::VolumetricFog {
                ambient_intensity: 0.,
                ..default()
            });
    }
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
    let Ok((
        camera_entity,
        has_skybox,
        has_envmap,
        has_fxaa,
        has_tonemapping,
        has_bloom,
        has_ssr,
        has_vol_fog,
    )) = query_cam.single() else {
        return;
    };

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
                ambient_intensity: 0.,
                ..default()
            });
        }
    } else if has_vol_fog.is_some() {
        ent.remove::<bevy::light::VolumetricFog>();
    }

    if cli.render_skybox {
        if let Some(cubemap) = skybox_cubemap {
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

fn configure_visuals_system(mut contexts: EguiContexts) -> Result {
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

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        std::sync::Arc::new(
            FontData::from_static(include_bytes!("../../../assets/fonts/menlo.ttf")),
        ),
    );

    // Put my font first (highest priority):
    fonts.families.get_mut(&FontFamily::Proportional).ok_or(crate::err_opt_is_none!())?.insert(0, "my_font".to_owned());

    // Put my font as last fallback for monospace:
    fonts.families.get_mut(&FontFamily::Monospace).ok_or(crate::err_opt_is_none!())?.push("my_font".to_owned());

    contexts.ctx_mut()?.set_fonts(fonts);
    Ok(())
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

fn sync_ui_window_metrics_system(query_window: Query<&Window, With<PrimaryWindow>>) {
    let Ok(window) = query_window.single() else {
        return;
    };

    set_window_size(Vec2::new(window.resolution.width(), window.resolution.height()));
    if cfg!(target_os = "android") {
        let safe_top = (window.resolution.height() * 0.045).clamp(32.0, 72.0);
        set_ui_safe_top(safe_top);
    }
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
                                sfx_play(ui.add_sized([140.0, 46.0], egui::Button::new("Back")))
                            } else {
                                sfx_play(ui.selectable_label(false, "⬅Back"))
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
        sfx_play(self.add(egui::SelectableLabel::new(false, text)))
    }
}
