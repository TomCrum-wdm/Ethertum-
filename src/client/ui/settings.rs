use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Color32, Layout, Ui, Widget},
    EguiContexts,
};

use super::{new_egui_window, sfx_play, ui_lr_panel};
use crate::client::prelude::*;

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


pub fn ui_setting_line(ui: &mut Ui, text: impl Into<egui::RichText>, widget: impl Widget) {
    ui_setting_line_colored(ui, text, widget, Color32::WHITE);
}

pub fn ui_setting_line_colored(ui: &mut Ui, text: impl Into<egui::RichText>, widget: impl Widget, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(20.);
        ui.colored_label(color, text);
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

pub fn ui_settings(
    mut ctx: EguiContexts,
    mut settings_panel: Local<SettingsPanel>,

    mut cli: ResMut<ClientInfo>,
    mut cfg: ResMut<ClientSettings>,
    mut worldinfo: Option<ResMut<WorldInfo>>,
    mut cmds: Commands,
    mut chunk_sys: Option<ResMut<crate::voxel::ClientChunkSystem>>,
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

                match curr_settings_panel {
                    SettingsPanel::General => {
                        // 蓝色分组
                        let blue = Color32::from_rgb(80, 160, 255);
                        let rainbow = Color32::from_rgb(255, 120, 0); // 彩虹色可后续细化
                        let _danger = Color32::RED;

                        ui.colored_label(blue, "Profile: ");
                        ui_setting_line_colored(ui, "Username", egui::TextEdit::singleline(&mut cfg.username), blue);
                        ui_setting_line_colored(ui, "Touch UI (large buttons)", egui::Checkbox::new(&mut cfg.touch_ui, ""), blue);

                        ui.add_space(12.);
                        if ui.add(egui::Button::new("编辑器模式 / 上帝模式").fill(rainbow)).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/bevyengine/bevy_editor_pls"));
                        }
                        if ui.add(egui::Button::new("电路板实验室").fill(rainbow)).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/Dreamtowards/Ethertum/tree/main/assets/test/comp/circuit"));
                        }

                        ui.colored_label(blue, "Voxel:");
                        ui_setting_line_colored(ui, "Chunk Load Distance X", egui::Slider::new(&mut cfg.chunks_load_distance.x, -1..=25), blue);
                        ui_setting_line_colored(ui, "Chunk Load Distance Y", egui::Slider::new(&mut cfg.chunks_load_distance.y, -1..=25), blue);

                        ui.colored_label(blue, "地形模式:");
                        // Detect mode change so we can clear chunks and force regen when switching between Planet/Flat
                        let prev_mode = cfg.terrain_mode;
                        ui.horizontal(|ui| {
                            let mode = &mut cfg.terrain_mode;
                            let planet = *mode == crate::client::settings::TerrainMode::Planet;
                            let flat = *mode == crate::client::settings::TerrainMode::Flat;
                            if ui.radio(planet, "星球（球体）").clicked() {
                                *mode = crate::client::settings::TerrainMode::Planet;
                            }
                            if ui.radio(flat, "平面").clicked() {
                                *mode = crate::client::settings::TerrainMode::Flat;
                            }
                        });
                        // Sync setting into live WorldInfo if world is loaded
                        if let Some(w) = &mut worldinfo {
                            w.terrain_mode = cfg.terrain_mode;
                            w.planet_center = Vec3::new(cfg.planet_center[0], cfg.planet_center[1], cfg.planet_center[2]);
                            w.planet_radius = cfg.planet_radius;
                            w.planet_shell_thickness = cfg.planet_shell_thickness;
                            w.gravity_accel = cfg.gravity_accel;
                        }
                        // If mode changed, clear existing client chunks so they will be regenerated under new mode
                        if prev_mode != cfg.terrain_mode {
                            if let Some(cs_mut) = chunk_sys.as_mut() {
                                let keys: Vec<_> = cs_mut.chunks.keys().cloned().collect();
                                for cp in keys {
                                    if let Some(chunkptr) = cs_mut.despawn_chunk(cp, &mut cmds) {
                                            if let Some(w) = &worldinfo {
                                                let guard = crate::util::lock_arc(&chunkptr);
                                                let _ = crate::voxel::chunk_storage::spawn_save_chunk_from_chunk(&*guard, Some(w.name.clone()), w.seed);
                                            } else {
                                                let guard = crate::util::lock_arc(&chunkptr);
                                                let _ = crate::voxel::chunk_storage::spawn_save_chunk_from_chunk(&*guard, None, 0);
                                            }
                                    }
                                }
                            }
                        }
                        if cfg.terrain_mode == crate::client::settings::TerrainMode::Planet {
                            ui.add_space(6.);
                            ui.colored_label(Color32::from_rgb(200, 220, 255), "自定义星球参数:");
                            ui_setting_line_colored(ui, "中心 X", egui::DragValue::new(&mut cfg.planet_center[0]).speed(1.0), Color32::from_rgb(200, 220, 255));
                            ui_setting_line_colored(ui, "中心 Y", egui::DragValue::new(&mut cfg.planet_center[1]).speed(1.0), Color32::from_rgb(200, 220, 255));
                            ui_setting_line_colored(ui, "中心 Z", egui::DragValue::new(&mut cfg.planet_center[2]).speed(1.0), Color32::from_rgb(200, 220, 255));
                            ui_setting_line_colored(ui, "半径", egui::DragValue::new(&mut cfg.planet_radius).speed(1.0), Color32::from_rgb(200, 220, 255));
                            ui_setting_line_colored(ui, "壳厚度", egui::DragValue::new(&mut cfg.planet_shell_thickness).speed(1.0), Color32::from_rgb(200, 220, 255));
                            ui_setting_line_colored(ui, "重力 (m/s²)", egui::DragValue::new(&mut cfg.gravity_accel).speed(0.1), Color32::from_rgb(200, 220, 255));
                            // Also apply to live WorldInfo
                            if let Some(w) = &mut worldinfo {
                                w.planet_center = Vec3::new(cfg.planet_center[0], cfg.planet_center[1], cfg.planet_center[2]);
                                w.planet_radius = cfg.planet_radius;
                                w.planet_shell_thickness = cfg.planet_shell_thickness;
                                w.gravity_accel = cfg.gravity_accel;
                            }
                        }
                        ui.separator();
                        ui.colored_label(blue, "Voxel Brush:");
                        ui_setting_line_colored(ui, "Size", egui::Slider::new(&mut vox_brush.size, 0.0..=20.0), blue);
                        ui_setting_line_colored(ui, "Indensity", egui::Slider::new(&mut vox_brush.strength, 0.0..=1.0), blue);
                        ui_setting_line_colored(ui, "Tex", egui::Slider::new(&mut vox_brush.tex, 0..=25), blue);

                        if let Some(def) = items.defs.first() {
                            ui.separator();
                            ui.colored_label(rainbow, format!("物品: {}", def.name));
                            ui.label(format!("质量: {:.3} kg", def.props.mass));
                            ui.label(format!("体积: {:.5} m³", def.props.volume));
                            ui.label(format!("密度: {:.1} kg/m³", def.props.density));
                            ui.label(format!("摩尔质量: {:.2} g/mol", def.props.molar_mass));
                        }

                        if let Some(worldinfo) = &mut worldinfo {
                            ui.colored_label(blue, "World:");
                            ui_setting_line_colored(ui, "Day Time", egui::Slider::new(&mut worldinfo.daytime, 0.0..=1.0), blue);
                            ui_setting_line_colored(ui, "Day Time Length", egui::Slider::new(&mut worldinfo.daytime_length, 0.0..=60.0 * 24.0), blue);
                        }

                        ui.colored_label(blue, "Video:");
                        ui_setting_line_colored(ui, "FOV", egui::Slider::new(&mut cfg.fov, 10.0..=170.0), blue);
                        ui_setting_line_colored(ui, "VSync", egui::Checkbox::new(&mut cfg.vsync, ""), blue);

                        ui.colored_label(blue, "UI");
                        ui_setting_line_colored(ui, "HUD Padding", egui::Slider::new(&mut cfg.hud_padding, 0.0..=48.0), blue);

                        ui.colored_label(blue, "Controls");
                        if let Ok(mut ctl) = query_char.single_mut() {
                            ui_setting_line_colored(ui, "Unfly on Grounded", egui::Checkbox::new(&mut ctl.unfly_on_ground, ""), blue);
                        }
                    }
                    SettingsPanel::CurrentWorld => {
                        ui.colored_label(Color32::from_rgb(200, 220, 255), "当前存档:");
                        if let Some(w) = &worldinfo {
                            ui.label(format!("Name: {}", w.name));
                            ui.label(format!("Seed: {:016x}", w.seed));
                            ui.add_space(6.);
                            if ui.add(egui::Button::new("导出存档 (Zip)" )).clicked() {
                                // Run export in background to avoid blocking the UI/main thread.
                                let world_name = w.name.clone();
                                let seed = w.seed;
                                let handle = crate::voxel::chunk_storage::spawn_export_world_save(Some(world_name.clone()), seed);
                                // Detach: spawn a watcher thread that joins and logs result when ready.
                                std::thread::spawn(move || {
                                    match handle.join() {
                                        Ok(Some(path)) => info!("Exported world to {:?}", path),
                                        Ok(None) => warn!("Export failed for world {}", world_name),
                                        Err(e) => warn!("Export thread panicked: {:?}", e),
                                    }
                                });
                            }
                        } else {
                            ui.label("未加载世界");
                        }
                    }
                    SettingsPanel::Graphics => {
                        ui.label("Render Effects");

                        ui_setting_line(ui, "FXAA", egui::Checkbox::new(&mut cli.render_fxaa, ""));
                        ui_setting_line(ui, "Tonemapping", egui::Checkbox::new(&mut cli.render_tonemapping, ""));
                        ui_setting_line(ui, "Bloom", egui::Checkbox::new(&mut cli.render_bloom, ""));
                        ui_setting_line(ui, "Screen Space Reflections", egui::Checkbox::new(&mut cli.render_ssr, ""));
                        ui_setting_line(ui, "Volumetric Fog", egui::Checkbox::new(&mut cli.render_volumetric_fog, ""));
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
                        if ui.button("Undo Last Drag").clicked() {
                            cfg.controls.touch_layout_request_undo = true;
                        }
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

                        ui.separator();
                        ui.label("Touch Button Action Mapping");
                        egui::ComboBox::from_label("Attack Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.attack_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sprint, "Sprint");
                                ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sneak, "Sneak");
                            });
                        egui::ComboBox::from_label("Use Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.use_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sprint, "Sprint");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sneak, "Sneak");
                            });
                        egui::ComboBox::from_label("Jump Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.jump_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sprint, "Sprint");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sneak, "Sneak");
                            });
                        egui::ComboBox::from_label("Sprint Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.sprint_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sprint, "Sprint");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sneak, "Sneak");
                            });
                        egui::ComboBox::from_label("Crouch Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.crouch_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sprint, "Sprint");
                                ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sneak, "Sneak");
                            });

                        if ui.button("Reset Touch Layout").clicked() {
                            cfg.controls.touch = Default::default();
                            cli.touch_controls_edit_mode = false;
                        }

                        ui.separator();
                        ui.label("Touch Layout Presets");
                        ui_setting_line(
                            ui,
                            "Preset Name",
                            egui::TextEdit::singleline(&mut cfg.controls.touch_layout_preset_name),
                        );
                        if ui.button("Save Current Layout As Preset").clicked() {
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
