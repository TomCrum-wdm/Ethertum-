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
                (configure_visuals_system, configure_ui_state_system),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    /* test */
                    (ui_example_system, update_ui_scale_factor_system),
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
        let w = window_size.clamp(Vec2::new(200.0, 140.0), window_size);
        return egui::Window::new(title)
            .fixed_size([w.x, w.y])
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, [0., 0.]);
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
            DespawnOnWorldUnload,
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
        // Android-safe path: avoid skybox/env-map and heavy post stack during startup.
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            DistanceFog {
                ..default()
            },
            CharacterControllerCamera,
            Name::new("Camera"),
            DespawnOnWorldUnload,
            Msaa::Off,
        ));
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

fn update_ui_scale_factor_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut toggle_scale_factor: Local<Option<bool>>,
    mut query_egui_camera: Query<(&mut EguiContextSettings, &Camera)>,
) {
    let Ok((mut egui_settings, camera)) = query_egui_camera.single_mut() else {
        return;
    };
    if keyboard_input.just_pressed(KeyCode::Slash) || toggle_scale_factor.is_none() {
        let use_default_scale = !toggle_scale_factor.unwrap_or(true);
        *toggle_scale_factor = Some(use_default_scale);

        let scale_factor = if use_default_scale {
            1.0
        } else {
            let target = camera.target_scaling_factor().unwrap_or(1.0);
            if target.is_finite() && target > f32::EPSILON {
                1.0 / target
            } else {
                1.0
            }
        };
        egui_settings.scale_factor = scale_factor;
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

        if sfx_state.back_requested {
            sfx_state.back_requested = false;
            cli.curr_ui = CurrentUI::MainMenu;
        }
    }
}

// UI Panel: Left-Navs and Right-Content
pub fn ui_lr_panel(ui: &mut Ui, separator: bool, mut add_nav: impl FnMut(&mut Ui), mut add_main: impl FnMut(&mut Ui)) {
    let mut builder = StripBuilder::new(ui).size(Size::exact(120.0)); // Left
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
                            if sfx_play(ui.selectable_label(false, "⬅Back")).clicked() {
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
        sfx_play(self.add_sized([220., 24.], egui::Button::new(text)))
    }
    fn btn_borderless(&mut self, text: impl Into<WidgetText>) -> Response {
        sfx_play(self.add(egui::SelectableLabel::new(false, text)))
    }
}
