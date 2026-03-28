use std::collections::VecDeque;

use crate::{client::client_world::ClientPlayerInfo, prelude::*, voxel::{ChunkSystem, ClientChunkSystem, VoxShape, VoxelBrush}};

use bevy_egui::{
    egui::{text::CCursorRange, Align, Frame, Id, Layout, TextEdit},
    EguiContexts,
};
use bevy_renet::renet::RenetClient;

use crate::ui::prelude::*;
use crate::{
    client::{
        game_client::ClientInfo,
        input::{TouchButtonState, TouchStickState},
        settings::{ClientSettings, TouchActionBinding},
        ui::CurrentUI,
    },
    net::{CPacket, RenetClientHelper},
};

use super::{new_egui_window, settings::ui_setting_line};

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
                for i in 0..ClientPlayerInfo::HOTBAR_SLOTS {
                    if let Some(item) = player.inventory.items.get_mut(i as usize) {
                        ui_item_stack(ui, item, i as usize, &items, &mut inv_ui_state);
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

pub fn hud_touch_sticks(
    mut ctx: EguiContexts,
    mut cfg: ResMut<ClientSettings>,
    cli: Res<ClientInfo>,
    sticks: Res<TouchStickState>,
    mut buttons: ResMut<TouchButtonState>,
    asset_server: Res<AssetServer>,
    mut texture_ids: Local<(Option<egui::TextureId>, Option<egui::TextureId>)>,
    mut texture_handles: Local<(Handle<Image>, Handle<Image>)>,
    mut drag_in_progress: Local<bool>,
) {
    let show_for_edit = cli.touch_controls_edit_mode && cli.curr_ui == CurrentUI::Settings;
    let show_runtime = cfg.touch_ui && cli.curr_ui == CurrentUI::None && !cli.touch_controls_edit_mode && cfg!(target_os = "android");
    if !cfg.touch_ui || !(show_runtime || show_for_edit) {
        buttons.attack_pressed = false;
        buttons.attack_just_pressed = false;
        buttons.use_pressed = false;
        buttons.use_just_pressed = false;
        buttons.jump_pressed = false;
        buttons.jump_just_pressed = false;
        buttons.sprint_pressed = false;
        buttons.sprint_just_pressed = false;
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
    let mut draw_button = |id: &str, label: &str, pos_uv: &mut [f32; 2], binding: TouchActionBinding| {
        let pos = to_pos(*pos_uv);

        egui::Area::new(Id::new(format!("touch_btn_{id}")))
            .fixed_pos(egui::pos2(pos.x - button_radius, pos.y - button_radius))
            .interactable(true)
            .show(ctx_mut, |ui| {
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(button_radius * 2.0, button_radius * 2.0),
                    egui::Sense::click_and_drag(),
                );
                let p = rect.center();

                let pressed = resp.hovered() && ui.input(|i| i.pointer.primary_down()) && !cli.touch_controls_edit_mode;

                if cli.touch_controls_edit_mode && resp.dragged() {
                    if let Some(pointer_pos) = resp.interact_pointer_pos() {
                        let new_pos = to_uv(bevy::math::Vec2::new(pointer_pos.x, pointer_pos.y));
                        if *pos_uv != new_pos {
                            *pos_uv = new_pos;
                            layout_changed = true;
                        }
                    }
                }

                let fill = if pressed {
                    Color32::from_rgba_premultiplied(255, 255, 255, 64)
                } else {
                    Color32::from_rgba_premultiplied(255, 255, 255, 28)
                };
                let stroke = if cli.touch_controls_edit_mode {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 220, 120, 220))
                } else {
                    egui::Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 255, 255, 140))
                };

                ui.painter().circle_filled(p, button_radius, fill);
                ui.painter().circle_stroke(p, button_radius, stroke);
                ui.painter().text(p, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(16.0), Color32::WHITE);

                if pressed {
                    match binding {
                        TouchActionBinding::Attack => buttons.attack_pressed = true,
                        TouchActionBinding::UseItem => buttons.use_pressed = true,
                        TouchActionBinding::Jump => buttons.jump_pressed = true,
                        TouchActionBinding::Sprint => buttons.sprint_pressed = true,
                    }
                }
            });
    };

    draw_button("attack", "ATK", &mut touch_cfg.attack_button_pos, touch_cfg.attack_button_action);
    draw_button("use", "USE", &mut touch_cfg.use_button_pos, touch_cfg.use_button_action);
    draw_button("jump", "JMP", &mut touch_cfg.jump_button_pos, touch_cfg.jump_button_action);
    draw_button("sprint", "SPR", &mut touch_cfg.sprint_button_pos, touch_cfg.sprint_button_action);

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
}

