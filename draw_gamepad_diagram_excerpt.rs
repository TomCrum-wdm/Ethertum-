fn draw_gamepad_diagram(ui: &mut Ui, jump: &str, sprint: &str, use_btn: &str, attack: &str) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(580.0, 280.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bg = Color32::from_rgb(30, 35, 46);
    let dark_bg = Color32::from_rgb(20, 24, 32);
    let shell = Color32::from_rgb(82, 92, 114);
    let shell_grip = Color32::from_rgb(70, 80, 100);
    let mapped_color = Color32::from_rgb(95, 170, 255);
    let text_color = Color32::WHITE;

    painter.rect_filled(rect, 8.0, bg);
    
    let center = rect.center() + egui::vec2(0.0, -10.0);
    let scale = 1.3;
    
    // Matcher
    let is_match = |button: &str| -> bool {
        let b = button.to_ascii_lowercase();
        let mut matched = false;
        for m in [jump, sprint, use_btn, attack] {
            let m = m.to_ascii_lowercase();
            if m.contains(&b) 
            || (b == "a" && m.contains("south"))
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

    let mut draw_btn = |p: egui::Pos2, r: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        
        let p_shadow = p + egui::vec2(0.0, 2.0);
        painter.circle_filled(p_shadow, r, Color32::from_rgb(15, 18, 24));
        painter.circle_filled(p, r, if mapped { mapped_color } else { dark_bg });
        
        painter.text(p, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 1.2), if mapped { Color32::BLACK } else { text_color });
    };
    
    let mut draw_joystick = |p: egui::Pos2, r: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let dark = Color32::from_rgb(15, 18, 24);
        let mid = if mapped { mapped_color } else { Color32::from_rgb(35, 40, 50) };
        let indent = if mapped { Color32::from_rgb(60, 130, 200) } else { dark_bg };
        
        let p_shadow = p + egui::vec2(0.0, 4.0);
        painter.circle_filled(p_shadow, r, Color32::from_black_alpha(150));
        
        // Detailed 3D thumbstick
        painter.circle_filled(p, r, dark); // base ring
        painter.circle_filled(p + egui::vec2(0.0, -1.0), r - 2.0, mid); // outer rim
        painter.circle_filled(p + egui::vec2(0.0, 1.0), r - 5.0, dark); // inner slope
        painter.circle_filled(p, r - 7.0, indent); // thumb indent center
        
        painter.text(p, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(r * 0.8), if mapped { Color32::BLACK } else { text_color });
    };

    let mut draw_rect_btn = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let r_shadow = r.translate(egui::vec2(0.0, 2.0));
        
        painter.rect_filled(r_shadow, radius, Color32::from_rgb(15, 18, 24));
        painter.rect_filled(r, radius, if mapped { mapped_color } else { dark_bg });
        
        if !text.is_empty() {
            painter.text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(14.0), if mapped { Color32::BLACK } else { text_color });
        }
    };

    let mut draw_rect_btn_3d = |r: egui::Rect, radius: f32, text: &str, name: &str| {
        let mapped = is_match(name);
        let shadow_col = Color32::from_rgb(15, 18, 24);
        let base_col = if mapped { mapped_color } else { dark_bg };
        let top_col = if mapped { Color32::from_rgb(130, 200, 255) } else { Color32::from_rgb(50, 55, 65) };

        // Outer shadow
        painter.rect_filled(r.translate(egui::vec2(0.0, 4.0)), radius, shadow_col);
        // Base / front face
        painter.rect_filled(r, radius, base_col);
        // Top bevel face to create 3D button press look
        let mut top_r = r;
        top_r.max.y = top_r.min.y + (r.height() * 0.4);
        painter.rect_filled(top_r, radius * 0.8, top_col);
        
        if !text.is_empty() {
            painter.text(r.center() + egui::vec2(0.0, 2.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), if mapped { Color32::BLACK } else { text_color });
        }
    };

    let shell = Color32::from_rgb(45, 48, 55);
    let shell_shadow = Color32::from_rgb(25, 28, 34);

    // Triggers (LT/RT) - Tall rectangles behind
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-80.0 * scale, -55.0 * scale), egui::vec2(55.0 * scale, 30.0 * scale)), 6.0, "LT", "lt");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(80.0 * scale, -55.0 * scale), egui::vec2(55.0 * scale, 30.0 * scale)), 6.0, "RT", "rt");
    
    // Bumpers (LB/RB) - Wider rectangles lower down
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(-85.0 * scale, -38.0 * scale), egui::vec2(65.0 * scale, 22.0 * scale)), 8.0, "LB", "lb");
    draw_rect_btn_3d(egui::Rect::from_center_size(center + egui::vec2(85.0 * scale, -38.0 * scale), egui::vec2(65.0 * scale, 22.0 * scale)), 8.0, "RB", "rb");

    // Main Body (Xbox Style)
    // Flatter top edge, sloped bottom grips
    let p_tl = center + egui::vec2(-105.0 * scale, -25.0 * scale);
    let p_tr = center + egui::vec2(105.0 * scale, -25.0 * scale);
    let p_gl = center + egui::vec2(-125.0 * scale, 90.0 * scale);
    let p_gr = center + egui::vec2(125.0 * scale, 90.0 * scale);
    let p_gbl = center + egui::vec2(-80.0 * scale, 120.0 * scale);
    let p_gbr = center + egui::vec2(80.0 * scale, 120.0 * scale);
    let p_bl = center + egui::vec2(-40.0 * scale, 60.0 * scale);
    let p_br = center + egui::vec2(40.0 * scale, 60.0 * scale);

    let shift_shadow = egui::vec2(0.0, 6.0);
    let body_pts = vec![p_tl, p_tr, p_gr, p_gbr, p_br, p_bl, p_gbl, p_gl];
    let shadow_pts = body_pts.iter().map(|p| *p + shift_shadow).collect::<Vec<_>>();
    
    painter.add(egui::Shape::convex_polygon(shadow_pts, shell_shadow, egui::Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(body_pts, shell, egui::Stroke::NONE));

    // Fill in top gap to make it straight but slightly indented
    painter.rect_filled(egui::Rect::from_min_max(center + egui::vec2(-60.0 * scale, -15.0 * scale), center + egui::vec2(60.0 * scale, 0.0)), 0.0, shell);
    
    // Fill in shoulders to round top corners
    painter.circle_filled(center + egui::vec2(-85.0 * scale, -5.0 * scale), 20.0 * scale, shell); 
    painter.circle_filled(center + egui::vec2(85.0 * scale, -5.0 * scale), 20.0 * scale, shell); 
    // Fill bottom grip curves
    painter.circle_filled(center + egui::vec2(-102.0 * scale, 105.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);
    painter.circle_filled(center + egui::vec2(102.0 * scale, 105.0 * scale) + shift_shadow, 25.0 * scale, shell_shadow);
    painter.circle_filled(center + egui::vec2(-102.0 * scale, 105.0 * scale), 25.0 * scale, shell);
    painter.circle_filled(center + egui::vec2(102.0 * scale, 105.0 * scale), 25.0 * scale, shell);

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
    draw_btn(center + egui::vec2(90.0 * scale, -30.0 * scale), 10.0 * scale, "Y", "y");
    draw_btn(center + egui::vec2(115.0 * scale, -10.0 * scale), 10.0 * scale, "B", "b");
    draw_btn(center + egui::vec2(65.0 * scale, -10.0 * scale), 10.0 * scale, "X", "x");
    draw_btn(center + egui::vec2(90.0 * scale, 10.0 * scale), 10.0 * scale, "A", "a");

    // Start / Select
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(-25.0 * scale, -10.0 * scale), egui::vec2(15.0 * scale, 10.0 * scale)), 4.0, "", "sel");
    draw_rect_btn(egui::Rect::from_center_size(center + egui::vec2(25.0 * scale, -10.0 * scale), egui::vec2(15.0 * scale, 10.0 * scale)), 4.0, "", "start");
    
    // Home
    painter.circle_filled(center + egui::vec2(0.0, -30.0 * scale), 15.0 * scale, Color32::from_gray(120));
}

