use bevy::prelude::*;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton, GamepadRumbleIntensity, GamepadRumbleRequest};
use bevy_egui::{
    egui::{self, Color32, Layout, Ui, Widget},
    EguiContexts,
};
use core::time::Duration;

use super::{new_egui_window, sfx_play, ui_lr_panel};
use crate::client::l10n;
use crate::client::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingTag {
    Performance,
    Fun,
    Dangerous,
}

// Render the floating Touch Tile Style overlay so it can appear over the main menu.
pub fn ui_touch_tile_style_overlay(
    mut ctx: EguiContexts,
    mut cfg: ResMut<ClientSettings>,
    mut cli: ResMut<ClientInfo>,
    mut images: ResMut<Assets<Image>>,
    mut prev_style: Local<Option<crate::client::settings::TouchTileStyle>>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else { return; };

    if !cfg.touch_tile_style_overlay_enabled {
        return;
    }

    let alpha_u8 = (cfg.touch_tile_style_window_alpha.clamp(0.0, 1.0) * 255.0) as u8;
    let mut frame = egui::Frame::default();
    frame.fill = Color32::from_rgba_unmultiplied(22, 26, 34, alpha_u8);
    frame.stroke = egui::Stroke::new(1.0, Color32::from_gray(60));

    let pos = ctx_mut.available_rect().right_top();
    let default_x = pos.x - 320.0;
    let default_y = crate::client::ui::ui_safe_top() + 64.0;
    // copy open state to avoid double mutable borrow of `cfg`
    let mut overlay_open = cfg.touch_tile_style_overlay_enabled;
    egui::Window::new(l10n::tr("Touch Tile Style Overlay"))
        .open(&mut overlay_open)
        .default_pos(egui::pos2(default_x, default_y))
        .movable(true)
        .resizable(false)
        .frame(frame)
        .show(ctx_mut, |ui| {
            ui.set_width(260.0);

            // Background mode
            ui.horizontal(|ui| {
                ui.label(l10n::tr("Background Mode"));
                let is_cover = matches!(cfg.touch_tile_style.background_mode, crate::client::settings::TileBackgroundMode::Cover);
                if ui.radio(is_cover, l10n::tr("Cover (fill)")) .changed() {
                    cfg.touch_tile_style.background_mode = crate::client::settings::TileBackgroundMode::Cover;
                    cli.curr_ui = CurrentUI::MainMenu;
                }
                if ui.radio(!is_cover, l10n::tr("Contain (fit)")) .changed() {
                    cfg.touch_tile_style.background_mode = crate::client::settings::TileBackgroundMode::Contain;
                    cli.curr_ui = CurrentUI::MainMenu;
                }
            });

            // Corner radius
            let mut corner = cfg.touch_tile_style.corner_radius;
            let resp = ui.add(egui::Slider::new(&mut corner, 0.0..=24.0).text(l10n::tr("Corner Radius")));
            if resp.changed() {
                cfg.touch_tile_style.corner_radius = corner;
                cli.curr_ui = CurrentUI::MainMenu;
            }

            // Icon scale
            let mut iscale = cfg.touch_tile_style.icon_scale;
            let resp2 = ui.add(egui::Slider::new(&mut iscale, 0.5..=2.0).text(l10n::tr("Icon Scale")));
            if resp2.changed() {
                cfg.touch_tile_style.icon_scale = iscale;
                cli.curr_ui = CurrentUI::MainMenu;
            }

            // Preload toggle
            if ui.checkbox(&mut cfg.touch_tile_style.preload_rasterized, l10n::tr("Preload Rasterized Icons")).changed() {
                cli.curr_ui = CurrentUI::MainMenu;
            }

            ui.separator();

            // Window alpha slider
            let mut alpha = cfg.touch_tile_style_window_alpha;
            let resp3 = ui.add(egui::Slider::new(&mut alpha, 0.0..=1.0).text(l10n::tr("Overlay Transparency")));
            if resp3.changed() {
                cfg.touch_tile_style_window_alpha = alpha;
            }

            // Small preview
            ui.add_space(6.0);
            ui.label(l10n::tr("Preview"));
            let preview_size = egui::vec2(220.0, 84.0);
            let (rect, _resp) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
            let visuals = ui.style().interact(&_resp);
            ui.painter().rect_filled(rect, cfg.touch_tile_style.corner_radius, visuals.bg_fill);
            ui.painter().text(
                rect.center_top() + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_TOP,
                l10n::tr("Tile Preview"),
                egui::FontId::proportional(13.0),
                egui::Color32::WHITE,
            );
            ui.add_space(6.0);
            if ui.add_sized([120.0, 28.0], egui::Button::new(l10n::tr("Refresh UI"))).clicked() {
                crate::client::ui::main_menu::clear_touch_menu_caches(&mut images);
                cli.curr_ui = CurrentUI::MainMenu;
            }
            });

        // write back open state
        cfg.touch_tile_style_overlay_enabled = overlay_open;

    // detect changes to style and clear caches so tiles update immediately
    let new_style = cfg.touch_tile_style.clone();
    if prev_style.as_ref() != Some(&new_style) {
        crate::client::ui::main_menu::clear_touch_menu_caches(&mut images);
        cli.curr_ui = CurrentUI::MainMenu;
    }
    *prev_style = Some(new_style);
}

impl SettingTag {
    fn color(self) -> Color32 {
        match self {
            SettingTag::Performance => Color32::from_rgb(70, 140, 255),
            SettingTag::Fun => Color32::from_rgb(180, 90, 255),
            SettingTag::Dangerous => Color32::from_rgb(255, 70, 70),
        }
    }

    fn label(self, language: &str) -> &'static str {
        match self {
            SettingTag::Performance => l10n::text(language, "settings.tag.performance"),
            SettingTag::Fun => l10n::text(language, "settings.tag.fun"),
            SettingTag::Dangerous => l10n::text(language, "settings.tag.dangerous"),
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

fn ui_setting_legend(ui: &mut Ui, language: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(l10n::text(language, "settings.legend.title"));
        for tag in [SettingTag::Performance, SettingTag::Fun, SettingTag::Dangerous] {
            ui.colored_label(tag.color(), format!("| {}", tag.label(language)));
        }
    });

    

    
    ui.small(l10n::text(language, "settings.legend.note1"));
    ui.small(l10n::text(language, "settings.legend.note2"));
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

fn ui_toggle_button(ui: &mut Ui, value: &mut bool) {
    let (label, fill) = if *value {
        (l10n::tr("Enabled"), Color32::from_rgb(56, 150, 90))
    } else {
        (l10n::tr("Disabled"), Color32::from_rgb(84, 88, 104))
    };
    let response = ui.add_sized(
        [150.0, 24.0],
        egui::Button::new(egui::RichText::new(label).strong().color(Color32::WHITE))
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, Color32::from_gray(30))),
    );
    if response.clicked() {
        *value = !*value;
    }
}

fn ui_setting_toggle(ui: &mut Ui, text: &str, value: &mut bool) {
    ui_setting_line_custom(ui, text, |ui| {
        ui_toggle_button(ui, value);
    });
}

fn ui_fog_color_palette(ui: &mut Ui, fog_color: &mut Vec3) {
    let mut rgb = [
        (fog_color.x.clamp(0.0, 1.0) * 255.0).round() as u8,
        (fog_color.y.clamp(0.0, 1.0) * 255.0).round() as u8,
        (fog_color.z.clamp(0.0, 1.0) * 255.0).round() as u8,
    ];
    let mut changed = false;

    ui.horizontal(|ui| {
        changed |= ui.color_edit_button_srgb(&mut rgb).changed();
        let preview = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        ui.colored_label(preview, l10n::tr("Preview"));
    });

    let presets: [(&str, [u8; 3]); 6] = [
        ("Neutral", [180, 188, 202]),
        ("Cold", [125, 172, 240]),
        ("Sunset", [255, 175, 120]),
        ("Toxic", [120, 220, 120]),
        ("Ash", [110, 112, 126]),
        ("Night", [70, 96, 150]),
    ];

    ui.horizontal_wrapped(|ui| {
        ui.small(l10n::tr("Palette"));
        for (name, [r, g, b]) in presets {
            let color = Color32::from_rgb(r, g, b);
            let selected = rgb == [r, g, b];
            let stroke = if selected {
                egui::Stroke::new(2.0, Color32::WHITE)
            } else {
                egui::Stroke::new(1.0, Color32::from_gray(40))
            };
            let response = ui.add_sized(
                [24.0, 18.0],
                egui::Button::new("").fill(color).stroke(stroke),
            );
            if response.clicked() {
                rgb = [r, g, b];
                changed = true;
            }
            response.on_hover_text(name);
        }
    });

    if changed {
        fog_color.x = rgb[0] as f32 / 255.0;
        fog_color.y = rgb[1] as f32 / 255.0;
        fog_color.z = rgb[2] as f32 / 255.0;
    }
}

fn draw_keyboard_mouse_diagram(ui: &mut Ui, jump: &str, sprint: &str, sneak: &str, pause: &str, keys: &ButtonInput<KeyCode>, mouse_btns: &ButtonInput<MouseButton>) {
    let side_padding = 2.0;
    let base_size = egui::vec2(580.0, 160.0);
    let total_w = ui.available_width().max(side_padding * 2.0 + 1.0);
    let inner_w = (total_w - side_padding * 2.0).max(1.0);
    let scale = inner_w / base_size.x;
    let total_h = base_size.y * scale;

    let (rect, _response) = ui.allocate_exact_size(egui::vec2(total_w, total_h), egui::Sense::hover());
    let draw_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(side_padding, 0.0),
        egui::vec2(inner_w, total_h),
    );
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(30, 35, 46);
    let key_bg = Color32::from_rgb(60, 68, 86);
    let key_highlight = Color32::from_rgb(110, 120, 140);
    let key_mapped = Color32::from_rgb(95, 170, 255);
    let key_pressed = Color32::from_rgb(255, 230, 80);

    painter.rect_filled(draw_rect, (8.0 * scale).max(2.0), bg);

    let u = 16.0 * scale;
    let gap = 4.0 * scale;
    let kbd_start = draw_rect.min + egui::vec2(20.0 * scale, 20.0 * scale);

    let board_r = egui::Rect::from_min_size(kbd_start - egui::vec2(6.0 * scale, 6.0 * scale), egui::vec2(23.0 * u + 22.0 * gap + 12.0 * scale, 6.0 * u + 5.0 * gap + 12.0 * scale));
    painter.rect_filled(board_r, (6.0 * scale).max(2.0), Color32::from_rgb(20, 24, 32));

    let is_kbd_match = |display: &str| -> bool {
        let check = |mapped: &str| -> bool {
            let m = mapped.to_ascii_lowercase();
            let d = display.to_ascii_lowercase();
            if m == d { return true; }
            if m.contains("escape") && d == "esc" { return true; }
            if m.contains("lshift") && d == "lsh" { return true; }
            if m.contains("shiftleft") && d == "lsh" { return true; }
            if m.contains("rshift") && d == "rsh" { return true; }
            if m.contains("rightshift") && d == "rsh" { return true; }
            if m.contains("lcontrol") && d == "lctrl" { return true; }
            if m.contains("controlleft") && d == "lctrl" { return true; }
            if m.contains("lalt") && d == "lalt" { return true; }
            if m.contains("space") && d == "space" { return true; }
            if m.contains("return") && d == "enter" { return true; }
            if m.contains("back") && d == "bksp" { return true; }
            if m.contains("up") && d == "^" { return true; }
            if m.contains("down") && d == "v" { return true; }
            if m.contains("left") && d == "<" { return true; }
            if m.contains("right") && d == ">" { return true; }
            false
        };
        check(jump) || check(sprint) || check(sneak) || check(pause)
    };

    let is_kbd_pressed = |display: &str| -> bool {
        let d = display.to_ascii_lowercase();
        let code = match d.as_str() {
            "w" => KeyCode::KeyW,
            "a" => KeyCode::KeyA,
            "s" => KeyCode::KeyS,
            "d" => KeyCode::KeyD,
            "q" => KeyCode::KeyQ,
            "e" => KeyCode::KeyE,
            "r" => KeyCode::KeyR,
            "t" => KeyCode::KeyT,
            "f" => KeyCode::KeyF,
            "g" => KeyCode::KeyG,
            "space" => KeyCode::Space,
            "lsh" => KeyCode::ShiftLeft,
            "rsh" => KeyCode::ShiftRight,
            "lctrl" => KeyCode::ControlLeft,
            "lalt" => KeyCode::AltLeft,
            "esc" => KeyCode::Escape,
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "^" => KeyCode::ArrowUp,
            "v" => KeyCode::ArrowDown,
            "<" => KeyCode::ArrowLeft,
            ">" => KeyCode::ArrowRight,
            _ => return false,
        };
        keys.pressed(code)
    };

    let font = egui::FontId::proportional((10.0 * scale).max(7.0));
    let mut draw_row = |row_idx: usize, keys_data: &[(&str, f32, f32)]| {
        let mut curr_x = 0.0;
        let y = (u + gap) * row_idx as f32;
        for &(text, width_u, offset_u) in keys_data {
            curr_x += offset_u * (u + gap);
            let w = width_u * u + (width_u - 1.0) * gap;
            let h = u;

            let p = kbd_start + egui::vec2(curr_x, y);
            let r = egui::Rect::from_min_size(p, egui::vec2(w, h));

            let mapped = is_kbd_match(text);
            let pressed = is_kbd_pressed(text);

            let color = if pressed { key_pressed } else if mapped { key_mapped } else { key_bg };
            let text_color = if pressed || mapped { Color32::BLACK } else { Color32::WHITE };

            let is_down = pressed;
            let draw_r = if is_down { r.translate(egui::vec2(0.0, 2.0 * scale)) } else { r };

            // 3D effect shadow
            let r_shadow = r.translate(egui::vec2(0.0, 2.0 * scale));
            painter.rect_filled(r_shadow, (3.0 * scale).max(1.0), Color32::from_rgb(15, 18, 24));

            painter.rect_filled(draw_r, (3.0 * scale).max(1.0), color);
            painter.rect_filled(draw_r.shrink((1.0 * scale).max(0.5)), (3.0 * scale).max(1.0), if pressed { key_pressed } else if mapped { key_mapped } else { key_highlight });

            painter.text(draw_r.center(), egui::Align2::CENTER_CENTER, text, font.clone(), text_color);

            curr_x += w + gap;
        }
    };

    let row0 = [("Esc",1.0,0.0), ("F1",1.0,1.0), ("F2",1.0,0.0), ("F3",1.0,0.0), ("F4",1.0,0.0), ("F5",1.0,0.5), ("F6",1.0,0.0), ("F7",1.0,0.0), ("F8",1.0,0.0), ("F9",1.0,0.5), ("F10",1.0,0.0), ("F11",1.0,0.0), ("F12",1.0,0.0),  ("Prt",1.0,0.5), ("Scr",1.0,0.0), ("Pau",1.0,0.0)];
    let row1 = [("`",1.0,0.0), ("1",1.0,0.0), ("2",1.0,0.0), ("3",1.0,0.0), ("4",1.0,0.0), ("5",1.0,0.0), ("6",1.0,0.0), ("7",1.0,0.0), ("8",1.0,0.0), ("9",1.0,0.0), ("0",1.0,0.0), ("-",1.0,0.0), ("=",1.0,0.0), ("Bksp",2.0,0.0),  ("Ins",1.0,0.5), ("Hm",1.0,0.0), ("Pu",1.0,0.0), ("Num", 1.0, 0.5), ("/", 1.0, 0.0), ("*", 1.0, 0.0), ("-", 1.0, 0.0)];
    let row2 = [("Tab",1.5,0.0), ("Q",1.0,0.0), ("W",1.0,0.0), ("E",1.0,0.0), ("R",1.0,0.0), ("T",1.0,0.0), ("Y",1.0,0.0), ("U",1.0,0.0), ("I",1.0,0.0), ("O",1.0,0.0), ("P",1.0,0.0), ("[",1.0,0.0), ("]",1.0,0.0), ("\\",1.5,0.0),  ("Del",1.0,0.5), ("End",1.0,0.0), ("Pd",1.0,0.0), ("7", 1.0, 0.5), ("8", 1.0, 0.0), ("9", 1.0, 0.0), ("+", 1.0, 0.0)];
    let row3 = [("Caps",1.75,0.0), ("A",1.0,0.0), ("S",1.0,0.0), ("D",1.0,0.0), ("F",1.0,0.0), ("G",1.0,0.0), ("H",1.0,0.0), ("J",1.0,0.0), ("K",1.0,0.0), ("L",1.0,0.0), (";",1.0,0.0), ("'",1.0,0.0), ("Enter",2.25,0.0), ("4", 1.0, 4.0), ("5", 1.0, 0.0), ("6", 1.0, 0.0)];
    let row4 = [("LSh",2.25,0.0), ("Z",1.0,0.0), ("X",1.0,0.0), ("C",1.0,0.0), ("V",1.0,0.0), ("B",1.0,0.0), ("N",1.0,0.0), ("M",1.0,0.0), (",",1.0,0.0), (".",1.0,0.0), ("/",1.0,0.0), ("RSh",2.75,0.0),  ("^",1.0,1.5), ("1", 1.0, 1.5), ("2", 1.0, 0.0), ("3", 1.0, 0.0), ("Ent", 1.0, 0.0)];
    let row5 = [("LCtrl",1.25,0.0), ("Win",1.25,0.0), ("LAlt",1.25,0.0), ("Space",6.25,0.0), ("RAlt",1.25,0.0), ("Win",1.25,0.0), ("Menu",1.25,0.0), ("RCtrl",1.25,0.0),  ("<",1.0,0.5), ("v",1.0,0.0), (">",1.0,0.0), ("0", 2.0, 0.5), (".", 1.0, 0.0)];

    draw_row(0, &row0);
    draw_row(1, &row1);
    draw_row(2, &row2);
    draw_row(3, &row3);
    draw_row(4, &row4);
    draw_row(5, &row5);

    // Mouse Base
    let is_mouse_match = |button: &str| -> bool {
        let check = |mapped: &str| -> bool {
            let m = mapped.to_ascii_lowercase();
            if m.contains("mouse") || m.contains("mb") || m.contains("click") {
                if (m.contains("left") || m.contains("1")) && button == "left" { return true; }
                if (m.contains("right") || m.contains("2")) && button == "right" { return true; }
                if (m.contains("middle") || m.contains("3")) && button == "middle" { return true; }
            }
            false
        };
        check(jump) || check(sprint) || check(sneak) || check(pause)
    };

    let is_mouse_pressed = |button: &str| -> bool {
        let code = match button {
            "left" => MouseButton::Left,
            "right" => MouseButton::Right,
            "middle" => MouseButton::Middle,
            _ => return false,
        };
        mouse_btns.pressed(code)
    };

    let mouse_rect = egui::Rect::from_min_size(kbd_start + egui::vec2(24.0 * u + 23.0 * gap + 10.0 * scale, 1.0 * (u + gap)), egui::vec2(60.0 * scale, 90.0 * scale));
    painter.rect_filled(mouse_rect.translate(egui::vec2(0.0, 3.0 * scale)), (28.0 * scale).max(8.0), Color32::from_rgb(15, 18, 24));
    painter.rect_filled(mouse_rect, (28.0 * scale).max(8.0), Color32::from_rgb(25, 30, 40));

    let lmb = egui::Rect::from_min_size(mouse_rect.min, egui::vec2(28.0 * scale, 40.0 * scale));
    let rmb = egui::Rect::from_min_size(mouse_rect.min + egui::vec2(32.0 * scale, 0.0), egui::vec2(28.0 * scale, 40.0 * scale));

    let color_mouse = |name: &str| {
        if is_mouse_pressed(name) { key_pressed }
        else if is_mouse_match(name) { key_mapped }
        else { key_bg }
    };

    // Draw left/right buttons. Translated if pressed for visual feedback
    let lmb_draw = if is_mouse_pressed("left") { lmb.translate(egui::vec2(0.0, 2.0 * scale)) } else { lmb };
    let rmb_draw = if is_mouse_pressed("right") { rmb.translate(egui::vec2(0.0, 2.0 * scale)) } else { rmb };

    painter.rect_filled(lmb_draw, (6.0 * scale).max(2.0), color_mouse("left"));
    painter.rect_filled(rmb_draw, (6.0 * scale).max(2.0), color_mouse("right"));

    painter.circle_filled(mouse_rect.min + egui::vec2(30.0 * scale, 24.0 * scale) + if is_mouse_pressed("middle") { egui::vec2(0.0, 2.0 * scale) } else { egui::Vec2::ZERO }, (5.0 * scale).max(2.0), color_mouse("middle"));
}

fn draw_gamepad_diagram(
    ui: &mut Ui,
    jump: &str,
    sprint: &str,
    use_btn: &str,
    attack: &str,
    gamepads: &Query<(Entity, &Gamepad)>,
) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(680.0, 360.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(30, 35, 46);
    let dark_bg = Color32::from_rgb(20, 24, 32);
    let shell = Color32::from_rgb(82, 92, 114);
    let shell_grip = Color32::from_rgb(70, 80, 100);
    let mapped_color = Color32::from_rgb(95, 170, 255);
    let pressed_color = Color32::from_rgb(255, 230, 80);
    let text_color = Color32::WHITE;

    painter.rect_filled(rect, 8.0, bg);

    let center = rect.center() + egui::vec2(0.0, 25.0);
    let scale = 1.45;

    let active_gamepad = gamepads.iter().next();
    let button_down = |gp: &Gamepad, btn: GamepadButton| gp.get(btn).unwrap_or(0.0) > 0.5;

    // Matcher
    let is_match = |button: &str| -> bool {
        let b = button.to_ascii_lowercase();
        let mut matched = false;
        for m in [jump, sprint, use_btn, attack] {
            let m = m.to_ascii_lowercase();
            if (b == "a" && m.contains("south"))
            || (b == "b" && m.contains("east"))
            || (b == "x" && m.contains("west"))
            || (b == "y" && m.contains("north"))
            || (b == "up" && m.contains("dpadup"))
            || (b == "down" && m.contains("dpaddown"))
            || (b == "left" && m.contains("dpadleft"))
            || (b == "right" && m.contains("dpadright"))
            || (b == "lt" && (m.contains("lefttrigger") && !m.contains("2")))
            || (b == "rt" && (m.contains("righttrigger") && !m.contains("2")))
            || (b == "lb" && m.contains("lefttrigger2"))
            || (b == "rb" && m.contains("righttrigger2"))
            || (b == "ls" && m.contains("leftthumb"))
            || (b == "rs" && m.contains("rightthumb"))
            || (b == "start" && (m.contains("start") || m.contains("menu")))
            || (b == "sel" && (m.contains("select") || m.contains("view")))
            {
                matched = true;
            }
        }
        matched
    };

    let is_pressed = |button: &str| -> bool {
        if let Some((_entity, gp)) = active_gamepad {
            let btn = match button.to_ascii_lowercase().as_str() {
                "a" => GamepadButton::South,
                "b" => GamepadButton::East,
                "x" => GamepadButton::West,
                "y" => GamepadButton::North,
                "up" => GamepadButton::DPadUp,
                "down" => GamepadButton::DPadDown,
                "left" => GamepadButton::DPadLeft,
                "right" => GamepadButton::DPadRight,
                "lt" => GamepadButton::LeftTrigger,
                "rt" => GamepadButton::RightTrigger,
                "lb" => GamepadButton::LeftTrigger2,
                "rb" => GamepadButton::RightTrigger2,
                "ls" => GamepadButton::LeftThumb,
                "rs" => GamepadButton::RightThumb,
                "start" => GamepadButton::Start,
                "sel" => GamepadButton::Select,
                _ => return false,
            };
            return button_down(gp, btn);
        }
        false
    };

    let mut draw_btn = |p: egui::Pos2, r: f32, text: &str, name: &str, base_color: Color32| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);

        let is_down = pressed;
        let mut color = if pressed { pressed_color } else if mapped { base_color } else { Color32::from_rgba_premultiplied(base_color.r()/3, base_color.g()/3, base_color.b()/3, 255) };
        if base_color == Color32::WHITE { // For neutral buttons
             color = if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg };
        }

        let p_draw = if is_down { p + egui::vec2(0.0, 2.0) } else { p };

        let p_shadow = p + egui::vec2(0.0, 2.0);
        painter.circle_filled(p_shadow, r, Color32::from_rgb(15, 18, 24));
        painter.circle_filled(p_draw, r, color);

        painter.text(p_draw, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 1.2), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    let mut draw_joystick = |p: egui::Pos2, r: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);

        let dark = Color32::from_rgb(15, 18, 24);
        let mid = if pressed { pressed_color } else if mapped { mapped_color } else { Color32::from_rgb(35, 40, 50) };
        let indent = if pressed { Color32::from_rgb(255, 255, 150) } else if mapped { Color32::from_rgb(60, 130, 200) } else { dark_bg };

        let p_shadow = p + egui::vec2(0.0, 4.0);
        let mut axis_offset = egui::vec2(0.0, 0.0);

          if let Some((_entity, gp)) = active_gamepad {
            if name == "ls" {
                    let x = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
                    let y = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
               axis_offset = egui::vec2(x, -y) * 10.0;
            } else if name == "rs" {
                    let x = gp.get(GamepadAxis::RightStickX).unwrap_or(0.0);
                    let y = gp.get(GamepadAxis::RightStickY).unwrap_or(0.0);
               axis_offset = egui::vec2(x, -y) * 10.0;
            }
        }

        let p_draw = p + axis_offset;

        painter.circle_filled(p_shadow, r, Color32::from_black_alpha(150));

        // Detailed 3D thumbstick
        painter.circle_filled(p_draw, r, dark); // base ring
        painter.circle_filled(p_draw + egui::vec2(0.0, -1.0), r - 2.0, mid); // outer rim
        painter.circle_filled(p_draw + egui::vec2(0.0, 1.0), r - 5.0, dark); // inner slope
        painter.circle_filled(p_draw, r - 7.0, indent); // thumb indent center

        painter.text(p_draw, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 0.8), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    let mut draw_rect_btn = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let r_shadow = r.translate(egui::vec2(0.0, 2.0));

        let mut draw_r = r;
        if pressed {
            draw_r = r.translate(egui::vec2(0.0, 2.0));
        }

        painter.rect_filled(r_shadow, radius, Color32::from_rgb(15, 18, 24));
        painter.rect_filled(draw_r, radius, if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg });

        if !text.is_empty() {
            painter.text(draw_r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(14.0), if mapped || pressed { Color32::BLACK } else { text_color });
        }
    };

    let mut draw_rect_btn_3d = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let shadow_col = Color32::from_rgb(15, 18, 24);
        let base_col = if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg };
        let top_col = if pressed { Color32::from_rgb(255, 255, 150) } else if mapped { Color32::from_rgb(130, 200, 255) } else { Color32::from_rgb(50, 55, 65) };

        let mut draw_r = r;
        if pressed {
            draw_r = r.translate(egui::vec2(0.0, 4.0));
        }

        // Outer shadow
        painter.rect_filled(r.translate(egui::vec2(0.0, 4.0)), radius, shadow_col);
        // Base / front face
        painter.rect_filled(draw_r, radius, base_col);
        // Top bevel face to create 3D button press look
        let mut top_r = draw_r;
        top_r.max.y = top_r.min.y + (draw_r.height() * 0.4);
        painter.rect_filled(top_r, radius * 0.8, top_col);

        if !text.is_empty() {
            painter.text(draw_r.center() + egui::vec2(0.0, 2.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), if mapped || pressed { Color32::BLACK } else { text_color });
        }
    };

    let shell = Color32::from_rgb(45, 48, 55);
    let shell_shadow = Color32::from_rgb(25, 28, 34);

    // Triggers (LT/RT) - Top view indicators above the gamepad
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-90.0 * scale, -95.0 * scale), egui::vec2(60.0 * scale, 30.0 * scale)), 4.0, "LT", "lt");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(90.0 * scale, -95.0 * scale), egui::vec2(60.0 * scale, 30.0 * scale)), 4.0, "RT", "rt");

    // Bumpers (LB/RB) - Top view indicators just below triggers
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-90.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 18.0 * scale)), 4.0, "LB", "lb");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(90.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 18.0 * scale)), 4.0, "RB", "rb");

    // Main Body (Xbox Style)
    // Create concave shape by overlapping simple primitives
    let shift_shadow = egui::vec2(0.0, 6.0);

    // Central chassis (Wider to fit all buttons)
    let core_rect = egui::Rect::from_center_size(center + egui::vec2(0.0, 15.0 * scale), egui::vec2(260.0 * scale, 90.0 * scale));
    painter.rect_filled(core_rect.translate(shift_shadow), 20.0 * scale, shell_shadow);

    // Grips (angled overlapping boxes with rounded bottoms)
    let left_grip = vec![
        center + egui::vec2(-130.0 * scale, -30.0 * scale), // top left match core
        center + egui::vec2(-50.0 * scale, 60.0 * scale),   // inner right blend to core bottom
        center + egui::vec2(-85.0 * scale, 120.0 * scale),  // grip point inner
        center + egui::vec2(-145.0 * scale, 95.0 * scale),  // grip point outer
    ];
    let right_grip = vec![
        center + egui::vec2(130.0 * scale, -30.0 * scale),
        center + egui::vec2(145.0 * scale, 95.0 * scale),
        center + egui::vec2(85.0 * scale, 120.0 * scale),
        center + egui::vec2(50.0 * scale, 60.0 * scale),
    ];

    painter.add(egui::Shape::convex_polygon(left_grip.iter().map(|p| *p + shift_shadow).collect(), shell_shadow, egui::Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(right_grip.iter().map(|p| *p + shift_shadow).collect(), shell_shadow, egui::Stroke::NONE));

    // Fill bottom grip curves (shadows)
    painter.circle_filled(center + egui::vec2(-120.0 * scale, 107.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);
    painter.circle_filled(center + egui::vec2(120.0 * scale, 107.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);

    // Main layers
    painter.rect_filled(core_rect, 20.0 * scale, shell);
    painter.add(egui::Shape::convex_polygon(left_grip, shell, egui::Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(right_grip, shell, egui::Stroke::NONE));

    // Fill bottom grip curves
    painter.circle_filled(center + egui::vec2(-120.0 * scale, 107.0 * scale), 25.0 * scale, shell);
    painter.circle_filled(center + egui::vec2(120.0 * scale, 107.0 * scale), 25.0 * scale, shell);

    // Xbox logo button surround
    painter.circle_filled(center + egui::vec2(0.0, -25.0 * scale), 25.0 * scale, Color32::from_rgb(35, 38, 45));

    // D-Pad
    let dpad_c = center + egui::vec2(-55.0 * scale, 35.0 * scale);
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, -15.0 * scale), egui::vec2(15.0 * scale, 20.0 * scale)), 2.0, "", "up");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, 15.0 * scale), egui::vec2(15.0 * scale, 20.0 * scale)), 2.0, "", "down");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(-15.0 * scale, 0.0), egui::vec2(20.0 * scale, 15.0 * scale)), 2.0, "", "left");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(15.0 * scale, 0.0), egui::vec2(20.0 * scale, 15.0 * scale)), 2.0, "", "right");
    painter.rect_filled(egui::Rect::from_center_size(dpad_c, egui::vec2(15.0 * scale, 15.0 * scale)), 0.0, dark_bg);

    // Left / Right Sticks
    draw_joystick(center + egui::vec2(-70.0 * scale, -10.0 * scale), 20.0 * scale, "LS", "ls");
    draw_joystick(center + egui::vec2(40.0 * scale, 35.0 * scale), 20.0 * scale, "RS", "rs");

    // ABXY
    draw_btn(center + egui::vec2(90.0 * scale, -30.0 * scale), 10.0 * scale, "Y", "y", Color32::from_rgb(250, 200, 10)); // Yellow
    draw_btn(center + egui::vec2(115.0 * scale, -10.0 * scale), 10.0 * scale, "B", "b", Color32::from_rgb(220, 40, 30));  // Red
    draw_btn(center + egui::vec2(65.0 * scale, -10.0 * scale), 10.0 * scale, "X", "x", Color32::from_rgb(40, 100, 230)); // Blue
    draw_btn(center + egui::vec2(90.0 * scale, 10.0 * scale), 10.0 * scale, "A", "a", Color32::from_rgb(30, 200, 60));   // Green

    // Start / Select
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(-25.0 * scale, -10.0 * scale), egui::vec2(15.0 * scale, 10.0 * scale)), 4.0, "", "sel");
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(25.0 * scale, -10.0 * scale), egui::vec2(15.0 * scale, 10.0 * scale)), 4.0, "", "start");

    // Home
    painter.circle_filled(center + egui::vec2(0.0, -30.0 * scale), 15.0 * scale, Color32::from_gray(120));
}

fn draw_gamepad_diagram_ps(ui: &mut Ui, jump: &str, sprint: &str, use_btn: &str, attack: &str,
    gamepads: &Query<(Entity, &Gamepad)>,
) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(680.0, 360.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(30, 35, 46);
    let dark_bg = Color32::from_rgb(20, 24, 32);
    let shell = Color32::from_rgb(180, 185, 195);
    let shell_grip = Color32::from_rgb(160, 165, 180);
    let shell_center = Color32::from_rgb(40, 45, 56);
    let mapped_color = Color32::from_rgb(95, 170, 255);
    let pressed_color = Color32::from_rgb(255, 230, 80);
    let text_color = Color32::WHITE;

    painter.rect_filled(rect, 8.0, bg);

    let center = rect.center() + egui::vec2(0.0, 25.0);
    let scale = 1.45;

    let active_gamepad = gamepads.iter().next();
    let button_down = |gp: &Gamepad, btn: GamepadButton| gp.get(btn).unwrap_or(0.0) > 0.5;

    let is_match = |button: &str| -> bool {
        let b = button.to_ascii_lowercase();
        let mut matched = false;
        for m in [jump, sprint, use_btn, attack] {
            let m = m.to_ascii_lowercase();
            if (b == "cross" && (m.contains("south") || m.contains("cross")))
            || (b == "circle" && (m.contains("east") || m.contains("circle")))
            || (b == "square" && (m.contains("west") || m.contains("square")))
            || (b == "triangle" && (m.contains("north") || m.contains("triangle")))
            || (b == "up" && m.contains("dpadup"))
            || (b == "down" && m.contains("dpaddown"))
            || (b == "left" && m.contains("dpadleft"))
            || (b == "right" && m.contains("dpadright"))
            || (b == "l2" && (m.contains("lefttrigger") && !m.contains("2")))
            || (b == "r2" && (m.contains("righttrigger") && !m.contains("2")))
            || (b == "l1" && m.contains("lefttrigger2"))
            || (b == "r1" && m.contains("righttrigger2"))
            || (b == "l3" && m.contains("leftthumb"))
            || (b == "r3" && m.contains("rightthumb"))
            || (b == "options" && (m.contains("start") || m.contains("options")))
            || (b == "share" && (m.contains("select") || m.contains("share")))
            {
                matched = true;
            }
        }
        matched
    };

    let is_pressed = |button: &str| -> bool {
        if let Some((_entity, gp)) = active_gamepad {
            let btn_type = match button.to_ascii_lowercase().as_str() {
                "a" | "cross" => GamepadButton::South,
                "b" | "circle" => GamepadButton::East,
                "x" | "square" => GamepadButton::West,
                "y" | "triangle" => GamepadButton::North,
                "up" => GamepadButton::DPadUp,
                "down" => GamepadButton::DPadDown,
                "left" => GamepadButton::DPadLeft,
                "right" => GamepadButton::DPadRight,
                "lt" | "l2" | "l" => GamepadButton::LeftTrigger,
                "rt" | "r2" | "r" => GamepadButton::RightTrigger,
                "lb" | "l1" => GamepadButton::LeftTrigger2,
                "rb" | "r1" => GamepadButton::RightTrigger2,
                "ls" | "l3" => GamepadButton::LeftThumb,
                "rs" | "r3" => GamepadButton::RightThumb,
                "start" | "options" => GamepadButton::Start,
                "sel" | "share" => GamepadButton::Select,
                _ => return false,
            };
            return button_down(gp, btn_type);
        }
        false
    };

    let mut draw_btn = |p: egui::Pos2, r: f32, text: &str, name: &str, base_color: Color32| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let p_shadow = p + egui::vec2(0.0, 2.0);
        painter.circle_filled(p_shadow, r, Color32::from_rgb(15, 18, 24));

        let mut color = if pressed { pressed_color } else if mapped { base_color } else { Color32::from_rgba_premultiplied(base_color.r()/3, base_color.g()/3, base_color.b()/3, 255) };
        if base_color == Color32::WHITE { // For neutral buttons
             color = if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg };
        }
        let p_draw = if pressed { p + egui::vec2(0.0, 2.0) } else { p };
        painter.circle_filled(p_draw, r, color);
        painter.text(p_draw, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 1.2), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    let mut draw_joystick = |p: egui::Pos2, r: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let dark = Color32::from_rgb(15, 18, 24);
        let mid = if pressed { pressed_color } else if mapped { mapped_color } else { Color32::from_rgb(35, 40, 50) };
        let indent = if pressed { Color32::from_rgb(255, 255, 150) } else if mapped { Color32::from_rgb(60, 130, 200) } else { dark_bg };

        let mut axis_offset = egui::vec2(0.0, 0.0);
          if let Some((_entity, gp)) = active_gamepad {
            if name == "l3" {
                    let x = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
                    let y = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
               axis_offset = egui::vec2(x, -y) * 10.0;
            } else if name == "r3" {
                    let x = gp.get(GamepadAxis::RightStickX).unwrap_or(0.0);
                    let y = gp.get(GamepadAxis::RightStickY).unwrap_or(0.0);
               axis_offset = egui::vec2(x, -y) * 10.0;
            }
        }

        let p_draw = p + axis_offset;

        let p_shadow = p + egui::vec2(0.0, 4.0);
        painter.circle_filled(p_shadow, r, Color32::from_black_alpha(150));

        painter.circle_filled(p_draw, r, dark);
        painter.circle_filled(p_draw + egui::vec2(0.0, -1.0), r - 2.0, mid);
        painter.circle_filled(p_draw + egui::vec2(0.0, 1.0), r - 5.0, dark);
        painter.circle_filled(p_draw, r - 7.0, indent);

        painter.text(p_draw, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 0.8), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    let mut draw_rect_btn = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let mut draw_r = r;
        if pressed { draw_r = r.translate(egui::vec2(0.0, 2.0)); }

        let r_shadow = r.translate(egui::vec2(0.0, 2.0));
        painter.rect_filled(r_shadow, radius, Color32::from_rgb(15, 18, 24));
        painter.rect_filled(draw_r, radius, if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg });
        if !text.is_empty() {
            painter.text(draw_r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(14.0), if mapped || pressed { Color32::BLACK } else { text_color });
        }
    };

    let mut draw_rect_btn_3d = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let shadow_col = Color32::from_rgb(15, 18, 24);
        let base_col = if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg };
        let top_col = if pressed { Color32::from_rgb(255, 255, 150) } else if mapped { Color32::from_rgb(130, 200, 255) } else { Color32::from_rgb(50, 55, 65) };

        let mut draw_r = r;
        if pressed { draw_r = r.translate(egui::vec2(0.0, 4.0)); }

        painter.rect_filled(r.translate(egui::vec2(0.0, 4.0)), radius, shadow_col);
        painter.rect_filled(draw_r, radius, base_col);

        let mut top_r = draw_r;
        top_r.max.y = top_r.min.y + (draw_r.height() * 0.4);
        painter.rect_filled(top_r, radius * 0.8, top_col);

        if !text.is_empty() {
            painter.text(draw_r.center() + egui::vec2(0.0, 2.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), if mapped || pressed { Color32::BLACK } else { text_color });
        }
    };

    // Triggers (L2 / R2) - Top view indicators above the gamepad
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-85.0 * scale, -95.0 * scale), egui::vec2(60.0 * scale, 30.0 * scale)), 4.0, "L2", "l2");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(85.0 * scale, -95.0 * scale), egui::vec2(60.0 * scale, 30.0 * scale)), 4.0, "R2", "r2");

    // Bumpers (L1 / R1) - Top view indicators just below triggers
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-85.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 18.0 * scale)), 4.0, "L1", "l1");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(85.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 18.0 * scale)), 4.0, "R1", "r1");

    // Slanted Body Silhouette (PlayStation style)
    let shell = Color32::from_rgb(180, 185, 195);
    let shell_shadow = Color32::from_rgb(140, 145, 155);

    let shift_shadow = egui::vec2(0.0, 6.0);

    // Central chassis (Wider to fit all buttons)
    let core_rect = egui::Rect::from_center_size(center + egui::vec2(0.0, 15.0 * scale), egui::vec2(250.0 * scale, 85.0 * scale));
    painter.rect_filled(core_rect.translate(shift_shadow), 20.0 * scale, shell_shadow);

    let left_grip = vec![
        center + egui::vec2(-125.0 * scale, -25.0 * scale),
        center + egui::vec2(-45.0 * scale, 50.0 * scale),
        center + egui::vec2(-75.0 * scale, 110.0 * scale),
        center + egui::vec2(-135.0 * scale, 100.0 * scale),
    ];
    let right_grip = vec![
        center + egui::vec2(125.0 * scale, -25.0 * scale),
        center + egui::vec2(135.0 * scale, 100.0 * scale),
        center + egui::vec2(75.0 * scale, 110.0 * scale),
        center + egui::vec2(45.0 * scale, 50.0 * scale),
    ];

    painter.add(egui::Shape::convex_polygon(left_grip.iter().map(|p| *p + shift_shadow).collect(), shell_shadow, egui::Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(right_grip.iter().map(|p| *p + shift_shadow).collect(), shell_shadow, egui::Stroke::NONE));

    // Shadows for grip drops
    painter.circle_filled(center + egui::vec2(-105.0 * scale, 105.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);
    painter.circle_filled(center + egui::vec2(105.0 * scale, 105.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);

    painter.rect_filled(core_rect, 20.0 * scale, shell);
    painter.add(egui::Shape::convex_polygon(left_grip, shell, egui::Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(right_grip, shell, egui::Stroke::NONE));

    // Fill in top gap to make it straight
    painter.rect_filled(egui::Rect::from_min_max(center + egui::vec2(-60.0 * scale, -15.0 * scale), center + egui::vec2(60.0 * scale, 0.0)), 0.0, shell);

    // Fill grip round ends
    painter.circle_filled(center + egui::vec2(-105.0 * scale, 105.0 * scale), 25.0 * scale, shell);
    painter.circle_filled(center + egui::vec2(105.0 * scale, 105.0 * scale), 25.0 * scale, shell);

    // Center Touchpad Area
    painter.rect_filled(egui::Rect::from_center_size(center + egui::vec2(0.0, -10.0 * scale), egui::vec2(80.0 * scale, 50.0 * scale)), 8.0, shell_center);

    // D-Pad (Separated buttons for PS)
    let dpad_c = center + egui::vec2(-75.0 * scale, -10.0 * scale);
    painter.circle_filled(dpad_c, 25.0 * scale, Color32::from_rgb(160, 165, 180));
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, -15.0 * scale), egui::vec2(12.0 * scale, 14.0 * scale)), 3.0, "", "up");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, 15.0 * scale), egui::vec2(12.0 * scale, 14.0 * scale)), 3.0, "", "down");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(-15.0 * scale, 0.0), egui::vec2(14.0 * scale, 12.0 * scale)), 3.0, "", "left");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(15.0 * scale, 0.0), egui::vec2(14.0 * scale, 12.0 * scale)), 3.0, "", "right");

    // Left / Right Sticks (Symmetrical, now 3D)
    draw_joystick(center + egui::vec2(-40.0 * scale, 35.0 * scale), 20.0 * scale, "L3", "l3");
    draw_joystick(center + egui::vec2(40.0 * scale, 35.0 * scale), 20.0 * scale, "R3", "r3");

    // Face Buttons
    let face_c = center + egui::vec2(75.0 * scale, -10.0 * scale);
    draw_btn(face_c + egui::vec2(0.0, -20.0 * scale), 9.0 * scale, "^", "triangle", Color32::from_rgb(0, 200, 150)); // Green triangle
    draw_btn(face_c + egui::vec2(20.0 * scale, 0.0), 9.0 * scale, "O", "circle", Color32::from_rgb(220, 40, 50));    // Red circle
    draw_btn(face_c + egui::vec2(-20.0 * scale, 0.0), 9.0 * scale, "[]", "square", Color32::from_rgb(200, 100, 200));// Pink square
    draw_btn(face_c + egui::vec2(0.0, 20.0 * scale), 9.0 * scale, "X", "cross", Color32::from_rgb(80, 140, 250));    // Blue cross

    // Share / Options
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(-50.0 * scale, -30.0 * scale), egui::vec2(8.0 * scale, 15.0 * scale)), 2.0, "", "share");
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(50.0 * scale, -30.0 * scale), egui::vec2(8.0 * scale, 15.0 * scale)), 2.0, "", "options");

    // PS Button
    painter.circle_filled(center + egui::vec2(0.0, 30.0 * scale), 8.0 * scale, dark_bg);
}

fn draw_gamepad_diagram_retro(ui: &mut Ui, jump: &str, sprint: &str, use_btn: &str, attack: &str,
    gamepads: &Query<(Entity, &Gamepad)>,
) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(680.0, 320.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(30, 35, 46);
    let dark_bg = Color32::from_rgb(40, 40, 48);
    let shell = Color32::from_rgb(200, 205, 210); // Light gray SNES style
    let shell_center = Color32::from_rgb(180, 185, 195);
    let mapped_color = Color32::from_rgb(95, 170, 255);
    let pressed_color = Color32::from_rgb(255, 230, 80);
    let text_color = Color32::WHITE;

    painter.rect_filled(rect, 8.0, bg);

    let center = rect.center() + egui::vec2(0.0, 20.0);
    let scale = 1.45;

    let active_gamepad = gamepads.iter().next();
    let button_down = |gp: &Gamepad, btn: GamepadButton| gp.get(btn).unwrap_or(0.0) > 0.5;

    let is_match = |button: &str| -> bool {
        let b = button.to_ascii_lowercase();
        let mut matched = false;
        for m in [jump, sprint, use_btn, attack] {
            let m = m.to_ascii_lowercase();
            if (b == "b" && m.contains("south"))
            || (b == "a" && m.contains("east"))
            || (b == "y" && m.contains("west"))
            || (b == "x" && m.contains("north"))
            || (b == "up" && m.contains("dpadup"))
            || (b == "down" && m.contains("dpaddown"))
            || (b == "left" && m.contains("dpadleft"))
            || (b == "right" && m.contains("dpadright"))
            || (b == "l" && (m.contains("lefttrigger") || m.contains("leftbumper")))
            || (b == "r" && (m.contains("righttrigger") || m.contains("rightbumper")))
            || (b == "start" && (m.contains("start") || m.contains("menu")))
            || (b == "sel" && (m.contains("select") || m.contains("view")))
            {
                matched = true;
            }
        }
        matched
    };

    let is_pressed = |button: &str| -> bool {
        if let Some((_entity, gp)) = active_gamepad {
            let btn_type = match button.to_ascii_lowercase().as_str() {
                "b" | "south" => GamepadButton::South,
                "a" | "east" => GamepadButton::East,
                "y" | "west" => GamepadButton::West,
                "x" | "north" => GamepadButton::North,
                "up" => GamepadButton::DPadUp,
                "down" => GamepadButton::DPadDown,
                "left" => GamepadButton::DPadLeft,
                "right" => GamepadButton::DPadRight,
                "l" | "lt" | "l1" | "l2" => GamepadButton::LeftTrigger, // Simplification for Retro
                "r" | "rt" | "r1" | "r2" => GamepadButton::RightTrigger,
                "start" | "menu" => GamepadButton::Start,
                "sel" | "select" | "view" => GamepadButton::Select,
                _ => return false,
            };
            return button_down(gp, btn_type);
        }
        false
    };

    let mut draw_btn = |p: egui::Pos2, r: f32, text: &str, name: &str, base_color: Color32| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let p_shadow = p + egui::vec2(0.0, 2.0);
        painter.circle_filled(p_shadow, r, Color32::from_rgb(15, 18, 24));
        let mut color = if pressed { pressed_color } else if mapped { base_color } else { Color32::from_rgba_premultiplied(base_color.r()/3, base_color.g()/3, base_color.b()/3, 255) };
        if base_color == Color32::WHITE { // For neutral buttons
             color = if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg };
        }
        let p_draw = if pressed { p + egui::vec2(0.0, 2.0) } else { p };
        painter.circle_filled(p_draw, r, color);
        painter.text(p_draw, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 1.2), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    let mut draw_rect_btn = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let pressed = is_pressed(name);
        let r_shadow = r.translate(egui::vec2(0.0, 2.0));
        let mut draw_r = r;
        if pressed { draw_r = r.translate(egui::vec2(0.0, 2.0)); }
        painter.rect_filled(r_shadow, radius, Color32::from_rgb(15, 18, 24));
        painter.rect_filled(draw_r, radius, if pressed { pressed_color } else if mapped { mapped_color } else { dark_bg });
        painter.text(draw_r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), if mapped || pressed { Color32::BLACK } else { text_color });
    };

    // Bumpers (L / R)
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(-75.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 15.0 * scale)), 5.0, "L", "l");
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(75.0 * scale, -65.0 * scale), egui::vec2(60.0 * scale, 15.0 * scale)), 5.0, "R", "r");

    // Main Dogbone Body
    let body_rect = egui::Rect::from_center_size(center, egui::vec2(250.0 * scale, 65.0 * scale));
    painter.rect_filled(body_rect.translate(egui::vec2(0.0, 3.0)), 32.5 * scale, Color32::from_rgb(15, 18, 24));
    painter.rect_filled(body_rect, 32.5 * scale, shell);

    // Center indented area
    painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(100.0 * scale, 55.0 * scale)), 8.0, shell_center);

    // D-Pad (Cross shaped)
    let dpad_c = center + egui::vec2(-70.0 * scale, 0.0);
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, -12.0 * scale), egui::vec2(10.0 * scale, 14.0 * scale)), 1.0, "", "up");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(0.0, 12.0 * scale), egui::vec2(10.0 * scale, 14.0 * scale)), 1.0, "", "down");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(-12.0 * scale, 0.0), egui::vec2(14.0 * scale, 10.0 * scale)), 1.0, "", "left");
    draw_rect_btn(egui::Rect::from_center_size(dpad_c + egui::vec2(12.0 * scale, 0.0), egui::vec2(14.0 * scale, 10.0 * scale)), 1.0, "", "right");
    painter.rect_filled(egui::Rect::from_center_size(dpad_c, egui::vec2(10.0 * scale, 10.0 * scale)), 0.0, dark_bg);

    // Face Buttons (Diamond)
    let face_c = center + egui::vec2(70.0 * scale, 0.0);
    draw_btn(face_c + egui::vec2(0.0, -15.0 * scale), 8.0 * scale, "X", "x", Color32::from_rgb(40, 100, 230)); // Top
    draw_btn(face_c + egui::vec2(15.0 * scale, 0.0), 8.0 * scale, "A", "a", Color32::from_rgb(220, 40, 30));  // Right
    draw_btn(face_c + egui::vec2(-15.0 * scale, 0.0), 8.0 * scale, "Y", "y", Color32::from_rgb(30, 200, 60)); // Left
    draw_btn(face_c + egui::vec2(0.0, 15.0 * scale), 8.0 * scale, "B", "b", Color32::from_rgb(250, 200, 10));  // Bottom

    // Start / Select (Slanted)
    let sel_rect = egui::Rect::from_center_size(center + egui::vec2(-18.0 * scale, 5.0 * scale), egui::vec2(15.0 * scale, 6.0 * scale));
    let st_rect = egui::Rect::from_center_size(center + egui::vec2(18.0 * scale, 5.0 * scale), egui::vec2(15.0 * scale, 6.0 * scale));
    draw_rect_btn(sel_rect, 3.0, "", "sel");
    draw_rect_btn(st_rect, 3.0, "", "start");
}

fn draw_stick_debug_scope(ui: &mut Ui, title: &str, x: f32, y: f32, dead_zone: f32) {
    let width = (ui.available_width() * 0.5 - 10.0).clamp(160.0, 220.0);
    let size = egui::vec2(width, width + 34.0);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    let plot_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + width));
    let center = plot_rect.center();
    let radius = plot_rect.width() * 0.38;

    let magnitude = (x * x + y * y).sqrt().clamp(0.0, 1.0);
    let dead_clamped = dead_zone.clamp(0.0, 0.95);
    let in_dead_zone = magnitude <= dead_clamped;

    painter.rect_filled(rect, 8.0, Color32::from_rgb(24, 29, 38));
    painter.text(
        egui::pos2(rect.min.x + 10.0, rect.min.y + 8.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(13.0),
        Color32::from_rgb(185, 220, 255),
    );

    painter.circle_stroke(center, radius, egui::Stroke::new(1.5, Color32::from_rgb(110, 125, 145)));
    painter.circle_filled(center, radius * dead_clamped, Color32::from_rgba_premultiplied(255, 220, 90, 50));
    painter.line_segment(
        [egui::pos2(center.x - radius, center.y), egui::pos2(center.x + radius, center.y)],
        egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
    );
    painter.line_segment(
        [egui::pos2(center.x, center.y - radius), egui::pos2(center.x, center.y + radius)],
        egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
    );

    let marker = center + egui::vec2(x, -y) * radius;
    let marker_color = if in_dead_zone {
        Color32::from_rgb(255, 195, 85)
    } else {
        Color32::from_rgb(90, 230, 140)
    };
    painter.circle_filled(marker, 5.5, marker_color);
    painter.circle_stroke(marker, 8.0, egui::Stroke::new(1.0, marker_color.gamma_multiply(0.65)));

    let status = if in_dead_zone { "Inside dead zone" } else { "Input active" };
    painter.text(
        egui::pos2(rect.min.x + 10.0, rect.min.y + width + 7.0),
        egui::Align2::LEFT_TOP,
        format!("X {x:+.2}  Y {y:+.2}  |V| {:.2}  {}", magnitude, status),
        egui::FontId::proportional(11.0),
        Color32::from_rgb(210, 220, 235),
    );
}

fn ui_gamepad_debug_panel(
    ui: &mut Ui,
    gamepad_cfg: &mut crate::client::settings::GamepadControlsConfig,
    gamepads: &Query<(Entity, &Gamepad)>,
    rumble_requests: &mut MessageWriter<GamepadRumbleRequest>,
) {
    let active_gamepad = gamepads.iter().next();

    let axis = |axis: GamepadAxis| -> f32 {
        active_gamepad
            .and_then(|(_entity, gp)| gp.get(axis))
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0)
    };

    let lsx = axis(GamepadAxis::LeftStickX);
    let lsy = axis(GamepadAxis::LeftStickY);
    let rsx = axis(GamepadAxis::RightStickX);
    let rsy = axis(GamepadAxis::RightStickY);
    let lz = axis(GamepadAxis::LeftZ);
    let rz = axis(GamepadAxis::RightZ);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.colored_label(Color32::from_rgb(160, 210, 255), "Gamepad Debug");
            if active_gamepad.is_some() {
                ui.colored_label(Color32::from_rgb(100, 230, 135), "Device connected");
            } else {
                ui.colored_label(Color32::from_rgb(255, 190, 110), "No gamepad detected");
            }
        });

        ui.small("Live axis visualizer + rumble diagnostics.");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            draw_stick_debug_scope(ui, "Left Stick", lsx, lsy, gamepad_cfg.left_stick_dead_zone);
            draw_stick_debug_scope(ui, "Right Stick", rsx, rsy, gamepad_cfg.right_stick_dead_zone);
        });

        ui.label(
            egui::RichText::new(format!("LZ: {lz:+.2} | RZ: {rz:+.2} | LS dead zone: {:.2} | RS dead zone: {:.2}", gamepad_cfg.left_stick_dead_zone, gamepad_cfg.right_stick_dead_zone))
                .color(Color32::from_rgb(210, 220, 235))
                .size(12.0),
        );

        ui.separator();
        ui.checkbox(&mut gamepad_cfg.rumble_debug_enabled, "Enable rumble debug");
        ui.add_enabled_ui(gamepad_cfg.rumble_debug_enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Weak motor");
                ui.add(egui::Slider::new(&mut gamepad_cfg.rumble_weak_motor, 0.0..=1.0));
                ui.label("Strong motor");
                ui.add(egui::Slider::new(&mut gamepad_cfg.rumble_strong_motor, 0.0..=1.0));
            });

            ui.horizontal(|ui| {
                ui.label("Duration (ms)");
                ui.add(egui::Slider::new(&mut gamepad_cfg.rumble_duration_ms, 30..=2500));

                egui::ComboBox::from_id_source("rumble_preset_picker")
                    .selected_text(match gamepad_cfg.rumble_preset {
                        1 => "Short Tap",
                        2 => "Heavy Pulse",
                        3 => "Weak Buzz",
                        _ => "Custom",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut gamepad_cfg.rumble_preset, 0, "Custom");
                        ui.selectable_value(&mut gamepad_cfg.rumble_preset, 1, "Short Tap");
                        ui.selectable_value(&mut gamepad_cfg.rumble_preset, 2, "Heavy Pulse");
                        ui.selectable_value(&mut gamepad_cfg.rumble_preset, 3, "Weak Buzz");
                    });
            });
        });

        ui.horizontal(|ui| {
            let can_rumble = active_gamepad.is_some() && gamepad_cfg.rumble_debug_enabled;

            if ui.add_enabled(can_rumble, egui::Button::new("Test Selected Preset")).clicked() {
                if let Some((gamepad, _gp)) = active_gamepad {
                    let (weak, strong, dur_ms) = match gamepad_cfg.rumble_preset {
                        1 => (0.30, 0.65, 140),
                        2 => (0.80, 1.00, 650),
                        3 => (0.60, 0.12, 900),
                        _ => (
                            gamepad_cfg.rumble_weak_motor,
                            gamepad_cfg.rumble_strong_motor,
                            gamepad_cfg.rumble_duration_ms,
                        ),
                    };

                    rumble_requests.write(GamepadRumbleRequest::Add {
                        gamepad,
                        intensity: GamepadRumbleIntensity {
                            weak_motor: weak.clamp(0.0, 1.0),
                            strong_motor: strong.clamp(0.0, 1.0),
                        },
                        duration: Duration::from_secs_f32((dur_ms as f32 / 1000.0).clamp(0.03, 2.5)),
                    });
                }
            }

            if ui.add_enabled(active_gamepad.is_some(), egui::Button::new("Stop Rumble")).clicked() {
                if let Some((gamepad, _gp)) = active_gamepad {
                    rumble_requests.write(GamepadRumbleRequest::Stop { gamepad });
                }
            }
        });
    });
}

fn ui_controls_device_map(ui: &mut Ui, cfg: &ClientSettings,
    keys: &ButtonInput<KeyCode>,
    mouse_btns: &ButtonInput<MouseButton>,
    gamepads: &Query<(Entity, &Gamepad)>,
) {
    let mut gamepad_style = ui.ctx().data_mut(|d| d.get_temp::<usize>(egui::Id::new("gamepad_style")).unwrap_or(0));

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(l10n::tr("Control Map"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_source("gamepad_style_picker")
                    .selected_text(match gamepad_style {
                        0 => "Xbox Layout",
                        1 => "PlayStation Layout",
                        2 => "Retro Layout",
                        _ => "Xbox Layout"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut gamepad_style, 0, "Xbox Layout");
                        ui.selectable_value(&mut gamepad_style, 1, "PlayStation Layout");
                        ui.selectable_value(&mut gamepad_style, 2, "Retro Layout");
                    });
                ui.label(l10n::tr("Gamepad Style:"));
            });
        });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("gamepad_style"), gamepad_style));

        ui.small(l10n::tr("Procedural UI graphics for device mapping."));
        ui.add_space(6.0);

        ui.vertical(|ui| {
            ui.vertical(|ui| {
                ui.colored_label(Color32::from_rgb(160, 210, 255), l10n::tr("Keyboard + Mouse"));
                draw_keyboard_mouse_diagram(
                    ui,
                    &cfg.controls.keyboard_mouse.key_jump,
                    &cfg.controls.keyboard_mouse.key_sprint,
                    &cfg.controls.keyboard_mouse.key_sneak,
                    &cfg.controls.keyboard_mouse.key_pause,
                    keys,
                    mouse_btns
                );

                let info_font = egui::FontId::proportional(12.0);
                let color = Color32::from_rgb(215, 223, 240);
                ui.label(egui::RichText::new(format!("Jump: {} | Sprint: {}", cfg.controls.keyboard_mouse.key_jump, cfg.controls.keyboard_mouse.key_sprint)).font(info_font.clone()).color(color));
                ui.label(egui::RichText::new(format!("Sneak: {} | Pause: {}", cfg.controls.keyboard_mouse.key_sneak, cfg.controls.keyboard_mouse.key_pause)).font(info_font).color(color));
            });

            ui.add_space(16.0);
            let sep_rect = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover()).0;
            ui.painter().rect_filled(sep_rect, 0.0, Color32::from_white_alpha(30));
            ui.add_space(16.0);

            ui.vertical(|ui| {
                ui.colored_label(Color32::from_rgb(160, 210, 255), l10n::tr("Gamepad"));

                match gamepad_style {
                    1 => draw_gamepad_diagram_ps(
                            ui,
                            &cfg.controls.gamepad.button_jump,
                            &cfg.controls.gamepad.button_sprint,
                            &cfg.controls.gamepad.button_use,
                            &cfg.controls.gamepad.button_attack,
                            gamepads,
                        ),
                    2 => draw_gamepad_diagram_retro(
                            ui,
                            &cfg.controls.gamepad.button_jump,
                            &cfg.controls.gamepad.button_sprint,
                            &cfg.controls.gamepad.button_use,
                            &cfg.controls.gamepad.button_attack,
                            gamepads,
                        ),
                    _ => draw_gamepad_diagram(
                            ui,
                            &cfg.controls.gamepad.button_jump,
                            &cfg.controls.gamepad.button_sprint,
                            &cfg.controls.gamepad.button_use,
                            &cfg.controls.gamepad.button_attack,
                            gamepads,
                        ),
                }

                let info_font = egui::FontId::proportional(12.0);
                let color = Color32::from_rgb(215, 223, 240);
                ui.label(egui::RichText::new(format!("Jump: {} | Sprint: {}", cfg.controls.gamepad.button_jump, cfg.controls.gamepad.button_sprint)).font(info_font.clone()).color(color));
                ui.label(egui::RichText::new(format!("Use: {} | Attack: {}", cfg.controls.gamepad.button_use, cfg.controls.gamepad.button_attack)).font(info_font).color(color));
            });
        });
    });
}

pub fn ui_settings(
    mut ctx: EguiContexts,
    mut settings_panel: Local<SettingsPanel>,

    mut cli: ResMut<ClientInfo>,
    mut cfg: ResMut<ClientSettings>,
    mut worldinfo: Option<ResMut<WorldInfo>>,
    mut images: ResMut<Assets<Image>>,
    mut prev_touch_style: Local<Option<crate::client::settings::TouchTileStyle>>,
    //mut egui_settings: ResMut<EguiSettings>,
    mut query_char: Query<&mut CharacterController>,
    // chunk_sys: Option<ResMut<ClientChunkSystem>>,
    mut vox_brush: ResMut<crate::voxel::VoxelBrush>,
    items: Res<crate::item::Items>,
    // mut global_volume: ResMut<GlobalVolume>,

    // mut cmds: Commands,
    // asset_server: Res<AssetServer>,
    // mut materials: ResMut<Assets<StandardMaterial>>,

    keys: Res<ButtonInput<KeyCode>>,
    mouse_btns: Res<ButtonInput<MouseButton>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut rumble_requests: MessageWriter<GamepadRumbleRequest>,
) {
    let is_world_loaded = worldinfo.is_some();
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    cfg.language = l10n::normalize_language(&cfg.language).to_string();
    let current_lang = cfg.language.clone();
    let nav_general = l10n::text(&current_lang, "settings.nav.general");
    let nav_current_world = l10n::text(&current_lang, "settings.nav.current_world");
    let nav_graphics = l10n::text(&current_lang, "settings.nav.graphics");
    let nav_audio = l10n::text(&current_lang, "settings.nav.audio");
    let nav_controls = l10n::text(&current_lang, "settings.nav.controls");
    let nav_languages = l10n::text(&current_lang, "settings.nav.languages");
    let nav_mods = l10n::text(&current_lang, "settings.nav.mods");
    let nav_assets = l10n::text(&current_lang, "settings.nav.assets");

    new_egui_window(l10n::text(&current_lang, "window.settings")).show(ctx_mut, |ui| {
        let curr_settings_panel = *settings_panel;

        ui_lr_panel(
            ui,
            true,
            |ui| {
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::General, nav_general));
                if is_world_loaded {
                    sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::CurrentWorld, nav_current_world));
                }
                ui.separator();
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Graphics, nav_graphics));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Audio, nav_audio));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Controls, nav_controls));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Language, nav_languages));
                ui.separator();
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Mods, nav_mods));
                sfx_play(ui.selectable_value(&mut *settings_panel, SettingsPanel::Assets, nav_assets));
            },
            |ui| {
                ui.style_mut().spacing.item_spacing.y = 12.;

                ui.add_space(16.);
                ui_setting_legend(ui, &current_lang);
                ui.separator();

                match curr_settings_panel {
                    SettingsPanel::General => {
                        ui.label(l10n::tr("Profile"));
                        ui_setting_line(ui, l10n::tr("Username"), egui::TextEdit::singleline(&mut cfg.username));
                        ui_setting_toggle(ui, l10n::tr("Touch UI (large buttons)"), &mut cfg.touch_ui);
                        ui_setting_toggle(ui, l10n::tr("Touch Tile Style Overlay"), &mut cfg.touch_tile_style_overlay_enabled);

                        ui.separator();
                        ui.label(l10n::tr("World Streaming (Basic)"));
                        ui_setting_line(ui, l10n::tr("Chunk Load Distance X"), egui::Slider::new(&mut cfg.chunks_load_distance.x, 2..=64));
                        ui_setting_line(ui, l10n::tr("Chunk Load Distance Y"), egui::Slider::new(&mut cfg.chunks_load_distance.y, 1..=32));
                        ui_setting_toggle(ui, l10n::tr("Surface-First Meshing"), &mut cfg.surface_first_meshing);
                        ui_setting_toggle(ui, l10n::tr("Surface-Only (No Full Upgrade)"), &mut cfg.surface_only_meshing);
                        ui_setting_toggle(ui, l10n::tr("GPU WorldGen"), &mut cfg.gpu_worldgen);
                        ui_setting_toggle(ui, l10n::tr("Allow GPU On Persisted Worlds"), &mut cfg.gpu_worldgen_allow_persisted_world);

                        ui_setting_line_custom(ui, l10n::tr("Default Terrain For New Worlds"), |ui| {
                            let mode = &mut cfg.terrain_mode;
                            let planet = *mode == crate::voxel::WorldTerrainMode::Planet;
                            let flat = *mode == crate::voxel::WorldTerrainMode::Flat;
                            let superflat = *mode == crate::voxel::WorldTerrainMode::SuperFlat;
                            if ui.radio(planet, l10n::tr("Spherical Planet")).clicked() {
                                *mode = crate::voxel::WorldTerrainMode::Planet;
                            }
                            if ui.radio(flat, l10n::tr("Flat World")).clicked() {
                                *mode = crate::voxel::WorldTerrainMode::Flat;
                            }
                            if ui.radio(superflat, l10n::tr("SuperFlat World")).clicked() {
                                *mode = crate::voxel::WorldTerrainMode::SuperFlat;
                            }
                        });

                        ui_setting_line_custom(ui, l10n::tr("Reset Recommended WorldGen Values"), |ui| {
                            if ui.button(l10n::tr("Reset")).clicked() {
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

                        egui::CollapsingHeader::new(l10n::tr("Advanced GPU WorldGen Tuning"))
                            .default_open(false)
                            .show(ui, |ui| {
                                ui_setting_line(ui, l10n::tr("GPU WorldGen Batch Size"), egui::Slider::new(&mut cfg.gpu_worldgen_batch_size, 1..=128));
                                ui_setting_line(ui, l10n::tr("GPU Max Loading Window"), egui::Slider::new(&mut cfg.gpu_worldgen_max_loading, 16..=1024));
                                ui_setting_line(ui, l10n::tr("CPU Max Loading Window"), egui::Slider::new(&mut cfg.cpu_worldgen_max_loading, 1..=64));
                                ui_setting_line(ui, l10n::tr("Adaptive Backlog Mid"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_backlog_mid, 1..=1024));
                                ui_setting_line(ui, l10n::tr("Adaptive Backlog High"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_backlog_high, 1..=2048));
                                ui_setting_line(ui, l10n::tr("Adaptive Multiplier Low"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_low, 1..=16));
                                ui_setting_line(ui, l10n::tr("Adaptive Multiplier Mid"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_mid, 1..=32));
                                ui_setting_line(ui, l10n::tr("Adaptive Multiplier High"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_mult_high, 1..=64));
                                ui_setting_line(ui, l10n::tr("Adaptive Batch Min"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_batch_min, 1..=512));
                                ui_setting_line(ui, l10n::tr("Adaptive Batch Max"), egui::Slider::new(&mut cfg.gpu_worldgen_adaptive_batch_max, 1..=2048));
                                ui.small(l10n::tr("Higher backlog usually means larger GPU batch and wider loading windows."));
                            });

                        ui.separator();
                        ui.label(l10n::tr("Video"));
                        ui_setting_line(ui, l10n::tr("FOV"), egui::Slider::new(&mut cfg.fov, 10.0..=170.0));
                        ui_setting_toggle(ui, l10n::tr("VSync"), &mut cfg.vsync);

                        ui.separator();
                        ui.label(l10n::tr("UI"));
                        ui_setting_line(ui, l10n::tr("HUD Padding"), egui::Slider::new(&mut cfg.hud_padding, 0.0..=48.0));
                        ui_setting_line(
                            ui,
                            l10n::tr("Touch Main Menu Tile Overlay"),
                            egui::Slider::new(&mut cfg.touch_menu_tile_overlay_strength, 0.0..=0.9),
                        );
                        egui::CollapsingHeader::new(l10n::tr("Touch Tile Style"))
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(l10n::tr("Background Mode"));
                                    let is_cover = matches!(cfg.touch_tile_style.background_mode, crate::client::settings::TileBackgroundMode::Cover);
                                    if ui.radio(is_cover, l10n::tr("Cover (fill)")) .clicked() {
                                        cfg.touch_tile_style.background_mode = crate::client::settings::TileBackgroundMode::Cover;
                                    }
                                    if ui.radio(!is_cover, l10n::tr("Contain (fit)")) .clicked() {
                                        cfg.touch_tile_style.background_mode = crate::client::settings::TileBackgroundMode::Contain;
                                    }
                                });

                                ui_setting_line(ui, l10n::tr("Corner Radius"), egui::Slider::new(&mut cfg.touch_tile_style.corner_radius, 0.0..=24.0));
                                ui_setting_line(ui, l10n::tr("Icon Scale"), egui::Slider::new(&mut cfg.touch_tile_style.icon_scale, 0.5..=2.0));
                                ui_setting_toggle(ui, l10n::tr("Preload Rasterized Icons"), &mut cfg.touch_tile_style.preload_rasterized);

                                // Preview
                                ui.add_space(6.0);
                                ui.label(l10n::tr("Preview"));
                                let preview_size = egui::vec2(220.0, 120.0);
                                let (rect, _resp) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
                                let visuals = ui.style().interact(&_resp);
                                ui.painter().rect_filled(rect, cfg.touch_tile_style.corner_radius, visuals.bg_fill);
                                ui.painter().text(
                                    rect.center_top() + egui::vec2(0.0, 12.0),
                                    egui::Align2::CENTER_TOP,
                                    l10n::tr("Tile Preview"),
                                    egui::FontId::proportional(14.0),
                                    egui::Color32::WHITE,
                                );
                            });
                        ui_setting_toggle(ui, l10n::tr("Show Level Indicator"), &mut cfg.show_level_indicator);
                        ui_setting_toggle(ui, l10n::tr("Show Pitch Indicator"), &mut cfg.show_pitch_indicator);
                    }
                    SettingsPanel::CurrentWorld => {
                        ui.label(l10n::tr("World"));
                        if let Some(worldinfo) = &mut worldinfo {
                            ui_setting_line(ui, l10n::tr("Day Time"), egui::Slider::new(&mut worldinfo.daytime, 0.0..=1.0));
                            ui_setting_line(ui, l10n::tr("Day Time Length"), egui::Slider::new(&mut worldinfo.daytime_length, 0.0..=60.0 * 24.0));
                        }

                        ui.separator();
                        ui.label(l10n::tr("Voxel Brush"));
                        ui_setting_line(ui, l10n::tr("Size"), egui::Slider::new(&mut vox_brush.size, 0.0..=20.0));
                        ui_setting_line(ui, l10n::tr("Intensity"), egui::Slider::new(&mut vox_brush.strength, 0.0..=1.0));
                        ui_setting_line(ui, l10n::tr("Tex"), egui::Slider::new(&mut vox_brush.tex, 0..=25));

                        ui.separator();
                        ui.label(l10n::tr("Character"));
                        if let Ok(mut ctl) = query_char.single_mut() {
                            ui_setting_toggle(ui, l10n::tr("Unfly on Grounded"), &mut ctl.unfly_on_ground);
                        }

                        egui::CollapsingHeader::new(l10n::tr("Item Physics Snapshot"))
                            .default_open(false)
                            .show(ui, |ui| {
                                if let Some(def) = items.defs.get(0) {
                                    ui.label(format!("{}: {}", l10n::tr("Item"), def.name));
                                    ui.label(format!("{}: {:.3} kg", l10n::tr("Mass"), def.props.mass));
                                    ui.label(format!("{}: {:.5} m³", l10n::tr("Volume"), def.props.volume));
                                    ui.label(format!("{}: {:.1} kg/m³", l10n::tr("Density"), def.props.density));
                                    ui.label(format!("{}: {:.2} g/mol", l10n::tr("Molar Mass"), def.props.molar_mass));
                                } else {
                                    ui.small(l10n::tr("No item definitions loaded."));
                                }
                            });
                    }
                    SettingsPanel::Graphics => {
                        ui.label(l10n::tr("Render Effects"));

                        ui_setting_toggle(ui, l10n::tr("FXAA"), &mut cli.render_fxaa);
                        ui_setting_toggle(ui, l10n::tr("Tonemapping"), &mut cli.render_tonemapping);
                        ui_setting_toggle(ui, l10n::tr("Bloom"), &mut cli.render_bloom);
                        ui_setting_toggle(ui, l10n::tr("Screen Space Reflections"), &mut cli.render_ssr);
                        ui_setting_toggle(ui, l10n::tr("Volumetric Fog"), &mut cli.render_volumetric_fog);
                        ui_setting_line(ui, l10n::tr("Volumetric Fog Density"), egui::Slider::new(&mut cli.volumetric_fog_density, 0.0..=3.0));
                        ui_setting_line_custom(ui, l10n::tr("Volumetric Fog Palette"), |ui| {
                            ui_fog_color_palette(ui, &mut cli.volumetric_fog_color);
                        });
                        egui::CollapsingHeader::new(l10n::tr("Advanced Fog RGB"))
                            .default_open(false)
                            .show(ui, |ui| {
                                ui_setting_line(ui, l10n::tr("Volumetric Fog Color R"), egui::Slider::new(&mut cli.volumetric_fog_color.x, 0.0..=1.0));
                                ui_setting_line(ui, l10n::tr("Volumetric Fog Color G"), egui::Slider::new(&mut cli.volumetric_fog_color.y, 0.0..=1.0));
                                ui_setting_line(ui, l10n::tr("Volumetric Fog Color B"), egui::Slider::new(&mut cli.volumetric_fog_color.z, 0.0..=1.0));
                            });
                        ui_setting_toggle(ui, l10n::tr("Skybox + EnvMap"), &mut cli.render_skybox);

                        ui.label(l10n::tr("Lighting"));
                        ui_setting_toggle(ui, l10n::tr("Skylight Shadow"), &mut cli.skylight_shadow);
                        ui_setting_line(ui, l10n::tr("Skylight Illuminance"), egui::Slider::new(&mut cli.skylight_illuminance, 0.1..=200.0));

                        ui.label(l10n::tr("Quality Profile"));
                        ui_setting_toggle(ui, l10n::tr("High Quality Rendering"), &mut cfg.high_quality_rendering);
                    }
                    SettingsPanel::Audio => {

                        // ui_setting_line(ui, l10n::tr("Global Volume"), egui::Slider::new(&mut global_volume.volume as &mut f32, 0.0..=1.0));
                    }
                    SettingsPanel::Controls => {
                        ui.label(l10n::tr("Input Schemes"));
                        ui_setting_toggle(ui, l10n::tr("Touch UI (large buttons)"), &mut cfg.touch_ui);
                        ui_setting_toggle(ui, l10n::tr("Touch Tile Style Overlay"), &mut cfg.touch_tile_style_overlay_enabled);

                        ui.add_space(6.0);
                        ui_controls_device_map(ui, &cfg, &keys, &mouse_btns, &gamepads);
                        ui.add_space(8.0);
                        ui_gamepad_debug_panel(ui, &mut cfg.controls.gamepad, &gamepads, &mut rumble_requests);

                        ui.separator();
                        ui.label(l10n::tr("Keyboard + Mouse"));
                        ui_setting_line(ui, l10n::tr("Look Sensitivity"), egui::Slider::new(&mut cfg.controls.keyboard_mouse.look_sensitivity, 0.1..=4.0));
                        ui_setting_toggle(ui, l10n::tr("Invert Y"), &mut cfg.controls.keyboard_mouse.invert_y);
                        ui_setting_line(ui, l10n::tr("Jump Key"), egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_jump));
                        ui_setting_line(ui, l10n::tr("Sprint Key"), egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_sprint));
                        ui_setting_line(ui, l10n::tr("Sneak Key"), egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_sneak));
                        ui_setting_line(ui, l10n::tr("Pause Key"), egui::TextEdit::singleline(&mut cfg.controls.keyboard_mouse.key_pause));

                        ui.separator();
                        ui.label(l10n::tr("Gamepad"));
                        ui_setting_line(ui, l10n::tr("Look Sensitivity"), egui::Slider::new(&mut cfg.controls.gamepad.look_sensitivity, 0.1..=4.0));
                        ui_setting_toggle(ui, l10n::tr("Invert Y"), &mut cfg.controls.gamepad.invert_y);
                        ui_setting_line(ui, l10n::tr("Left Stick Dead Zone"), egui::Slider::new(&mut cfg.controls.gamepad.left_stick_dead_zone, 0.0..=0.5));
                        ui_setting_line(ui, l10n::tr("Right Stick Dead Zone"), egui::Slider::new(&mut cfg.controls.gamepad.right_stick_dead_zone, 0.0..=0.5));
                        ui_setting_line(ui, l10n::tr("Jump Button"), egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_jump));
                        ui_setting_line(ui, l10n::tr("Sprint Button"), egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_sprint));
                        ui_setting_line(ui, l10n::tr("Use Button"), egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_use));
                        ui_setting_line(ui, l10n::tr("Attack Button"), egui::TextEdit::singleline(&mut cfg.controls.gamepad.button_attack));

                        ui.separator();
                        ui.label(l10n::tr("Touch"));
                        ui_setting_toggle(ui, l10n::tr("Layout Edit Mode"), &mut cli.touch_controls_edit_mode);
                        ui_setting_line_custom(ui, l10n::tr("Undo Last Drag"), |ui| {
                            if ui.button(l10n::tr("Undo")).clicked() {
                                cfg.controls.touch_layout_request_undo = true;
                            }
                        });
                        if cli.touch_controls_edit_mode {
                            ui.colored_label(
                                Color32::from_rgb(255, 214, 140),
                                l10n::tr("Designer Active: drag joystick and buttons on the overlay. Gameplay touch input is locked."),
                            );
                        } else {
                            ui.colored_label(
                                Color32::from_gray(170),
                                l10n::tr("Enable Layout Edit Mode to open the visual touch UI designer."),
                            );
                        }
                        ui_setting_line(ui, l10n::tr("Move Stick Radius"), egui::Slider::new(&mut cfg.controls.touch.move_stick_radius, 48.0..=200.0));
                        ui_setting_line(ui, l10n::tr("Move Dead Zone"), egui::Slider::new(&mut cfg.controls.touch.move_dead_zone, 0.0..=0.5));
                        ui.colored_label(
                            Color32::from_gray(180),
                            l10n::tr("Tip: push the move stick to the top edge to lock sprint; pull down to release."),
                        );
                        ui_setting_line(ui, l10n::tr("Button Radius"), egui::Slider::new(&mut cfg.controls.touch.button_radius, 30.0..=80.0));
                        ui_setting_line(ui, l10n::tr("Vertical Slider Height"), egui::Slider::new(&mut cfg.controls.touch.vertical_slider_height, 120.0..=320.0));
                        ui_setting_line(ui, l10n::tr("Vertical Slider Width"), egui::Slider::new(&mut cfg.controls.touch.vertical_slider_width, 44.0..=96.0));
                        ui_setting_line(ui, l10n::tr("Fly Double Tap Window (sec)"), egui::Slider::new(&mut cfg.controls.touch.fly_double_tap_window_secs, 0.18..=0.65));

                        ui.separator();
                        ui.label(l10n::tr("Touch Button Action Mapping"));
                        ui_setting_line_custom(ui, l10n::tr("Attack Button Action"), |ui| {
                            egui::ComboBox::from_id_source("touch_attack_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.attack_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Attack, l10n::tr("Attack"));
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::UseItem, l10n::tr("UseItem"));
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Jump, l10n::tr("Jump"));
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sprint, l10n::tr("Sprint"));
                                    ui.selectable_value(&mut cfg.controls.touch.attack_button_action, TouchActionBinding::Sneak, l10n::tr("Sneak"));
                                });
                        });
                        ui_setting_line_custom(ui, l10n::tr("Use Button Action"), |ui| {
                            egui::ComboBox::from_id_source("touch_use_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.use_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Attack, l10n::tr("Attack"));
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::UseItem, l10n::tr("UseItem"));
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Jump, l10n::tr("Jump"));
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sprint, l10n::tr("Sprint"));
                                    ui.selectable_value(&mut cfg.controls.touch.use_button_action, TouchActionBinding::Sneak, l10n::tr("Sneak"));
                                });
                        });
                        ui_setting_line_custom(ui, l10n::tr("Jump Button Action"), |ui| {
                            egui::ComboBox::from_id_source("touch_jump_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.jump_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Attack, l10n::tr("Attack"));
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::UseItem, l10n::tr("UseItem"));
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Jump, l10n::tr("Jump"));
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sprint, l10n::tr("Sprint"));
                                    ui.selectable_value(&mut cfg.controls.touch.jump_button_action, TouchActionBinding::Sneak, l10n::tr("Sneak"));
                                });
                        });
                        ui_setting_line_custom(ui, l10n::tr("Sprint Button Action"), |ui| {
                            egui::ComboBox::from_id_source("touch_sprint_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.sprint_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Attack, l10n::tr("Attack"));
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::UseItem, l10n::tr("UseItem"));
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Jump, l10n::tr("Jump"));
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sprint, l10n::tr("Sprint"));
                                    ui.selectable_value(&mut cfg.controls.touch.sprint_button_action, TouchActionBinding::Sneak, l10n::tr("Sneak"));
                                });
                        });
                        ui_setting_line_custom(ui, l10n::tr("Sneak Button Action"), |ui| {
                            egui::ComboBox::from_id_source("touch_crouch_action")
                                .selected_text(format!("{:?}", cfg.controls.touch.crouch_button_action))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Attack, l10n::tr("Attack"));
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::UseItem, l10n::tr("UseItem"));
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Jump, l10n::tr("Jump"));
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sprint, l10n::tr("Sprint"));
                                    ui.selectable_value(&mut cfg.controls.touch.crouch_button_action, TouchActionBinding::Sneak, l10n::tr("Sneak"));
                                });
                        });

                        ui_setting_line_custom(ui, l10n::tr("Reset Touch Layout"), |ui| {
                            if ui.button(l10n::tr("Reset")).clicked() {
                                cfg.controls.touch = Default::default();
                                cli.touch_controls_edit_mode = false;
                            }
                        });

                        ui.separator();
                        ui.label(l10n::tr("Touch Layout Presets"));
                        ui_setting_line(
                            ui,
                            l10n::tr("Preset Name"),
                            egui::TextEdit::singleline(&mut cfg.controls.touch_layout_preset_name),
                        );
                        ui_setting_line_custom(ui, l10n::tr("Save Current Layout As Preset"), |ui| {
                            if ui.button(l10n::tr("Save")).clicked() {
                                let mut name = cfg.controls.touch_layout_preset_name.trim().to_string();
                                let current_layout = cfg.controls.touch.clone();
                                if name.is_empty() {
                                    name = format!("{} {}", l10n::tr("Preset"), cfg.controls.touch_layout_presets.len() + 1);
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
                                if ui.button(format!("{}: {}", l10n::tr("Load"), preset_name)).clicked() {
                                    cfg.controls.touch = preset_layout.clone();
                                    cli.touch_controls_edit_mode = true;
                                }
                                if ui.button(l10n::tr("Delete")).clicked() {
                                    remove_idx = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove_idx {
                            cfg.controls.touch_layout_presets.remove(i);
                        }

                        ui.separator();
                        ui.label(l10n::tr("Share Touch Layout"));
                        ui.add_sized(
                            [ui.available_width(), 66.0],
                            egui::TextEdit::multiline(&mut cfg.controls.touch_layout_share_text)
                                .hint_text(l10n::tr("Layout JSON for sharing/import")),
                        );
                        ui.horizontal(|ui| {
                            if ui.button(l10n::tr("Export + Copy")).clicked() {
                                if let Ok(text) = serde_json::to_string(&cfg.controls.touch) {
                                    cfg.controls.touch_layout_share_text = text;
                                    ui.ctx().copy_text(cfg.controls.touch_layout_share_text.clone());
                                }
                            }
                            if ui.button(l10n::tr("Import From Text")).clicked() {
                                if let Ok(layout) = serde_json::from_str::<crate::client::settings::TouchControlsConfig>(&cfg.controls.touch_layout_share_text) {
                                    cfg.controls.touch = layout;
                                    cli.touch_controls_edit_mode = true;
                                }
                            }
                        });
                    }
                    SettingsPanel::Language => {
                        ui.heading(l10n::text(&cfg.language, "settings.language.title"));
                        ui.small(l10n::text(&cfg.language, "settings.language.description"));
                        ui.separator();

                        let mut selected = cfg.language.clone();
                        let selected_label = l10n::supported_languages()
                            .iter()
                            .find(|opt| opt.code == selected)
                            .map(|opt| opt.native_name.to_string())
                            .unwrap_or_else(|| selected.clone());
                        ui_setting_line_custom(ui, l10n::text(&cfg.language, "settings.language.select"), |ui| {
                            egui::ComboBox::from_id_source("settings_language_combo")
                                .selected_text(selected_label)
                                .show_ui(ui, |ui| {
                                    for opt in l10n::supported_languages() {
                                        ui.selectable_value(&mut selected, opt.code.to_string(), opt.native_name);
                                    }
                                });
                        });

                        if selected != cfg.language {
                            cfg.language = l10n::normalize_language(&selected).to_string();
                        }

                        let current = l10n::supported_languages()
                            .iter()
                            .find(|opt| opt.code == cfg.language)
                            .map(|opt| opt.native_name.to_string())
                            .unwrap_or_else(|| cfg.language.clone());
                        ui.label(format!("{}: {}", l10n::text(&cfg.language, "settings.language.current"), current));

                        ui.separator();
                        ui.label(l10n::text(&cfg.language, "settings.language.preview_title"));
                        ui.label(l10n::text(&cfg.language, "settings.language.preview_line1"));
                        ui.label(l10n::text(&cfg.language, "settings.language.preview_line2"));
                        ui.small(l10n::text(&cfg.language, "settings.language.fallback"));
                    }
                    SettingsPanel::Mods => {}
                    _ => (),
                }
            },
        );
    });

    // detect changes to touch tile style and clear caches so tiles update immediately
    let new_style = cfg.touch_tile_style.clone();
    if prev_touch_style.as_ref() != Some(&new_style) {
        crate::client::ui::main_menu::clear_touch_menu_caches(&mut images);
        cli.curr_ui = CurrentUI::MainMenu;
    }
    *prev_touch_style = Some(new_style);
}
