use bevy::reflect::{FromReflect, Reflect, TypePath};

// ClientSettings Configs

use crate::prelude::*;
use std::path::PathBuf;

pub const CLIENT_SETTINGS_FILE: &str = "client.settings.json";

fn client_settings_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return PathBuf::from(home).join(CLIENT_SETTINGS_FILE);
            }
        }

        for base in [
            "/data/user/0/com.ethertia.client/files",
            "/data/data/com.ethertia.client/files",
        ] {
            let base_path = PathBuf::from(base);
            if base_path.exists() {
                return base_path.join(CLIENT_SETTINGS_FILE);
            }
        }

        return PathBuf::from("/data/user/0/com.ethertia.client/files").join(CLIENT_SETTINGS_FILE);
    }

    PathBuf::from(CLIENT_SETTINGS_FILE)
}

fn on_app_init(mut cfg: ResMut<ClientSettings>) {
    let cfg_path = client_settings_path();
    info!("Loading {}", cfg_path.display());
    match std::fs::read_to_string(&cfg_path) {
        Ok(str) => {
            if let Ok(val) = serde_json::from_str(&str) {
                *cfg = val;
            }
        }
        Err(err) => {
            debug!("Skip loading {}: {err}", cfg_path.display());
        }
    }

    cfg.sanitize();
}

fn on_app_exit(mut exit_events: EventReader<bevy::app::AppExit>, cfg: Res<ClientSettings>) {
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

    app.add_systems(PreStartup, on_app_init); // load settings
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

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum ResizeMinigameMode {
    Ball,
    VoxelDda,
}

impl Default for ResizeMinigameMode {
    fn default() -> Self {
        ResizeMinigameMode::Ball
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Reflect)]
#[serde(default)]
#[reflect(Default)]
pub struct MainMenuTileStyle {
    /// When true, tile width expands to fully fill the available row width.
    pub main_tile_fill_width: bool,
    /// When true, tile height expands to fill remaining vertical space.
    pub main_tile_fill_height: bool,
    /// Ideal tile width used to derive columns from the panel width.
    pub main_tile_target_w: f32,
    /// Minimum width when only one column fits.
    pub main_tile_min_w_single: f32,
    /// Horizontal spacing between large tiles.
    pub main_tile_gap_x: f32,
    /// Vertical spacing between large tiles.
    pub main_tile_gap_y: f32,
    /// Height when tile width >= main_tile_wide_threshold.
    pub main_tile_h_wide: f32,
    /// Height when tile width >= main_tile_med_threshold.
    pub main_tile_h_med: f32,
    /// Height when tile width < main_tile_med_threshold.
    pub main_tile_h_narrow: f32,
    /// Width threshold for main_tile_h_wide.
    pub main_tile_wide_threshold: f32,
    /// Width threshold for main_tile_h_med.
    pub main_tile_med_threshold: f32,
    /// Per-side horizontal padding ratio; total padding ~= 2% when set to 0.01.
    pub main_tile_pad_x_ratio: f32,
    /// Per-side vertical padding ratio; total padding ~= 16% when set to 0.08.
    pub main_tile_pad_y_ratio: f32,
    /// Title font size.
    pub main_tile_title_size: f32,
    /// Subtitle font size.
    pub main_tile_subtitle_size: f32,
    /// Bottom-right icon size.
    pub main_tile_icon_br_size: f32,
    /// Bottom-left icon size.
    pub main_tile_icon_bl_size: f32,
    /// Max width for small tiles; height is fixed.
    pub small_tile_max_w: f32,
    /// Height for small tiles.
    pub small_tile_h: f32,
    /// Horizontal spacing between small tiles.
    pub small_tile_gap_x: f32,
    /// Vertical spacing between small tiles.
    pub small_tile_gap_y: f32,
    /// Icon size in small tiles.
    pub small_tile_icon_size: f32,
    /// Icon margin from the tile edge.
    pub small_tile_icon_margin: f32,
}

impl Default for MainMenuTileStyle {
    fn default() -> Self {
        Self {
            main_tile_fill_width: false,
            main_tile_fill_height: false,
            main_tile_target_w: 320.0,
            main_tile_min_w_single: 220.0,
            main_tile_gap_x: 14.0,
            main_tile_gap_y: 14.0,
            main_tile_h_wide: 170.0,
            main_tile_h_med: 156.0,
            main_tile_h_narrow: 144.0,
            main_tile_wide_threshold: 320.0,
            main_tile_med_threshold: 280.0,
            main_tile_pad_x_ratio: 0.01,
            main_tile_pad_y_ratio: 0.08,
            main_tile_title_size: 26.0,
            main_tile_subtitle_size: 18.0,
            main_tile_icon_br_size: 42.0,
            main_tile_icon_bl_size: 36.0,
            small_tile_max_w: 180.0,
            small_tile_h: 64.0,
            small_tile_gap_x: 10.0,
            small_tile_gap_y: 10.0,
            small_tile_icon_size: 36.0,
            small_tile_icon_margin: 10.0,
        }
    }
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
    pub vertical_slider_pos: [f32; 2],
    pub vertical_slider_height: f32,
    pub vertical_slider_width: f32,
    pub fly_double_tap_window_secs: f32,
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
            move_stick_pos: [0.11, 0.80],
            move_stick_radius: 120.0,
            move_dead_zone: 0.06,

            attack_button_pos: [0.84, 0.78],
            use_button_pos: [0.72, 0.84],
            jump_button_pos: [0.90, 0.66],
            sprint_button_pos: [0.64, 0.68],
            crouch_button_pos: [0.76, 0.66],
            vertical_slider_pos: [0.88, 0.68],
            vertical_slider_height: 220.0,
            vertical_slider_width: 64.0,
            fly_double_tap_window_secs: 0.46,
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
    pub rumble_debug_enabled: bool,
    pub rumble_weak_motor: f32,
    pub rumble_strong_motor: f32,
    pub rumble_duration_ms: u32,
    pub rumble_preset: u8,
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
            rumble_debug_enabled: true,
            rumble_weak_motor: 0.35,
            rumble_strong_motor: 0.55,
            rumble_duration_ms: 220,
            rumble_preset: 0,
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
#[serde(default)]
#[reflect(Resource)]
pub struct ClientSettings {
    #[reflect(ignore)]
    pub serverlist: Vec<ServerListItem>,

    pub fov: f32,
    pub username: String,
    pub hud_padding: f32,
    pub vsync: bool,
    /// When true, request the windowing backend to avoid continuous redraw
    /// during interactive resizing. Default: true.
    pub suppress_interactive_resize_redraw: bool,
    /// Number of frames to wait since last resize event before considering
    /// the interactive resize finished. Used when `suppress_interactive_resize_redraw` is true.
    pub interactive_resize_debounce_frames: u32,
    pub resize_minigame_mode: ResizeMinigameMode,
    pub high_quality_rendering: bool,
    pub touch_ui: bool,
    pub touch_menu_tile_overlay_strength: f32,
    pub touch_tile_style: TouchTileStyle,
    pub main_menu_tile_style: MainMenuTileStyle,
    pub touch_tile_style_overlay_enabled: bool,
    pub touch_tile_style_window_alpha: f32,
    pub show_level_indicator: bool,
    pub show_pitch_indicator: bool,
    pub language: String,

    pub chunks_load_distance: IVec2,
    pub surface_first_meshing: bool,
    pub surface_only_meshing: bool,
    pub gpu_worldgen: bool,
    pub gpu_worldgen_allow_persisted_world: bool,
    pub gpu_worldgen_batch_size: i32,
    pub gpu_worldgen_max_loading: i32,
    pub cpu_worldgen_max_loading: i32,
    pub gpu_worldgen_adaptive_backlog_mid: i32,
    pub gpu_worldgen_adaptive_backlog_high: i32,
    pub gpu_worldgen_adaptive_mult_low: i32,
    pub gpu_worldgen_adaptive_mult_mid: i32,
    pub gpu_worldgen_adaptive_mult_high: i32,
    pub gpu_worldgen_adaptive_batch_min: i32,
    pub gpu_worldgen_adaptive_batch_max: i32,
    
    #[serde(default)]
    #[reflect(ignore)]
    pub controls: ControlsConfig,

    pub terrain_mode: crate::voxel::WorldTerrainMode,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            serverlist: Vec::default(),
            fov: 85.,
            username: crate::util::generate_simple_user_name(),
            hud_padding: 24.,
            vsync: true,
            suppress_interactive_resize_redraw: true,
            interactive_resize_debounce_frames: 3,
            resize_minigame_mode: ResizeMinigameMode::Ball,
            high_quality_rendering: true,
            touch_ui: true,
            touch_menu_tile_overlay_strength: 0.38,
            touch_tile_style: TouchTileStyle::default(),
            main_menu_tile_style: MainMenuTileStyle::default(),
            touch_tile_style_overlay_enabled: false,
            touch_tile_style_window_alpha: 0.9,
            show_level_indicator: true,
            show_pitch_indicator: true,
            language: "en-US".to_string(),

            chunks_load_distance: IVec2::new(4, 3),
            surface_first_meshing: true,
            surface_only_meshing: false,
            gpu_worldgen: true,
            gpu_worldgen_allow_persisted_world: false,
            gpu_worldgen_batch_size: 16,
            gpu_worldgen_max_loading: 256,
            cpu_worldgen_max_loading: 8,
            gpu_worldgen_adaptive_backlog_mid: 24,
            gpu_worldgen_adaptive_backlog_high: 64,
            gpu_worldgen_adaptive_mult_low: 2,
            gpu_worldgen_adaptive_mult_mid: 4,
            gpu_worldgen_adaptive_mult_high: 12,
            gpu_worldgen_adaptive_batch_min: 16,
            gpu_worldgen_adaptive_batch_max: 768,
            controls: ControlsConfig::default(),
            terrain_mode: crate::voxel::WorldTerrainMode::Planet,
        }
    }
}

impl ClientSettings {
    pub fn sanitize(&mut self) {
        self.fov = self.fov.clamp(10.0, 170.0);
        self.hud_padding = self.hud_padding.clamp(0.0, 128.0);
        self.touch_menu_tile_overlay_strength = self.touch_menu_tile_overlay_strength.clamp(0.0, 0.9);
        self.language = crate::client::l10n::normalize_language(&self.language).to_string();

        self.touch_tile_style_window_alpha = self.touch_tile_style_window_alpha.clamp(0.0, 1.0);

        let s = &mut self.main_menu_tile_style;
        s.main_tile_target_w = s.main_tile_target_w.clamp(200.0, 640.0);
        s.main_tile_min_w_single = s.main_tile_min_w_single.clamp(160.0, 640.0).min(s.main_tile_target_w);
        s.main_tile_gap_x = s.main_tile_gap_x.clamp(0.0, 48.0);
        s.main_tile_gap_y = s.main_tile_gap_y.clamp(0.0, 48.0);
        s.main_tile_h_wide = s.main_tile_h_wide.clamp(100.0, 320.0);
        s.main_tile_h_med = s.main_tile_h_med.clamp(80.0, 320.0).min(s.main_tile_h_wide);
        s.main_tile_h_narrow = s.main_tile_h_narrow.clamp(72.0, 320.0).min(s.main_tile_h_med);
        s.main_tile_wide_threshold = s.main_tile_wide_threshold.clamp(200.0, 640.0);
        s.main_tile_med_threshold = s.main_tile_med_threshold.clamp(160.0, 640.0).min(s.main_tile_wide_threshold);
        s.main_tile_pad_x_ratio = s.main_tile_pad_x_ratio.clamp(0.0, 0.2);
        s.main_tile_pad_y_ratio = s.main_tile_pad_y_ratio.clamp(0.0, 0.2);
        s.main_tile_title_size = s.main_tile_title_size.clamp(12.0, 48.0);
        s.main_tile_subtitle_size = s.main_tile_subtitle_size.clamp(10.0, 36.0).min(s.main_tile_title_size);
        s.main_tile_icon_br_size = s.main_tile_icon_br_size.clamp(16.0, 96.0);
        s.main_tile_icon_bl_size = s.main_tile_icon_bl_size.clamp(16.0, 96.0);
        s.small_tile_max_w = s.small_tile_max_w.clamp(120.0, 320.0);
        s.small_tile_h = s.small_tile_h.clamp(40.0, 120.0);
        s.small_tile_gap_x = s.small_tile_gap_x.clamp(0.0, 24.0);
        s.small_tile_gap_y = s.small_tile_gap_y.clamp(0.0, 24.0);
        s.small_tile_icon_size = s.small_tile_icon_size.clamp(16.0, 64.0);
        s.small_tile_icon_margin = s.small_tile_icon_margin.clamp(0.0, 24.0);

        self.chunks_load_distance.x = self.chunks_load_distance.x.max(2);
        self.chunks_load_distance.y = self.chunks_load_distance.y.max(1);

        self.gpu_worldgen_batch_size = self.gpu_worldgen_batch_size.max(1);
        self.gpu_worldgen_max_loading = self.gpu_worldgen_max_loading.max(1);
        self.cpu_worldgen_max_loading = self.cpu_worldgen_max_loading.max(1);
        self.gpu_worldgen_adaptive_backlog_mid = self.gpu_worldgen_adaptive_backlog_mid.max(1);
        self.gpu_worldgen_adaptive_backlog_high = self.gpu_worldgen_adaptive_backlog_high.max(self.gpu_worldgen_adaptive_backlog_mid);
        self.gpu_worldgen_adaptive_mult_low = self.gpu_worldgen_adaptive_mult_low.max(1);
        self.gpu_worldgen_adaptive_mult_mid = self.gpu_worldgen_adaptive_mult_mid.max(self.gpu_worldgen_adaptive_mult_low);
        self.gpu_worldgen_adaptive_mult_high = self.gpu_worldgen_adaptive_mult_high.max(self.gpu_worldgen_adaptive_mult_mid);
        self.gpu_worldgen_adaptive_batch_min = self.gpu_worldgen_adaptive_batch_min.max(1);
        self.gpu_worldgen_adaptive_batch_max = self.gpu_worldgen_adaptive_batch_max.max(self.gpu_worldgen_adaptive_batch_min);
        // Clamp interactive resize debounce to reasonable range
        if self.interactive_resize_debounce_frames > 600 {
            self.interactive_resize_debounce_frames = 600;
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Reflect)]
#[serde(default)]
#[reflect(Default)]
pub struct TouchTileStyle {
    pub background_mode: TileBackgroundMode,
    pub corner_radius: f32,
    pub icon_scale: f32,
    pub preload_rasterized: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq, Reflect)]
pub enum TileBackgroundMode {
    Cover,
    Contain,
}

impl Default for TouchTileStyle {
    fn default() -> Self {
        Self {
            background_mode: TileBackgroundMode::Cover,
            corner_radius: 6.0,
            icon_scale: 1.0,
            preload_rasterized: true,
        }
    }
}

impl Default for TileBackgroundMode {
    fn default() -> Self {
        TileBackgroundMode::Cover
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct ServerListItem {
    pub name: String,
    pub addr: String,

    #[serde(skip)]
    pub ui: crate::ui::serverlist::UiServerInfo,
}
