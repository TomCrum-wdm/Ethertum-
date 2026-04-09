use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Color32, Layout, Ui, Widget},
    EguiContexts,
};

use super::{new_egui_window, sfx_play, ui_lr_panel};
use crate::client::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingTag {
    Performance,
    Fun,
    Dangerous,
}

impl SettingTag {
    fn color(self) -> Color32 {
        match self {
            SettingTag::Performance => Color32::from_rgb(70, 140, 255),
            SettingTag::Fun => Color32::from_rgb(180, 90, 255),
            SettingTag::Dangerous => Color32::from_rgb(255, 70, 70),
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingTag::Performance => "Performance",
            SettingTag::Fun => "Fun",
            SettingTag::Dangerous => "Dangerous",
        }
    }
}

#[derive(Default)]
struct TagScore {
    perf: i32,
    fun: i32,
    danger: i32,
}

fn manual_setting_tags(label: &str) -> Option<Vec<SettingTag>> {
    match label {
        "Default Terrain For New Worlds" => Some(vec![SettingTag::Fun, SettingTag::Dangerous]),
        "Surface-Only (No Full Upgrade)" => Some(vec![SettingTag::Performance, SettingTag::Dangerous]),
        "GPU WorldGen" => Some(vec![SettingTag::Performance, SettingTag::Fun, SettingTag::Dangerous]),
        "Allow GPU On Persisted Worlds" => Some(vec![SettingTag::Performance, SettingTag::Dangerous]),
        "Reset Recommended WorldGen Values" => Some(vec![SettingTag::Dangerous]),
        "Layout Edit Mode" => Some(vec![SettingTag::Fun, SettingTag::Dangerous]),
        "Export + Copy" => Some(vec![SettingTag::Fun, SettingTag::Dangerous]),
        "Import From Text" => Some(vec![SettingTag::Fun, SettingTag::Dangerous]),
        _ => None,
    }
}

fn score_keywords(s: &str) -> TagScore {
    let mut score = TagScore::default();

    let perf_kw = [
        "gpu", "cpu", "batch", "backlog", "window", "distance", "vsync", "fxaa", "tonemapping", "bloom", "ssr",
        "fog", "shadow", "quality", "dead zone", "sensitivity", "concurrency", "scale", "render", "illumina",
    ];
    let fun_kw = [
        "planet", "flat", "terrain", "fov", "touch", "day time", "indicator", "brush", "tex", "size", "intensity",
        "jump", "sprint", "sneak", "skybox", "ui", "preset", "name", "username",
    ];
    let danger_kw = [
        "experimental", "persisted", "surface-only", "reset", "import", "delete", "undo", "layout edit", "share", "copy",
        "worldgen", "adaptive", "multiplier", "spawn", "gravity",
    ];

    for kw in perf_kw {
        if s.contains(kw) {
            score.perf += 2;
        }
    }
    for kw in fun_kw {
        if s.contains(kw) {
            score.fun += 2;
        }
    }
    for kw in danger_kw {
        if s.contains(kw) {
            score.danger += 2;
        }
    }

    if s.contains("adaptive") || s.contains("batch") || s.contains("window") {
        score.perf += 2;
        score.danger += 1;
    }
    if s.contains("planet") || s.contains("gravity") || s.contains("terrain") {
        score.fun += 2;
        score.danger += 1;
    }

    score
}

fn classify_setting_tags(label: &str) -> Vec<SettingTag> {
    if let Some(tags) = manual_setting_tags(label) {
        return tags;
    }

    let s = label.to_ascii_lowercase();
    let score = score_keywords(&s);
    let max_score = score.perf.max(score.fun).max(score.danger);

    let mut tags = Vec::new();
    if score.perf >= 2 && (score.perf >= max_score - 1) {
        tags.push(SettingTag::Performance);
    }
    if score.fun >= 2 && (score.fun >= max_score - 1) {
        tags.push(SettingTag::Fun);
    }
    if score.danger >= 2 && (score.danger >= max_score - 1) {
        tags.push(SettingTag::Dangerous);
    }

    if tags.is_empty() {
        tags.push(SettingTag::Performance);
    }
    tags
}

fn draw_tag_strips(ui: &mut Ui, tags: &[SettingTag]) {
    for (i, tag) in tags.iter().enumerate() {
        let (strip_rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 22.0), egui::Sense::hover());
        ui.painter().rect_filled(strip_rect, 1.0, tag.color());
        if i + 1 < tags.len() {
            ui.add_space(2.0);
        }
    }
}

fn ui_setting_legend(ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Legend:");
        for tag in [SettingTag::Performance, SettingTag::Fun, SettingTag::Dangerous] {
            ui.colored_label(tag.color(), format!("| {}", tag.label()));
        }
    });
    ui.small("One option may have multiple tags. Multiple colored bars mean mixed traits.");
    ui.small("Dangerous options can cause compatibility/perf issues and should be changed carefully.");
}

#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum SettingsPanel {
    #[default]
    General,
    CurrentWorld,
    Graphics,
    Audio,
    Controls,
    Language,
    Mods,
    Assets,
    // Credits,
}

pub fn ui_setting_line(ui: &mut Ui, text: &str, widget: impl Widget) {
    let tags = classify_setting_tags(text);
    ui.horizontal(|ui| {
        draw_tag_strips(ui, &tags);
        ui.add_space(12.);
        ui.colored_label(Color32::WHITE, text);
        let end_width = 150.;
        let end_margin = 8.;
        let line_margin = 10.;

        let p = ui.cursor().left_center() + egui::Vec2::new(line_margin, 0.);
        let p2 = egui::pos2(p.x + ui.available_width() - end_width - line_margin * 2. - end_margin, p.y);
        ui.painter().line_segment([p, p2], ui.visuals().widgets.noninteractive.bg_stroke);

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(end_margin);
            ui.add_sized([end_width, 22.], widget);
        });
    });
}

pub fn ui_setting_line_custom(ui: &mut Ui, text: &str, add_widget: impl FnOnce(&mut Ui)) {
    let tags = classify_setting_tags(text);
    ui.horizontal(|ui| {
        draw_tag_strips(ui, &tags);
        ui.add_space(12.);
        ui.colored_label(Color32::WHITE, text);
        let end_margin = 8.;
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(end_margin);
            add_widget(ui);
        });
    });
}

pub fn ui_settings(
    mut ctx: EguiContexts,
    mut settings_panel: Local<SettingsPanel>,

    mut cli: ResMut<ClientInfo>,
    mut cfg: ResMut<ClientSettings>,
    mut worldinfo: Option<ResMut<WorldInfo>>,
    //mut egui_settings: ResMut<EguiSettings>,
    mut query_char: Query<&mut CharacterController>,
    // chunk_sys: Option<ResMut<ClientChunkSystem>>,
    mut vox_brush: ResMut<crate::voxel::VoxelBrush>,
    items: Res<crate::item::Items>,
    // mut global_volume: ResMut<GlobalVolume>,

    // mut cmds: Commands,
    // asset_server: Res<AssetServer>,
    // mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let is_world_loaded = worldinfo.is_some();
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    new_egui_window("Settings").show(ctx_mut, |ui| {
        let curr_settings_panel = *settings_panel;

        ui_lr_panel(
            ui,
            true,
            |ui| {
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::General, "General"));
                if is_world_loaded {
                    sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::CurrentWorld, "Current World"));
                }
                ui.separator();
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Graphics, "Graphics"));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Audio, "Audio"));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Controls, "Controls"));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Language, "Languages"));
                ui.separator();
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Mods, "Mods"));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Assets, "Assets"));
            },
            |ui| {
                ui.style_mut().spacing.item_spacing.y = 12.;

                ui.add_space(16.);
                ui_setting_legend(ui);
                ui.separator();

                match curr_settings_panel {
                    SettingsPanel::General => {
                        ui.label("Profile");
                        ui_setting_line(ui, "Username", egui::TextEdit::singleline(&mut cfg.username));
                        ui_setting_line(ui, "Touch UI (large buttons)", egui::Checkbox::new(&mut cfg.touch_ui, ""));

                        ui.separator();
                        ui.label("World Streaming (Basic)");
                        ui_setting_line(ui, "Chunk Load Distance X", egui::Slider::new(&mut cfg.chunks_load_distance.x, 2..=64));
                        ui_setting_line(ui, "Chunk Load Distance Y", egui::Slider::new(&mut cfg.chunks_load_distance.y, 1..=32));
                        ui_setting_line(ui, "Surface-First Meshing", egui::Checkbox::new(&mut cfg.surface_first_meshing, ""));
                        ui_setting_line(ui, "Surface-Only (No Full Upgrade)", egui::Checkbox::new(&mut cfg.surface_only_meshing, ""));
                        ui_setting_line(ui, "GPU WorldGen", egui::Checkbox::new(&mut cfg.gpu_worldgen, ""));
                        ui_setting_line(
                            ui,
                            "Allow GPU On Persisted Worlds",
                            egui::Checkbox::new(&mut cfg.gpu_worldgen_allow_persisted_world, ""),
                        );

                        ui_setting_line_custom(ui, "Default Terrain For New Worlds", |ui| {
                            let mode = &mut cfg.terrain_mode;
                            let planet = *mode == crate::voxel::WorldTerrainMode::Planet;
                            let flat = *mode == crate::voxel::WorldTerrainMode::Flat;
                            let superflat = *mode == crate::voxel::WorldTerrainMode::SuperFlat;
                            if ui.radio(planet, "Spherical Planet").clicked() {
                                *mode = crate::voxel::WorldTerrainMode::Planet;
                            }
                            if ui.radio(flat, "Flat World").clicked() {
                                *mode = crate::voxel::WorldTerrainMode::Flat;
                            }
                            if ui.radio(superflat, "SuperFlat World").clicked() {
                                *mode = crate::voxel::WorldTerrainMode::SuperFlat;
                            }
                        });

                        ui_setting_line_custom(ui, "Reset Recommended WorldGen Values", |ui| {
                            if ui.button("Reset").clicked() {
                                cfg.surface_first_meshing = true;
                                cfg.surface_only_meshing = false;
                                cfg.gpu_worldgen = true;
                                cfg.gpu_worldgen_allow_persisted_world = false;
                                cfg.gpu_worldgen_batch_size = 16;
                                cfg.gpu_worldgen_max_loading = 256;
                                cfg.cpu_worldgen_max_loading = 8;
                                cfg.gpu_worldgen_adaptive_backlog_mid = 24;
                                cfg.gpu_worldgen_adaptive_backlog_high = 64;
                                cfg.gpu_worldgen_adaptive_mult_low = 2;
                                cfg.gpu_worldgen_adaptive_mult_mid = 4;
                                cfg.gpu_worldgen_adaptive_mult_high = 12;
                                cfg.gpu_worldgen_adaptive_batch_min = 16;
                                cfg.gpu_worldgen_adaptive_batch_max = 768;
                            }
                        });

                        egui::CollapsingHeader::new("Advanced GPU WorldGen Tuning")
                            .default_open(false)
                            .show(ui, |ui| {
                                ui_setting_line(ui, "GPU WorldGen Batch Size", egui::Slider::new(&mut cfg.gpu_worldgen_batch_size, 1..=128));
                                ui_setting_line(ui, "GPU Max Loading Window", egui::Slider::new(&mut cfg.gpu_worldgen_max_loading, 16..=1024));
                                ui_setting_line(ui, "CPU Max Loading Window", egui::Slider::new(&mut cfg.cpu_worldgen_max_loading, 1..=64));
                                ui_setting_line(
                                    ui,
                                    "Adaptive Backlog Mid",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_backlog_mid, 1..=1024),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Backlog High",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_backlog_high, 1..=2048),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Multiplier Low",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_low, 1..=16),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Multiplier Mid",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_mid, 1..=32),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Multiplier High",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_high, 1..=64),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Batch Min",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_batch_min, 1..=512),
                                );
                                ui_setting_line(
                                    ui,
                                    "Adaptive Batch Max",
                                    egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_batch_max, 1..=2048),
                                );
                                ui.small("Higher backlog usually means larger GPU batch and wider loading windows.");
                            });

                        ui.separator();
                        ui.label("Video");
                        ui_setting_line(ui, "FOV", egui::Slider::new(&mut cfg.fov, 10.0..=170.0));
                        ui_setting_line(ui, "VSync", egui::Checkbox::new(&mut cfg.vsync, ""));

                        ui.separator();
                        ui.label("UI");
                        ui_setting_line(ui, "HUD Padding", egui::Slider::new(&mut cfg.hud_padding, 0.0..=48.0));
                        ui_setting_line(ui, "Show Level Indicator", egui::Checkbox::new(&mut cfg.show_level_indicator, ""));
                        ui_setting_line(ui, "Show Pitch Indicator", egui::Checkbox::new(&mut cfg.show_pitch_indicator, ""));
                    }
                    SettingsPanel::CurrentWorld => {
                        ui.label("World");
                        if let Some(worldinfo) = &mut worldinfo {
                            ui_setting_line(ui, "Day Time", egui::Slider::new(&mut worldinfo.daytime, 0.0..=1.0));
                            ui_setting_line(ui, "Day Time Length", egui::Slider::new(&mut worldinfo.daytime_length, 0.0..=60.0 * 24.0));
                        }

                        ui.separator();
                        ui.label("Voxel Brush");
                        ui_setting_line(ui, "Size", egui::Slider::new(&mut vox_brush.size, 0.0..=20.0));
                        ui_setting_line(ui, "Intensity", egui::Slider::new(&mut vox_brush.strength, 0.0..=1.0));
                        ui_setting_line(ui, "Tex", egui::Slider::new(&mut vox_brush.tex, 0..=25));

                        ui.separator();
                        ui.label("Character");
                        if let Ok(mut ctl) = query_char.single_mut() {
                            ui_setting_line(ui, "Unfly on Grounded", egui::Checkbox::new(&mut ctl.unfly_on_ground, ""));
                        }

                        egui::CollapsingHeader::new("Item Physics Snapshot")
                            .default_open(false)
                            .show(ui, |ui| {
                                if let Some(def) = items.defs.get(0) {
                                    ui.label(format!("Item: {}", def.name));
                                    ui.label(format!("Mass: {:.3} kg", def.props.mass));
                                    ui.label(format!("Volume: {:.5} m³", def.props.volume));
                                    ui.label(format!("Density: {:.1} kg/m³", def.props.density));
                                    ui.label(format!("Molar Mass: {:.2} g/mol", def.props.molar_mass));
                                } else {
                                    ui.small("No item definitions loaded.");
                                }
                            });
                    }
                    SettingsPanel::Graphics => {
                        ui.label("Render Effects");

                        ui_setting_line(ui, "FXAA", egui::Checkbox::new(&mut cli.render_fxaa, ""));
                        ui_setting_line(ui, "Tonemapping", egui::Checkbox::new(&mut cli.render_tonemapping, ""));
                        ui_setting_line(ui, "Bloom", egui::Checkbox::new(&mut cli.render_bloom, ""));
                        ui_setting_line(ui, "Screen Space Reflections", egui::Checkbox::new(&mut cli.render_ssr, ""));
                        ui_setting_line(ui, "Volumetric Fog", egui::Checkbox::new(&mut cli.render_volumetric_fog, ""));
                        ui_setting_line(ui, "Volumetric Fog Density", egui::Slider::new(&mut cli.volumetric_fog_density, 0.0..=3.0));
                        ui_setting_line(ui, "Volumetric Fog Color R", egui::Slider::new(&mut cli.volumetric_fog_color.x, 0.0..=1.0));
                        ui_setting_line(ui, "Volumetric Fog Color G", egui::Slider::new(&mut cli.volumetric_fog_color.y, 0.0..=1.0));
                        ui_setting_line(ui, "Volumetric Fog Color B", egui::Slider::new(&mut cli.volumetric_fog_color.z, 0.0..=1.0));
                        ui_setting_line(ui, "Skybox + EnvMap", egui::Checkbox::new(&mut cli.render_skybox, ""));

                        ui.label("Lighting");
                        ui_setting_line(ui, "Skylight Shadow", egui::Checkbox::new(&mut cli.skylight_shadow, ""));
                        ui_setting_line(ui, "Skylight Illuminance", egui::Slider::new(&mut cli.skylight_illuminance, 0.1..=200.0));

                        ui.label("Quality Profile");
                        ui_setting_line(ui, "High Quality Rendering", egui::Checkbox::new(&mut cfg.high_quality_rendering, ""));
                    }
                    SettingsPanel::Audio => {

                        // ui_setting_line(ui, "Global Volume", egui::Slider::new(&mut global_volume.volume as &mut f32, 0.0..=1.0));
                    }
                    SettingsPanel::Controls => {
                        ui.label("Input Schemes");
                        ui_setting_line(ui, "Touch UI (large buttons)", egui::Checkbox::new(&mut cfg.touch_ui, ""));

                        ui.separator();
                        ui.label("Keyboard + Mouse");
                        ui_setting_line(
                            ui,
                            "Look Sensitivity",
                            egui::Slider::new(&mut cfg.controls.keyboard_mouse.look_sensitivity, 0.1..=4.0),
                        );
                        ui_setting_line(ui, "Invert Y", egui::Checkbox::new(&mut cfg.controls.keyboard_mouse.invert_y, ""));
                        ui_setting_line(ui, "Jump Key", egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_jump));
                        ui_setting_line(ui, "Sprint Key", egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_sprint));
                        ui_setting_line(ui, "Sneak Key", egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_sneak));
                        ui_setting_line(ui, "Pause Key", egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_pause));

                        ui.separator();
                        ui.label("Gamepad");
                        ui_setting_line(
                            ui,
                            "Look Sensitivity",
                            egui::Slider::new(&mut cfg.controls.gamepad.look_sensitivity, 0.1..=4.0),
                        );
                        ui_setting_line(ui, "Invert Y", egui::Checkbox::new(&mut cfg.controls.gamepad.invert_y, ""));
                        ui_setting_line(
                            ui,
                            "Left Stick Dead Zone",
                            egui::Slider::new(&mut cfg.controls.gamepad.left_stick_dead_zone, 0.0..=0.5),
                        );
                        ui_setting_line(
                            ui,
                            "Right Stick Dead Zone",
                            egui::Slider::new(&mut cfg.controls.gamepad.right_stick_dead_zone, 0.0..=0.5),
                        );
                        ui_setting_line(ui, "Jump Button", egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_jump));
                        ui_setting_line(ui, "Sprint Button", egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_sprint));
                        ui_setting_line(ui, "Use Button", egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_use));
                        ui_setting_line(ui, "Attack Button", egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_attack));

                        ui.separator();
                        ui.label("Touch");
                        ui_setting_line(ui, "Layout Edit Mode", egui::Checkbox::new(&mut cli.touch_controls_edit_mode, ""));
                        ui_setting_line_custom(ui, "Undo Last Drag", |ui| {
                            if ui.button("Undo").clicked() {
                                cfg.controls.touch_layout_request_undo = true;
                            }
                        });
                        if cli.touch_controls_edit_mode {
                            ui.colored_label(
                                Color32::from_rgb(255, 214, 140),
                                "Designer Active: drag joystick and buttons on the overlay. Gameplay touch input is locked.",
                            );
                        } else {
                            ui.colored_label(
                                Color32::from_gray(170),
                                "Enable Layout Edit Mode to open the visual touch UI designer.",
                            );
                        }
                        ui_setting_line(
                            ui,
                            "Move Stick Radius",
                            egui::Slider::new(&mut cfg.controls.touch.move_stick_radius, 48.0..=200.0),
                        );
                        ui_setting_line(
                            ui,
                            "Move Dead Zone",
                            egui::Slider::new(&mut cfg.controls.touch.move_dead_zone, 0.0..=0.5),
                        );
                        ui.colored_label(
                            Color32::from_gray(180),
                            "Tip: push the move stick to the top edge to lock sprint; pull down to release.",
                        );
                        ui_setting_line(
                            ui,
                            "Button Radius",
                            egui::Slider::new(&mut cfg.controls.touch.button_radius, 30.0..=80.0),
                        );
                        ui_setting_line(
                            ui,
                            "Vertical Slider Height",
                            egui::Slider::new(&mut cfg.controls.touch.vertical_slider_height, 120.0..=320.0),
                        );
                        ui_setting_line(
                            ui,
                            "Vertical Slider Width",
                            egui::Slider::new(&mut cfg.controls.touch.vertical_slider_width, 44.0..=96.0),
                        );
                        ui_setting_line(
                            ui,
                            "Fly Double Tap Window (sec)",
                            egui::Slider::new(&mut cfg.controls.touch.fly_double_tap_window_secs, 0.18..=0.65),
                        );

                        ui.separator();
                        ui.label("Touch Button Action Mapping");
                        ui_setting_line_custom(ui, "Attack Button Action", |ui| {
                            egui::ComboBox::from_id_source("touch_attack_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.attack_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Attack, "Attack");
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::UseItem, "UseItem");
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Jump, "Jump");
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sprint, "Sprint");
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sneak, "Sneak");
                                });
                        });
                        ui_setting_line_custom(ui, "Use Button Action", |ui| {
                            egui::ComboBox::from_id_source("touch_use_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.use_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Attack, "Attack");
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::UseItem, "UseItem");
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Jump, "Jump");
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sprint, "Sprint");
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sneak, "Sneak");
                                });
                        });
                        ui_setting_line_custom(ui, "Jump Button Action", |ui| {
                            egui::ComboBox::from_id_source("touch_jump_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.jump_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Attack, "Attack");
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::UseItem, "UseItem");
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Jump, "Jump");
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sprint, "Sprint");
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sneak, "Sneak");
                                });
                        });
                        ui_setting_line_custom(ui, "Sprint Button Action", |ui| {
                            egui::ComboBox::from_id_source("touch_sprint_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.sprint_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Attack, "Attack");
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::UseItem, "UseItem");
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Jump, "Jump");
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sprint, "Sprint");
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sneak, "Sneak");
                                });
                        });
                        ui_setting_line_custom(ui, "Crouch Button Action", |ui| {
                            egui::ComboBox::from_id_source("touch_crouch_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.crouch_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Attack, "Attack");
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::UseItem, "UseItem");
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Jump, "Jump");
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sprint, "Sprint");
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sneak, "Sneak");
                                });
                        });

                        ui_setting_line_custom(ui, "Reset Touch Layout", |ui| {
                            if ui.button("Reset").clicked() {
                                cfg.controls.touch = Default::default();
                                cli.touch_controls_edit_mode = false;
                            }
                        });

                        ui.separator();
                        ui.label("Touch Layout Presets");
                        ui_setting_line(
                            ui,
                            "Preset Name",
                            egui::TextEdit::singleline(&mut cfg.controls.touch_layout_preset_name),
                        );
                        ui_setting_line_custom(ui, "Save Current Layout As Preset", |ui| {
                            if ui.button("Save").clicked() {
                                let mut name = cfg.controls.touch_layout_preset_name.trim().to_string();
                                let current_layout = cfg.controls.touch.clone();
                                if name.is_empty() {
                                    name = format!("Preset {}", cfg.controls.touch_layout_presets.len() + 1);
                                }
                                if let Some(existing) = cfg.controls.touch_layout_presets.iter_mut().find(|p| p.name == name) {
                                    existing.layout = current_layout;
                                } else {
                                    cfg.controls.touch_layout_presets.push(crate::client::settings::TouchLayoutPreset {
                                        name,
                                        layout: current_layout,
                                    });
                                }
                            }
                        });

                        let mut remove_idx: Option<usize> = None;
                        let preset_rows = cfg
                            .controls
                            .touch_layout_presets
                            .iter()
                            .enumerate()
                            .map(|(i, p)| (i, p.name.clone(), p.layout.clone()))
                            .collect::<Vec<_>>();
                        for (i, preset_name, preset_layout) in preset_rows {
                            ui.horizontal(|ui| {
                                if ui.button(format!("Load: {}", preset_name)).clicked() {
                                    cfg.controls.touch = preset_layout.clone();
                                    cli.touch_controls_edit_mode = true;
                                }
                                if ui.button("Delete").clicked() {
                                    remove_idx = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove_idx {
                            cfg.controls.touch_layout_presets.remove(i);
                        }

                        ui.separator();
                        ui.label("Share Touch Layout");
                        ui.add_sized(
                            [ui.available_width(), 66.0],
                            egui::TextEdit::multiline(&mut cfg.controls.touch_layout_share_text)
                                .hint_text("Layout JSON for sharing/import"),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Export + Copy").clicked() {
                                if let Ok(text) = serde_json::to_string(&cfg.controls.touch) {
                                    cfg.controls.touch_layout_share_text = text;
                                    ui.ctx().copy_text(cfg.controls.touch_layout_share_text.clone());
                                }
                            }
                            if ui.button("Import From Text").clicked() {
                                if let Ok(layout) = serde_json::from_str::<crate::client::settings::TouchControlsConfig>(&cfg.controls.touch_layout_share_text) {
                                    cfg.controls.touch = layout;
                                    cli.touch_controls_edit_mode = true;
                                }
                            }
                        });
                    }
                    SettingsPanel::Language => {}
                    SettingsPanel::Mods => {}
                    _ => (),
                }
            },
        );
    });
}
