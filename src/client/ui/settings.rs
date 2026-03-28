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
    ui.horizontal(|ui| {
        ui.add_space(20.);
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
                        ui.label("Profile: ");

                        ui_setting_line(ui, "Username", egui::TextEdit::singleline(&mut cfg.username));
                        ui_setting_line(ui, "Touch UI (large buttons)", egui::Checkbox::new(&mut cfg.touch_ui, ""));

                        // ui.group(|ui| {
                        //     ui.horizontal(|ui| {
                        //         ui.vertical(|ui| {
                        //             ui.colored_label(Color32::WHITE, cli.cfg.username.clone());
                        //             ui.small("ref.dreamtowards@gmail.com");
                        //         });

                        //         ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                        //             ui.button("Log out").clicked();
                        //             if ui.button("Account Info").clicked() {
                        //                 ui.ctx().open_url(egui::OpenUrl::new_tab("https://ethertia.com/profile/uuid"));
                        //             }
                        //         });
                        //     });

                        //     // if ui.button("Switch Account").clicked() {
                        //     //     ui.ctx().open_url(egui::OpenUrl::new_tab("https://auth.ethertia.com/login?client"));
                        //     // }
                        // });

                        // ui.label("General:");

                        ui.label("Voxel:");

                        // ui_setting_line(
                        //     ui,
                        //     "Chunks Meshing Max Concurrency",
                        //     egui::Slider::new(&mut chunk_sys.max_concurrent_meshing, 0..=50),
                        // );

                        ui_setting_line(ui, "Chunk Load Distance X", egui::Slider::new(&mut cfg.chunks_load_distance.x, -1..=25));
                        ui_setting_line(ui, "Chunk Load Distance Y", egui::Slider::new(&mut cfg.chunks_load_distance.y, -1..=25));

                        ui.label("Voxel Brush:");

                        ui_setting_line(ui, "Size", egui::Slider::new(&mut vox_brush.size, 0.0..=20.0));

                        ui_setting_line(ui, "Indensity", egui::Slider::new(&mut vox_brush.strength, 0.0..=1.0));

                        // ui_setting_line(ui, "Shape", egui::Slider::new(&mut vox_brush.shape, 0..=5));

                        ui_setting_line(ui, "Tex", egui::Slider::new(&mut vox_brush.tex, 0..=25));


                        if let Some(worldinfo) = &mut worldinfo {
                            
                            ui.label("World:");
                            
                            ui_setting_line(ui, "Day Time", egui::Slider::new(&mut worldinfo.daytime, 0.0..=1.0));

                            ui_setting_line(ui, "Day Time Length", egui::Slider::new(&mut worldinfo.daytime_length, 0.0..=60.0 * 24.0));

                        }
                        
                        ui.label("Video:");

                        ui_setting_line(ui, "FOV", egui::Slider::new(&mut cfg.fov, 10.0..=170.0));

                        ui_setting_line(ui, "VSync", egui::Checkbox::new(&mut cfg.vsync, ""));

                        ui.label("UI");

                        //ui_setting_line(ui, "UI Scale", egui::Slider::new(&mut egui_settings.scale_factor, 0.5..=2.5));

                        ui_setting_line(ui, "HUD Padding", egui::Slider::new(&mut cfg.hud_padding, 0.0..=48.0));
                        
                        ui.label("Controls");
                        if let Ok(mut ctl) = query_char.single_mut() {
                            ui_setting_line(ui, "Unfly on Grounded", egui::Checkbox::new(&mut ctl.unfly_on_ground, ""));
                        }
                    }
                    SettingsPanel::CurrentWorld => {
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
                            });
                        egui::ComboBox::from_label("Use Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.use_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sprint, "Sprint");
                            });
                        egui::ComboBox::from_label("Jump Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.jump_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sprint, "Sprint");
                            });
                        egui::ComboBox::from_label("Sprint Button Action")
                            .selected_text(format!("{:?}", cfg.controls.touch.sprint_button_action))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Attack, "Attack");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::UseItem, "UseItem");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Jump, "Jump");
                                ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sprint, "Sprint");
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
