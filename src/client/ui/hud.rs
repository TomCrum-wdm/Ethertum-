use std::collections::VecDeque;

use crate::{client::client_world::ClientPlayerInfo, prelude::*, voxel::{ChunkSystem, ClientChunkSystem, VoxShape, VoxelBrush}};

use bevy_egui::{
    egui::{text::CCursorRange, Align, Frame, Id, Layout, TextEdit},
    EguiContexts,
};
use bevy_renet::renet::RenetClient;
use avian3d::prelude::Position;
use leafwing_input_manager::action_state::ActionState;

use crate::client::prelude::{CharacterController, CharacterControllerCamera, WorldInfo};

use crate::ui::prelude::*;
use crate::{
    client::{
        game_client::ClientInfo,
        input::{InputAction, TouchButtonState, TouchStickState},
        settings::{ClientSettings, TouchActionBinding},
        ui::CurrentUI,
    },
    net::{CPacket, RenetClientHelper},
};

use super::{new_egui_window, settings::ui_setting_line};
use super::items::{draw_place_voxel, item_sort_key, placeable_voxel_defs, InventoryOperation};

// todo: Res是什么原理？每次sys调用会deep拷贝吗？还是传递指针？如果deep clone这么多消息记录 估计会很浪费性能。

#[derive(Resource, Default, Debug)]
pub struct ChatHistory {
    pub buf: String,
    pub scrollback: Vec<String>,
    pub history: VecDeque<String>,
    pub history_index: usize,
    // Line prefix symbol
    // pub symbol: String,
    // Number of commands to store in history
    // pub history_size: usize,
}

fn set_cursor_pos(ctx: &egui::Context, id: egui::Id, pos: usize) {
    if let Some(mut state) = TextEdit::load_state(ctx, id) {
        state.cursor.set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(pos))));
        // state.set_ccursor_range(Some(CCursorRange::one(egui::text::CCursor::new(pos))));
        state.store(ctx, id);
    }
}

fn safe_unit_vec3_hud(v: Vec3, fallback: Vec3) -> Vec3 {
    let n = v.normalize_or_zero();
    if n.length_squared() <= 1e-6 || !n.is_finite() {
        fallback
    } else {
        n
    }
}

fn hud_operation_color(op: InventoryOperation) -> Color32 {
    match op {
        InventoryOperation::Place => Color32::from_rgb(90, 210, 150),
        InventoryOperation::Mine => Color32::from_rgb(255, 155, 110),
        InventoryOperation::Weapon => Color32::from_rgb(255, 110, 120),
        InventoryOperation::Food => Color32::from_rgb(255, 205, 110),
        InventoryOperation::Inspect => Color32::from_rgb(150, 190, 255),
    }
}

fn hud_item_matches_operation(op: InventoryOperation, item_name: &str) -> bool {
    let name = item_name.to_ascii_lowercase();
    match op {
        InventoryOperation::Place => false,
        InventoryOperation::Mine => {
            name.contains("pickaxe")
                || name.contains("shovel")
                || name.contains("axe")
                || name.contains("shear")
                || name.contains("drill")
                || name.contains("tool")
        }
        InventoryOperation::Weapon => {
            name.contains("sword")
                || name.contains("bow")
                || name.contains("gun")
                || name.contains("spear")
                || name.contains("axe")
                || name.contains("pickaxe")
                || name.contains("grapple")
        }
        InventoryOperation::Food => {
            name.contains("apple")
                || name.contains("avocado")
                || name.contains("bread")
                || name.contains("meat")
                || name.contains("fish")
                || name.contains("berry")
                || name.contains("food")
        }
        InventoryOperation::Inspect => true,
    }
}

pub fn hud_chat(
    mut ctx: EguiContexts,
    mut state: ResMut<ChatHistory>,
    mut last_chat_count: Local<usize>,
    mut last_time_new_chat: Local<f32>,
    time: Res<Time>,
    input_key: Res<ButtonInput<KeyCode>>,
    mut cli: ResMut<ClientInfo>, // only curr_ui
    mut net_client: Option<ResMut<RenetClient>>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    let has_new_chat = state.scrollback.len() > *last_chat_count;
    *last_chat_count = state.scrollback.len();

    if input_key.just_pressed(KeyCode::Slash) && cli.curr_ui == CurrentUI::None {
        cli.curr_ui = CurrentUI::ChatInput;
    }

    // Hide ChatUi when long time no new message.
    let curr_time = time.elapsed_secs();
    if has_new_chat {
        *last_time_new_chat = curr_time;
    }
    if *last_time_new_chat < curr_time - 8. && cli.curr_ui != CurrentUI::ChatInput {
        return;
    }

    egui::Window::new("Chat")
        .default_size([620., 320.])
        .title_bar(false)
        .resizable(true)
        .collapsible(false)
        .anchor(Align2::LEFT_BOTTOM, [0., -100.])
        // .frame(Frame::default().fill(Color32::from_black_alpha(140)))
        .show(ctx_mut, |ui| {
            ui.vertical(|ui| {
                let scroll_height = ui.available_height() - 38.0;

                ui.add_space(4.);

                // Scroll area
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .stick_to_bottom(true)
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            for line in &state.scrollback {
                                ui.colored_label(Color32::WHITE, line);
                            }
                        });

                        // Scroll to bottom if have new message
                        // if has_new_chat {
                        //     ui.scroll_to_cursor(Some(Align::BOTTOM));
                        // }
                    });

                // hide input box when gaming.
                if cli.curr_ui != CurrentUI::ChatInput {
                    return;
                }

                // Input
                let text_edit = TextEdit::singleline(&mut state.buf).desired_width(f32::INFINITY).lock_focus(true);

                let text_edit_response = ui.add(text_edit);

                ui.add_space(5.);

                // Handle enter
                if text_edit_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let history_size = 20;

                    // let msg = format!("{}{}", state.symbol, state.buf);
                    // state.scrollback.push(msg.into());
                    let cmdstr = state.buf.clone();

                    if state.history.is_empty() {
                        state.history.push_front(String::default()); // editing line
                    }
                    state.history.insert(1, cmdstr.clone());
                    if state.history.len() > history_size + 1 {
                        state.history.pop_back();
                    }

                    if let Some(net_client) = net_client.as_mut() {
                        net_client.send_packet(&CPacket::ChatMessage { message: cmdstr.clone() });
                    }

                    // let mut args = Shlex::new(&state.buf).collect::<Vec<_>>();

                    // if !args.is_empty() {
                    //     let command_name = args.remove(0);
                    //     debug!("Command entered: `{command_name}`, with args: `{args:?}`");

                    //     let command = config.commands.get(command_name.as_str());

                    //     if command.is_some() {
                    //         command_entered
                    //             .send(ConsoleCommandEntered { command_name, args });
                    //     } else {
                    //         debug!(
                    //             "Command not recognized, recognized commands: `{:?}`",
                    //             config.commands.keys().collect::<Vec<_>>()
                    //         );

                    //         state.scrollback.push("error: Invalid command".into());
                    //     }
                    // }

                    state.buf.clear();

                    // Close ChatUi after Enter.
                    cli.curr_ui = CurrentUI::None;
                }

                // Clear on ctrl+l
                // if keyboard_input_events
                //     .iter()
                //     .any(|&k| k.state.is_pressed() && k.key_code == Some(KeyCode::L))
                //     && (keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]))
                // {
                //     state.scrollback.clear();
                // }

                // Handle up and down through history
                if text_edit_response.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                    && state.history.len() > 1
                    && state.history_index < state.history.len() - 1
                {
                    if state.history_index == 0 && !state.buf.trim().is_empty() {
                        let current_buf = state.buf.clone();
                        if let Some(entry) = state.history.get_mut(0) {
                            *entry = current_buf;
                        }
                    }

                    state.history_index += 1;
                    if let Some(entry) = state.history.get(state.history_index) {
                        state.buf = entry.clone();
                    }

                    set_cursor_pos(ui.ctx(), text_edit_response.id, state.buf.len());
                } else if text_edit_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && state.history_index > 0 {
                    state.history_index -= 1;
                    if let Some(entry) = state.history.get(state.history_index) {
                        state.buf = entry.clone();
                    }

                    set_cursor_pos(ui.ctx(), text_edit_response.id, state.buf.len());
                }

                // Focus on input
                ui.memory_mut(|m| m.request_focus(text_edit_response.id));
            });
        });
}

pub fn hud_hotbar(mut ctx: EguiContexts, cfg: Res<ClientSettings>, mut player: ResMut<ClientPlayerInfo>,
    items: Option<Res<crate::item::Items>>,
    mut inv_ui_state: ResMut<super::items::InventoryUiState>,
    mut voxbrush: ResMut<VoxelBrush>,
    // chunk_sys: Res<ClientChunkSystem>,
) {
    let Some(items) = items else {
        return;
    };
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    // new_egui_window("VoxBrush")
    //     .anchor(Align2::LEFT_BOTTOM, [cfg.hud_padding, -cfg.hud_padding])
    //     .frame(Frame::default().fill(Color32::from_black_alpha(30)))
    //     .show(ctx.ctx_mut(), |ui| {

    //         ui_setting_line(ui, "Size", egui::Slider::new(&mut voxbrush.size, 0.0..=25.0));
    //         ui_setting_line(ui, "Intensity", egui::Slider::new(&mut voxbrush.size, 0.0..=1.0));
    //         ui_setting_line(ui, "Tex", egui::Slider::new(&mut voxbrush.tex, 0..=28));

    //         // ui.painter().image(ctx.add_image(chunk_sys.mtl_terrain), rect, uv, tint)

    //         if ui.btn("Cube").clicked() {
    //             voxbrush.size = 1.;
    //             voxbrush.shape = VoxShape::Cube;
    //         }
    //     });

    egui::Window::new("HUD Hotbar")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_BOTTOM, [0., -cfg.hud_padding])
        .frame(Frame::default().fill(Color32::from_black_alpha(0)))
        .show(ctx_mut, |ui| {
            // Health bar
            {
                let health_bar_size = egui::Vec2::new(250., 4.);
                let mut rect = ui.min_rect();
                rect.set_height(health_bar_size.y);
                rect.set_width(health_bar_size.x);
                let rounding = ui.style().visuals.widgets.inactive.rounding();

                // bar bg
                ui.painter().rect_filled(rect, rounding, Color32::from_black_alpha(200));

                // bar fg
                let health_max = player.health_max.max(1);
                let hp_ratio = (player.health as f32 / health_max as f32).clamp(0.0, 1.0);
                let rect_fg = rect.with_max_x(rect.min.x + health_bar_size.x * hp_ratio);
                ui.painter().rect_filled(rect_fg, rounding, Color32::WHITE);

                // ui.painter().text(rect.left_center(), Align2::LEFT_CENTER,
                //     format!(" {} / {}", cli.health, cli.health_max), FontId::proportional(10.), Color32::BLACK, );

                ui.add_space(health_bar_size.y + 8.);
            }

            ui.horizontal(|ui| {
                let operation_slots = [
                    InventoryOperation::Place,
                    InventoryOperation::Mine,
                    InventoryOperation::Weapon,
                    InventoryOperation::Food,
                    InventoryOperation::Inspect,
                ];

                for op in operation_slots {
                    let active = inv_ui_state.active_operation == op;
                    let tint = hud_operation_color(op);
                    let resp = sfx_play(ui.add_sized(
                        [50.0, 50.0],
                        egui::Button::new(op.label())
                            .fill(if active {
                                Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 120)
                            } else {
                                Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 46)
                            })
                            .stroke(egui::Stroke::new(2.0, tint)),
                    ));
                    if resp.clicked() {
                        inv_ui_state.active_operation = op;
                        inv_ui_state.operation_filters = vec![op];
                    }
                }

                ui.separator();

                let visible_item_slots = 6usize;
                if inv_ui_state.active_operation == InventoryOperation::Place {
                    let mut place_defs = placeable_voxel_defs().to_vec();
                    let current_tex = voxbrush.tex;
                    place_defs.sort_by_key(|def| if def.tex == current_tex { 0 } else { 1 });

                    for i in 0..visible_item_slots {
                        if let Some(def) = place_defs.get(i).copied() {
                            let active = voxbrush.tex == def.tex;
                            let tint = hud_operation_color(InventoryOperation::Place);
                            let mut resp = sfx_play(ui.add_sized(
                                [50.0, 50.0],
                                egui::Button::new("")
                                    .fill(if active {
                                        Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 120)
                                    } else {
                                        Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 46)
                                    })
                                    .stroke(egui::Stroke::new(2.0, tint)),
                            ));
                            resp = resp.on_hover_text(format!("{} [tex:{}]", def.name, def.tex));
                            draw_place_voxel(&def, resp.rect, ui.painter(), &items);

                            if resp.clicked() {
                                voxbrush.tex = def.tex;
                                voxbrush.shape = def.shape;
                            }
                        } else {
                            ui.add_sized([50.0, 50.0], egui::Button::new(""));
                        }
                    }
                } else {
                    let mut filtered_indices = Vec::new();
                    for (idx, stack) in player.inventory.items.iter().enumerate() {
                        if stack.is_empty() {
                            continue;
                        }
                        let Some(name) = items.reg.at((stack.item_id - 1) as u16) else {
                            continue;
                        };
                        if hud_item_matches_operation(inv_ui_state.active_operation, name) {
                            filtered_indices.push(idx);
                        }
                    }

                    filtered_indices.sort_by_key(|slot_idx| {
                        let stack = &player.inventory.items[*slot_idx];
                        item_sort_key(&items, stack.item_id)
                    });

                    for i in 0..visible_item_slots {
                        if let Some(slot_idx) = filtered_indices.get(i).copied() {
                            if let Some(item) = player.inventory.items.get_mut(slot_idx) {
                                ui_item_stack(ui, item, slot_idx, &items, &mut inv_ui_state);
                            }
                        } else {
                            ui.add_sized([50.0, 50.0], egui::Button::new(""));
                        }
                    }
                }
            });
        });
}

pub fn hud_playerlist(
    mut ctx: EguiContexts,
    input_key: Res<ButtonInput<KeyCode>>,
    cli: Res<ClientInfo>,
    cfg: Res<ClientSettings>,
    mut net_client: Option<ResMut<RenetClient>>,
) {
    if !input_key.pressed(KeyCode::Tab) {
        return;
    }
    if input_key.just_pressed(KeyCode::Tab) {
        info!("Request PlayerList");
        if let Some(net_client) = net_client.as_mut() {
            net_client.send_packet(&CPacket::PlayerList);
        }
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    egui::Window::new("PlayerList")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_TOP, [0., cfg.hud_padding])
        .show(ctx_mut, |ui| {
            for player in &cli.playerlist {
                ui.horizontal(|ui| {
                    ui.set_width(280.);

                    // ui.add_sized([180., 24.], egui::Label::new(player.0.as_str()));
                    ui.colored_label(Color32::WHITE, player.0.as_str());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.colored_label(Color32::GRAY, format!("{}ms", player.1));
                    })
                });
            }
            // ui.separator();
            // ui.label("Server MOTD Footer Test");

            // Lock Focus when pressing Tab
            ui.memory_mut(|m| m.request_focus(Id::NULL));
        });
}

pub fn hud_attitude_indicators(
    mut ctx: EguiContexts,
    cfg: Res<ClientSettings>,
    worldinfo: Res<WorldInfo>,
    query_cam: Query<&Transform, With<CharacterControllerCamera>>,
    query_char: Query<&Position, (With<CharacterController>, Without<CharacterControllerCamera>)>,
) {
    if !cfg.show_level_indicator && !cfg.show_pitch_indicator {
        return;
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };
    let Ok(cam_trans) = query_cam.single() else {
        return;
    };
    let Some(char_pos) = query_char.iter().next() else {
        return;
    };

    let local_up = safe_unit_vec3_hud(worldinfo.world_config.world_up_at(char_pos.0), Vec3::Y);
    let look_dir = safe_unit_vec3_hud(cam_trans.rotation * -Vec3::Z, -Vec3::Z);
    let cam_up = safe_unit_vec3_hud(cam_trans.rotation * Vec3::Y, Vec3::Y);
    let projected_world_up = safe_unit_vec3_hud(local_up - look_dir * local_up.dot(look_dir), cam_up);

    let sin_roll = look_dir.dot(cam_up.cross(projected_world_up));
    let cos_roll = cam_up.dot(projected_world_up).clamp(-1.0, 1.0);
    let roll_rad = sin_roll.atan2(cos_roll);
    let roll_deg = roll_rad.to_degrees();
    let pitch_deg = look_dir.dot(local_up).clamp(-1.0, 1.0).asin().to_degrees();

    egui::Window::new("Attitude Indicators")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .movable(false)
        .anchor(Align2::RIGHT_TOP, [-14.0, cfg.hud_padding + 14.0])
        .frame(
            Frame::default()
                .fill(Color32::from_rgba_premultiplied(8, 12, 18, 180))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 180, 220, 180)))
                .corner_radius(6.0),
        )
        .show(ctx_mut, |ui| {
            ui.set_min_width(236.0);

            if cfg.show_level_indicator {
                ui.colored_label(Color32::from_rgb(200, 230, 255), format!("Level: {:+.1}°", roll_deg));

                let size = egui::vec2(220.0, 112.0);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                let center = rect.center();
                let radius = (rect.width().min(rect.height()) * 0.46).max(1.0);

                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(180, 220, 255, 210)),
                );

                let pitch_offset = (-pitch_deg / 45.0).clamp(-1.2, 1.2) * radius * 0.6;
                let horizon_dir = egui::vec2(roll_rad.cos(), roll_rad.sin());
                let center_shifted = center + egui::vec2(0.0, pitch_offset);
                let p1 = center_shifted - horizon_dir * radius * 1.2;
                let p2 = center_shifted + horizon_dir * radius * 1.2;

                painter.line_segment([p1, p2], egui::Stroke::new(2.0, Color32::from_rgb(120, 210, 255)));
                painter.line_segment(
                    [
                        egui::pos2(center.x - 9.0, center.y),
                        egui::pos2(center.x + 9.0, center.y),
                    ],
                    egui::Stroke::new(2.0, Color32::from_rgb(255, 230, 130)),
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x, center.y - 7.0),
                        egui::pos2(center.x, center.y + 7.0),
                    ],
                    egui::Stroke::new(2.0, Color32::from_rgb(255, 230, 130)),
                );

                ui.add_space(4.0);
            }

            if cfg.show_pitch_indicator {
                ui.colored_label(Color32::from_rgb(200, 230, 255), format!("Pitch: {:+.1}°", pitch_deg));
                let pitch_norm = ((pitch_deg + 90.0) / 180.0).clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(pitch_norm)
                        .desired_width(220.0)
                        .text(format!("{:+.1}°", pitch_deg)),
                );
            }
        });
}

pub fn hud_touch_sticks(
    mut ctx: EguiContexts,
    mut cfg: ResMut<ClientSettings>,
    cli: Res<ClientInfo>,
    time: Res<Time>,
    mut sticks: ResMut<TouchStickState>,
    mut buttons: ResMut<TouchButtonState>,
    mut inv_ui_state: ResMut<super::items::InventoryUiState>,
    query_action: Query<&ActionState<InputAction>>,
    mut query_controller: Query<&mut CharacterController>,
    asset_server: Res<AssetServer>,
    mut texture_ids: Local<(Option<egui::TextureId>, Option<egui::TextureId>)>,
    mut texture_handles: Local<(Handle<Image>, Handle<Image>)>,
    mut drag_in_progress: Local<bool>,
    mut last_up_tap_time: Local<f32>,
    mut up_hold_prev: Local<bool>,
) {
    let Ok(action_state) = query_action.single() else {
        return;
    };
    let show_for_edit = cli.touch_controls_edit_mode && cli.curr_ui == CurrentUI::Settings;
    let show_runtime = cfg.touch_ui && cli.curr_ui == CurrentUI::None && !cli.touch_controls_edit_mode;
    if !cfg.touch_ui || !(show_runtime || show_for_edit) {
        buttons.attack_pressed = false;
        buttons.attack_just_pressed = false;
        buttons.use_pressed = false;
        buttons.use_just_pressed = false;
        buttons.jump_pressed = false;
        buttons.jump_just_pressed = false;
        buttons.sprint_pressed = false;
        buttons.sprint_just_pressed = false;
        buttons.crouch_pressed = false;
        buttons.crouch_just_pressed = false;
        buttons.vertical_axis = 0.0;
        buttons.vertical_active = false;
        return;
    }

    if texture_handles.0.id() == Handle::<Image>::default().id() {
        texture_handles.0 = asset_server.load("knob.png");
    }
    if texture_handles.1.id() == Handle::<Image>::default().id() {
        texture_handles.1 = asset_server.load("outline.png");
    }
    if texture_ids.0.is_none() && texture_handles.0.id() != Handle::<Image>::default().id() {
        texture_ids.0 = Some(ctx.add_image(bevy_egui::EguiTextureHandle::Strong(texture_handles.0.clone())));
    }
    if texture_ids.1.is_none() && texture_handles.1.id() != Handle::<Image>::default().id() {
        texture_ids.1 = Some(ctx.add_image(bevy_egui::EguiTextureHandle::Strong(texture_handles.1.clone())));
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    if cfg.controls.touch_layout_request_undo {
        if let Some(prev) = cfg.controls.touch_layout_undo_stack.pop() {
            cfg.controls.touch = prev;
        }
        cfg.controls.touch_layout_request_undo = false;
    }

    let screen = ctx_mut.input(|i| i.screen_rect());
    let screen_size = bevy::math::Vec2::new(screen.width().max(1.0), screen.height().max(1.0));
    let touch_cfg = &mut cfg.controls.touch;
    let layout_before = touch_cfg.clone();
    let mut layout_changed = false;

    let to_pos = |uv: [f32; 2]| -> bevy::math::Vec2 {
        bevy::math::Vec2::new(uv[0].clamp(0.05, 0.95) * screen_size.x, uv[1].clamp(0.05, 0.95) * screen_size.y)
    };
    let to_uv = |pos: bevy::math::Vec2| -> [f32; 2] {
        [
            (pos.x / screen_size.x).clamp(0.05, 0.95),
            (pos.y / screen_size.y).clamp(0.05, 0.95),
        ]
    };

    let stick_center = to_pos(touch_cfg.move_stick_pos);
    let stick_radius = touch_cfg.move_stick_radius.clamp(48.0, 200.0);
    let knob_pos = if sticks.active {
        bevy::math::Vec2::new(
            stick_center.x + sticks.move_axis.x * stick_radius,
            stick_center.y - sticks.move_axis.y * stick_radius,
        )
    } else {
        stick_center
    };

    let prev = (*buttons).clone();
    buttons.attack_pressed = false;
    buttons.use_pressed = false;
    buttons.jump_pressed = false;
    buttons.sprint_pressed = false;
    buttons.crouch_pressed = false;
    buttons.vertical_axis = 0.0;
    buttons.vertical_active = false;

    let is_flying = query_controller
        .iter()
        .next()
        .is_some_and(|c| c.is_flying || c.noclip_enabled);
    let sprint_visual_active = action_state.pressed(&InputAction::Sprint) || sticks.sprint_locked;

    let action_pressed = |binding: TouchActionBinding| -> bool {
        match binding {
            TouchActionBinding::Attack => action_state.pressed(&InputAction::Attack),
            TouchActionBinding::UseItem => action_state.pressed(&InputAction::UseItem),
            TouchActionBinding::Jump => action_state.pressed(&InputAction::Jump),
            TouchActionBinding::Sprint => action_state.pressed(&InputAction::Sprint),
            TouchActionBinding::Sneak => action_state.pressed(&InputAction::Sneak),
        }
    };

    let painter = ctx_mut.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("touch_controls_overlay")));

    if let (Some(knob_tex), Some(outline_tex)) = (texture_ids.0, texture_ids.1) {
        let base_rect = egui::Rect::from_center_size(
            egui::pos2(stick_center.x, stick_center.y),
            egui::vec2(stick_radius * 2.0, stick_radius * 2.0),
        );
        let knob_rect = egui::Rect::from_center_size(
            egui::pos2(knob_pos.x, knob_pos.y),
            egui::vec2(stick_radius * 0.9, stick_radius * 0.9),
        );
        let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));

        painter.image(outline_tex, base_rect, uv, Color32::from_rgba_premultiplied(255, 255, 255, 180));
        painter.image(knob_tex, knob_rect, uv, Color32::from_rgba_premultiplied(255, 255, 255, 220));
    } else {
        painter.circle_filled(egui::pos2(stick_center.x, stick_center.y), stick_radius, Color32::from_rgba_premultiplied(80, 180, 255, 38));
        painter.circle_stroke(
            egui::pos2(stick_center.x, stick_center.y),
            stick_radius,
            egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(120, 210, 255, 160)),
        );
        painter.circle_filled(egui::pos2(knob_pos.x, knob_pos.y), stick_radius * 0.40, Color32::from_rgba_premultiplied(130, 220, 255, 190));
    }

    if sticks.sprint_locked {
        let lock_pos = egui::pos2(stick_center.x, stick_center.y - stick_radius - 18.0);
        painter.text(
            lock_pos,
            egui::Align2::CENTER_CENTER,
            "RUN LOCK",
            egui::FontId::proportional(13.0),
            Color32::from_rgb(255, 220, 130),
        );
    }

    if cli.touch_controls_edit_mode {
        egui::Area::new(Id::new("touch_move_stick_drag"))
            .fixed_pos(egui::pos2(stick_center.x - stick_radius, stick_center.y - stick_radius))
            .interactable(true)
            .show(ctx_mut, |ui| {
                let (_, resp) = ui.allocate_exact_size(egui::vec2(stick_radius * 2.0, stick_radius * 2.0), egui::Sense::click_and_drag());
                if resp.dragged() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let new_pos = to_uv(bevy::math::Vec2::new(pos.x, pos.y));
                        if touch_cfg.move_stick_pos != new_pos {
                            touch_cfg.move_stick_pos = new_pos;
                            layout_changed = true;
                        }
                    }
                }
            });
    }

    let button_radius = touch_cfg.button_radius.clamp(30.0, 80.0);
    let mut draw_operation_selector = |id: &str, pos_uv: &mut [f32; 2], op: InventoryOperation, label: &'static str| {
        let pos = to_pos(*pos_uv);
        let tint = hud_operation_color(op);

        egui::Area::new(Id::new(format!("touch_op_btn_{id}")))
            .fixed_pos(egui::pos2(pos.x - button_radius, pos.y - button_radius))
            .interactable(true)
            .show(ctx_mut, |ui| {
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(button_radius * 2.0, button_radius * 2.0),
                    egui::Sense::click_and_drag(),
                );
                let resp = sfx_play(resp);
                let p = rect.center();

                let touch_pressed = resp.hovered() && ui.input(|i| i.pointer.primary_down()) && !cli.touch_controls_edit_mode;
                let visual_pressed = touch_pressed || inv_ui_state.active_operation == op;

                if cli.touch_controls_edit_mode && resp.dragged() {
                    if let Some(pointer_pos) = resp.interact_pointer_pos() {
                        let new_pos = to_uv(bevy::math::Vec2::new(pointer_pos.x, pointer_pos.y));
                        if *pos_uv != new_pos {
                            *pos_uv = new_pos;
                            layout_changed = true;
                        }
                    }
                }

                let fill = if visual_pressed {
                    Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 110)
                } else {
                    Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 42)
                };
                let stroke = if cli.touch_controls_edit_mode {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 220, 120, 220))
                } else {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(tint.r(), tint.g(), tint.b(), 200))
                };

                ui.painter().circle_filled(p, button_radius, fill);
                ui.painter().circle_stroke(p, button_radius, stroke);
                let text_color = if visual_pressed {
                    Color32::from_rgb(255, 250, 220)
                } else {
                    Color32::WHITE
                };
                let text_offset = if visual_pressed { 1.2 } else { 0.0 };
                ui.painter().text(
                    egui::pos2(p.x, p.y + text_offset),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(14.0),
                    text_color,
                );

                if resp.clicked() && !cli.touch_controls_edit_mode {
                    inv_ui_state.active_operation = op;
                    inv_ui_state.operation_filters = vec![op];
                }
            });
    };

    draw_operation_selector("pick_as_op", &mut touch_cfg.attack_button_pos, InventoryOperation::Mine, "MINE");
    draw_operation_selector("use_as_op", &mut touch_cfg.use_button_pos, InventoryOperation::Place, "PLACE");
    if is_flying {
        let slider_center = to_pos(touch_cfg.vertical_slider_pos);
        let slider_h = touch_cfg.vertical_slider_height.clamp(120.0, 320.0);
        let slider_w = touch_cfg.vertical_slider_width.clamp(44.0, 96.0);
        let slider_half_h = slider_h * 0.5;
        let slider_rect = egui::Rect::from_center_size(
            egui::pos2(slider_center.x, slider_center.y),
            egui::vec2(slider_w, slider_h),
        );

        egui::Area::new(Id::new("touch_vertical_slider"))
            .fixed_pos(slider_rect.min)
            .interactable(true)
            .show(ctx_mut, |ui| {
                let (rect, resp) = ui.allocate_exact_size(slider_rect.size(), egui::Sense::click_and_drag());

                if cli.touch_controls_edit_mode && resp.dragged() {
                    if let Some(pointer_pos) = resp.interact_pointer_pos() {
                        let new_pos = to_uv(bevy::math::Vec2::new(pointer_pos.x, pointer_pos.y));
                        if touch_cfg.vertical_slider_pos != new_pos {
                            touch_cfg.vertical_slider_pos = new_pos;
                            layout_changed = true;
                        }
                    }
                }

                let runtime_dragging = !cli.touch_controls_edit_mode && resp.hovered() && ui.input(|i| i.pointer.primary_down());
                let mut display_axis = if buttons.vertical_active {
                    buttons.vertical_axis
                } else {
                    0.0
                };
                let keyboard_axis = match (action_pressed(TouchActionBinding::Jump), action_pressed(TouchActionBinding::Sneak)) {
                    (true, false) => 1.0,
                    (false, true) => -1.0,
                    _ => 0.0,
                };
                if keyboard_axis != 0.0 {
                    display_axis = keyboard_axis;
                }
                if runtime_dragging {
                    if let Some(pointer_pos) = resp.interact_pointer_pos() {
                        let axis = ((slider_center.y - pointer_pos.y) / slider_half_h).clamp(-1.0, 1.0);
                        buttons.vertical_axis = axis;
                        buttons.vertical_active = axis.abs() > 0.01;
                        display_axis = axis;
                    }
                }

                let p = ui.painter();
                let track_fill = if cli.touch_controls_edit_mode {
                    Color32::from_rgba_premultiplied(255, 220, 120, 52)
                } else {
                    Color32::from_rgba_premultiplied(120, 180, 255, 38)
                };
                let track_stroke = if cli.touch_controls_edit_mode {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 220, 120, 220))
                } else {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(135, 215, 255, 220))
                };
                p.rect(
                    rect,
                    egui::CornerRadius::same((slider_w * 0.28) as u8),
                    track_fill,
                    track_stroke,
                    egui::StrokeKind::Middle,
                );

                let mid_y = rect.center().y;
                p.line_segment(
                    [egui::pos2(rect.left() + 6.0, mid_y), egui::pos2(rect.right() - 6.0, mid_y)],
                    egui::Stroke::new(1.5, Color32::from_rgba_premultiplied(240, 248, 255, 170)),
                );

                let thumb_y = slider_center.y - display_axis * slider_half_h;
                let thumb_center = egui::pos2(rect.center().x, thumb_y.clamp(rect.top() + 14.0, rect.bottom() - 14.0));
                p.circle_filled(thumb_center, slider_w * 0.28, Color32::from_rgba_premultiplied(140, 225, 255, 220));
                p.circle_stroke(
                    thumb_center,
                    slider_w * 0.28,
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(220, 248, 255, 235)),
                );

                p.text(
                    egui::pos2(rect.center().x, rect.top() - 12.0),
                    egui::Align2::CENTER_CENTER,
                    "FLY",
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(255, 230, 140),
                );
                p.text(
                    egui::pos2(rect.center().x, rect.top() + 12.0),
                    egui::Align2::CENTER_CENTER,
                    "UP",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(170, 230, 255),
                );
                p.text(
                    egui::pos2(rect.center().x, rect.bottom() - 12.0),
                    egui::Align2::CENTER_CENTER,
                    "DOWN",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(195, 180, 255),
                );

                let land_button_pos = egui::pos2(rect.center().x - 48.0, rect.bottom() + 16.0);
                egui::Area::new(Id::new("touch_land_button"))
                    .fixed_pos(land_button_pos)
                    .show(ui.ctx(), |ui| {
                        if sfx_play(ui.add_sized([96.0, 32.0], egui::Button::new("LAND"))).clicked() {
                            if let Some(mut ctl) = query_controller.iter_mut().next() {
                                ctl.is_flying = false;
                                ctl.noclip_enabled = false;
                            }
                        }
                    });
            });
    } else {
        let slider_center = to_pos(touch_cfg.vertical_slider_pos);
        let slider_h = touch_cfg.vertical_slider_height.clamp(120.0, 320.0);
        let slider_w = touch_cfg.vertical_slider_width.clamp(44.0, 96.0);
        let capsule_rect = egui::Rect::from_center_size(
            egui::pos2(slider_center.x, slider_center.y),
            egui::vec2(slider_w, slider_h),
        );
        let mut up_just_pressed = false;

        egui::Area::new(Id::new("touch_land_capsule"))
            .fixed_pos(capsule_rect.min)
            .interactable(true)
            .show(ctx_mut, |ui| {
                let (rect, resp) = ui.allocate_exact_size(capsule_rect.size(), egui::Sense::click_and_drag());
                let resp = sfx_play(resp);

                if cli.touch_controls_edit_mode && resp.dragged() {
                    if let Some(pointer_pos) = resp.interact_pointer_pos() {
                        let new_pos = to_uv(bevy::math::Vec2::new(pointer_pos.x, pointer_pos.y));
                        if touch_cfg.vertical_slider_pos != new_pos {
                            touch_cfg.vertical_slider_pos = new_pos;
                            layout_changed = true;
                        }
                    }
                }

                let pointer_down = ui.input(|i| i.pointer.primary_down()) && !cli.touch_controls_edit_mode;
                let pointer_pos = resp.interact_pointer_pos();
                let top_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.center().y));
                let bottom_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.center().y), rect.max);

                let top_pressed = pointer_down
                    && resp.hovered()
                    && pointer_pos.is_some_and(|p| top_rect.contains(p));
                let bottom_pressed = pointer_down
                    && resp.hovered()
                    && pointer_pos.is_some_and(|p| bottom_rect.contains(p));

                up_just_pressed = top_pressed && !*up_hold_prev;
                *up_hold_prev = top_pressed;

                let jump_active = top_pressed || action_pressed(touch_cfg.jump_button_action);
                let crouch_active = bottom_pressed || action_pressed(touch_cfg.crouch_button_action);

                if top_pressed {
                    if matches!(touch_cfg.jump_button_action, TouchActionBinding::Jump) {
                        buttons.jump_pressed = true;
                    }
                    if matches!(touch_cfg.jump_button_action, TouchActionBinding::Sneak) {
                        buttons.crouch_pressed = true;
                    }
                }
                if bottom_pressed {
                    if matches!(touch_cfg.crouch_button_action, TouchActionBinding::Jump) {
                        buttons.jump_pressed = true;
                    }
                    if matches!(touch_cfg.crouch_button_action, TouchActionBinding::Sneak) {
                        buttons.crouch_pressed = true;
                    }
                }

                let p = ui.painter();
                let capsule_fill = if cli.touch_controls_edit_mode {
                    Color32::from_rgba_premultiplied(255, 220, 120, 46)
                } else {
                    Color32::from_rgba_premultiplied(120, 180, 255, 34)
                };
                let capsule_stroke = if cli.touch_controls_edit_mode {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 220, 120, 220))
                } else {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(140, 215, 255, 210))
                };
                p.rect(
                    rect,
                    egui::CornerRadius::same((slider_w * 0.48) as u8),
                    capsule_fill,
                    capsule_stroke,
                    egui::StrokeKind::Middle,
                );

                let split_color = Color32::from_rgba_premultiplied(232, 245, 255, 180);
                p.line_segment(
                    [egui::pos2(rect.left() + 6.0, rect.center().y), egui::pos2(rect.right() - 6.0, rect.center().y)],
                    egui::Stroke::new(1.6, split_color),
                );

                let up_fill = if jump_active {
                    Color32::from_rgba_premultiplied(90, 210, 255, 135)
                } else {
                    Color32::TRANSPARENT
                };
                let down_fill = if crouch_active {
                    Color32::from_rgba_premultiplied(255, 158, 96, 130)
                } else {
                    Color32::TRANSPARENT
                };
                p.rect_filled(top_rect.shrink2(egui::vec2(2.0, 2.0)), egui::CornerRadius::same((slider_w * 0.42) as u8), up_fill);
                p.rect_filled(bottom_rect.shrink2(egui::vec2(2.0, 2.0)), egui::CornerRadius::same((slider_w * 0.42) as u8), down_fill);

                let top_text_color = if jump_active { Color32::from_rgb(220, 250, 255) } else { Color32::from_rgb(180, 235, 255) };
                let bottom_text_color = if crouch_active { Color32::from_rgb(255, 236, 214) } else { Color32::from_rgb(255, 205, 174) };
                p.text(
                    egui::pos2(rect.center().x, top_rect.center().y + if jump_active { 1.0 } else { 0.0 }),
                    egui::Align2::CENTER_CENTER,
                    "UP",
                    egui::FontId::proportional(14.0),
                    top_text_color,
                );
                p.text(
                    egui::pos2(rect.center().x, bottom_rect.center().y + if crouch_active { 1.0 } else { 0.0 }),
                    egui::Align2::CENTER_CENTER,
                    "DOWN",
                    egui::FontId::proportional(14.0),
                    bottom_text_color,
                );
            });

        if up_just_pressed && !show_for_edit {
            let now = time.elapsed_secs();
            let tap_window = cfg.controls.touch.fly_double_tap_window_secs.clamp(0.18, 0.65);
            if now - *last_up_tap_time < tap_window {
                if let Some(mut ctl) = query_controller.iter_mut().next() {
                    ctl.is_flying = true;
                }
            }
            *last_up_tap_time = now;
        }

        if !show_for_edit {
            let jump_threshold = 0.35;
            let touch_jump_pressed = buttons.jump_pressed || action_pressed(TouchActionBinding::Jump);
            let touch_crouch_pressed = buttons.crouch_pressed || action_pressed(TouchActionBinding::Sneak);
            buttons.jump_pressed = touch_jump_pressed;
            buttons.crouch_pressed = touch_crouch_pressed;
            buttons.vertical_axis = if touch_jump_pressed && !touch_crouch_pressed {
                1.0
            } else if touch_crouch_pressed && !touch_jump_pressed {
                -1.0
            } else {
                0.0
            };
            buttons.vertical_active = buttons.vertical_axis.abs() > 0.01;

            if buttons.jump_pressed || buttons.crouch_pressed {
                if buttons.jump_pressed && !buttons.crouch_pressed && buttons.vertical_axis > jump_threshold {
                    buttons.jump_pressed = true;
                }
                if buttons.crouch_pressed && !buttons.jump_pressed && buttons.vertical_axis < -jump_threshold {
                    buttons.crouch_pressed = true;
                }
            }

        }
    }
    if show_for_edit {
        let pointer_down = ctx_mut.input(|i| i.pointer.primary_down());
        if layout_changed && !*drag_in_progress {
            if cfg.controls.touch_layout_undo_stack.len() >= 32 {
                cfg.controls.touch_layout_undo_stack.remove(0);
            }
            cfg.controls.touch_layout_undo_stack.push(layout_before);
            *drag_in_progress = true;
        }
        if !pointer_down {
            *drag_in_progress = false;
        }
    } else {
        *drag_in_progress = false;
    }
    if is_flying || show_for_edit {
        *up_hold_prev = false;
    }

    if show_for_edit {
        egui::Area::new(Id::new("touch_designer_hint"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 24.0])
            .show(ctx_mut, |ui| {
                egui::Frame::default()
                    .fill(Color32::from_rgba_premultiplied(10, 16, 24, 210))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(140, 200, 255, 180)))
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.colored_label(Color32::from_rgb(180, 220, 255), "Touch Layout Designer: drag joystick/buttons to reposition. Gameplay input is locked while editing.");
                    });
            });
    }

    buttons.attack_just_pressed = buttons.attack_pressed && !prev.attack_pressed;
    buttons.use_just_pressed = buttons.use_pressed && !prev.use_pressed;
    buttons.jump_just_pressed = buttons.jump_pressed && !prev.jump_pressed;
    buttons.sprint_just_pressed = buttons.sprint_pressed && !prev.sprint_pressed;
    buttons.crouch_just_pressed = buttons.crouch_pressed && !prev.crouch_pressed;
}

