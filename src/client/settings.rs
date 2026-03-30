use bevy::reflect::Reflect;
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum TerrainMode {
    Planet,
    Flat,
}

// ClientSettings Configs

use crate::prelude::*;
use std::path::PathBuf;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future::poll_once;
use bevy::prelude::Commands;

#[derive(Resource)]
struct ClientSettingsLoadTask(Task<Option<ClientSettings>>);

pub const CLIENT_SETTINGS_FILE: &str = "client.settings.json";

fn client_settings_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(CLIENT_SETTINGS_FILE);
        }
    }

    PathBuf::from(CLIENT_SETTINGS_FILE)
}

fn on_app_init(mut cfg: ResMut<ClientSettings>, mut commands: Commands) {
    let cfg_path = client_settings_path();
    info!("Scheduling async load of {}", cfg_path.display());

    // Start background task to read config file without blocking startup/splash.
    let pool = AsyncComputeTaskPool::get();
    let cfg_path_clone = cfg_path.clone();
    let task = pool.spawn(async move {
        match std::fs::read_to_string(&cfg_path_clone) {
            Ok(s) => serde_json::from_str::<ClientSettings>(&s).ok(),
            Err(_) => None,
        }
    });

    commands.insert_resource(ClientSettingsLoadTask(task));
}

fn poll_settings_load(
    mut cfg: ResMut<ClientSettings>,
    mut maybe_task: Option<ResMut<ClientSettingsLoadTask>>,
    mut commands: Commands,
) {
    if let Some(mut task_res) = maybe_task {
        if task_res.0.is_finished() {
            if let Some(polled) = futures_lite::future::block_on(poll_once(&mut task_res.0)) {
                if let Some(val) = polled {
                    *cfg = val;
                    info!("Client settings loaded asynchronously");
                }
            }
            commands.remove_resource::<ClientSettingsLoadTask>();
        }
    }
}

fn on_app_exit(mut exit_events: MessageReader<bevy::app::AppExit>, cfg: Res<ClientSettings>) {
    for _ in exit_events.read() {
        info!("Program Terminate");

        let cfg_path = client_settings_path();
        info!("Saving {}", cfg_path.display());
        match serde_json::to_string_pretty(&*cfg) {
            Ok(content) => {
                if let Some(parent) = cfg_path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    if let Err(err) = std::fs::create_dir_all(parent) {
                        warn!("Failed to create settings directory {}: {err}", parent.display());
                    }
                }
                if let Err(err) = std::fs::write(&cfg_path, content) {
                    warn!("Failed to save {}: {err}", cfg_path.display());
                }
            }
            Err(err) => warn!("Failed to serialize {}: {err}", cfg_path.display()),
        }
    }
}

pub fn build_plugin(app: &mut App) {
    app.insert_resource(ClientSettings::default());
    app.register_type::<ClientSettings>();

    app.add_systems(PreStartup, on_app_init); // schedule async load of settings
    app.add_systems(Startup, poll_settings_load); // apply when ready
    app.add_systems(Last, on_app_exit); // save settings
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchActionBinding {
    Attack,
    UseItem,
    Jump,
    Sprint,
    Sneak,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct TouchControlsConfig {
    pub move_stick_pos: [f32; 2],
    pub move_stick_radius: f32,
    pub move_dead_zone: f32,

    pub attack_button_pos: [f32; 2],
    pub use_button_pos: [f32; 2],
    pub jump_button_pos: [f32; 2],
    pub sprint_button_pos: [f32; 2],
    pub crouch_button_pos: [f32; 2],
    pub button_radius: f32,

    pub attack_button_action: TouchActionBinding,
    pub use_button_action: TouchActionBinding,
    pub jump_button_action: TouchActionBinding,
    pub sprint_button_action: TouchActionBinding,
    pub crouch_button_action: TouchActionBinding,
}

impl Default for TouchControlsConfig {
    fn default() -> Self {
        Self {
            move_stick_pos: [0.18, 0.80],
            move_stick_radius: 120.0,
            move_dead_zone: 0.06,

            attack_button_pos: [0.84, 0.78],
            use_button_pos: [0.72, 0.84],
            jump_button_pos: [0.90, 0.66],
            sprint_button_pos: [0.64, 0.68],
            crouch_button_pos: [0.76, 0.66],
            button_radius: 44.0,

            attack_button_action: TouchActionBinding::Attack,
            use_button_action: TouchActionBinding::UseItem,
            jump_button_action: TouchActionBinding::Jump,
            sprint_button_action: TouchActionBinding::Sprint,
            crouch_button_action: TouchActionBinding::Sneak,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TouchLayoutPreset {
    pub name: String,
    pub layout: TouchControlsConfig,
}

impl Default for TouchLayoutPreset {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            layout: TouchControlsConfig::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct KeyboardMouseControlsConfig {
    pub look_sensitivity: f32,
    pub invert_y: bool,
    pub key_jump: String,
    pub key_sprint: String,
    pub key_sneak: String,
    pub key_pause: String,
}

impl Default for KeyboardMouseControlsConfig {
    fn default() -> Self {
        Self {
            look_sensitivity: 1.0,
            invert_y: false,
            key_jump: "Space".to_string(),
            key_sprint: "LControl".to_string(),
            key_sneak: "LShift".to_string(),
            key_pause: "Escape".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct GamepadControlsConfig {
    pub look_sensitivity: f32,
    pub invert_y: bool,
    pub left_stick_dead_zone: f32,
    pub right_stick_dead_zone: f32,
    pub button_jump: String,
    pub button_sprint: String,
    pub button_use: String,
    pub button_attack: String,
}

impl Default for GamepadControlsConfig {
    fn default() -> Self {
        Self {
            look_sensitivity: 1.0,
            invert_y: false,
            left_stick_dead_zone: 0.15,
            right_stick_dead_zone: 0.12,
            button_jump: "South(A/Cross)".to_string(),
            button_sprint: "LeftThumb".to_string(),
            button_use: "RightTrigger2".to_string(),
            button_attack: "RightTrigger".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ControlsConfig {
    pub gamepad: GamepadControlsConfig,
    pub keyboard_mouse: KeyboardMouseControlsConfig,
    pub touch: TouchControlsConfig,
    pub touch_layout_presets: Vec<TouchLayoutPreset>,

    #[serde(skip)]
    pub touch_layout_undo_stack: Vec<TouchControlsConfig>,
    #[serde(skip)]
    pub touch_layout_request_undo: bool,
    #[serde(skip)]
    pub touch_layout_share_text: String,
    #[serde(skip)]
    pub touch_layout_preset_name: String,
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            gamepad: GamepadControlsConfig::default(),
            keyboard_mouse: KeyboardMouseControlsConfig::default(),
            touch: TouchControlsConfig::default(),
            touch_layout_presets: Vec::new(),
            touch_layout_undo_stack: Vec::new(),
            touch_layout_request_undo: false,
            touch_layout_share_text: String::new(),
            touch_layout_preset_name: String::new(),
        }
    }
}

#[derive(Resource, Deserialize, Serialize, Reflect)]
#[reflect(Resource)]
pub struct ClientSettings {
    #[reflect(ignore)]
    pub serverlist: Vec<ServerListItem>,

    pub fov: f32,
    pub username: String,
    pub hud_padding: f32,
    pub vsync: bool,
    pub high_quality_rendering: bool,
    pub touch_ui: bool,

    pub chunks_load_distance: IVec2,
    
    #[serde(default)]
    #[reflect(ignore)]
    pub controls: ControlsConfig,

    // Custom planet parameters (persisted in client settings)
    #[reflect(ignore)]
    pub planet_center: [f32; 3],
    pub planet_radius: f32,
    pub planet_shell_thickness: f32,
    pub gravity_accel: f32,

    pub terrain_mode: TerrainMode, // 新增：地形模式
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            serverlist: Vec::default(),
            fov: 85.,
            username: crate::util::generate_simple_user_name(),
            hud_padding: 24.,
            vsync: true,
            high_quality_rendering: true,
            touch_ui: true,

            chunks_load_distance: IVec2::new(4, 3),
            controls: ControlsConfig::default(),
            planet_center: [0.0, 512.0, 0.0],
            planet_radius: 512.0,
            planet_shell_thickness: 96.0,
            gravity_accel: 9.81,
            terrain_mode: TerrainMode::Planet, // 默认球体
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct ServerListItem {
    pub name: String,
    pub addr: String,

    #[serde(skip)]
    pub ui: crate::ui::serverlist::UiServerInfo,
}
