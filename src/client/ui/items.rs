use bevy::prelude::Res;
use bevy_egui::egui::Painter;
use std::sync::Mutex;

use crate::{
    item::{Inventory, ItemStack, Items},
    ui::prelude::*,
};

static UI_HOLDING_ITEM: Mutex<ItemStack> = Mutex::new(ItemStack { count: 0, item_id: 0 });

pub fn draw_ui_holding_item(mut ctx: EguiContexts, items: Option<Res<Items>>) {
    let Some(items) = items else {
        return;
    };

    let Ok(hold) = UI_HOLDING_ITEM.lock() else {
        return;
    };

    if !hold.is_empty() {
        let Ok(ctx_mut) = ctx.ctx_mut() else {
            return;
        };
        let Some(curpos) = ctx_mut.pointer_latest_pos() else {
            return;
        };
        let size = vec2(50., 50.);

        draw_item(&hold, Rect::from_min_size(curpos - size / 2., size), &ctx_mut.debug_painter(), &items);
    }
}

pub fn draw_item(slot: &ItemStack, rect: Rect, painter: &Painter, items: &Items) {
    let num_all_items = items.reg.len();
    if num_all_items == 0 || slot.item_id == 0 {
        return;
    }

    // Item Texture
    let uv_siz = 1. / num_all_items as f32;
    let uv_x = uv_siz * (slot.item_id - 1) as f32;
    painter.image(
        items.atlas_egui,
        rect.shrink(3.),
        Rect::from_min_size(pos2(uv_x, 0.), vec2(uv_siz, 1.)),
        Color32::WHITE,
    );
    // Item Count
    painter.text(
        rect.max - vec2(4., 2.),
        Align2::RIGHT_BOTTOM,
        slot.count.to_string(),
        egui::FontId::proportional(12.),
        Color32::from_gray(190),
    );
}

pub fn ui_item_stack(ui: &mut egui::Ui, slot: &mut ItemStack, items: &Items) {
    let num_all_items = items.reg.len();

    let slot_btn = egui::Button::new("").fill(Color32::from_black_alpha(100));
    // if cli.hotbar_index == i {
    //     slot = slot.stroke(Stroke::new(3., Color32::WHITE));
    // }

    let slot_btn_size = 50.;
    let mut resp = ui.add_sized([slot_btn_size, slot_btn_size], slot_btn);

    if !slot.is_empty() {
        // Tooltip
        resp = resp.on_hover_ui(|ui| {
            if let Some(name) = items.reg.at((slot.item_id - 1) as u16) {
                ui.label(name);
                ui.small(format!("{} [{}/{}] x{}", name, slot.item_id, num_all_items, slot.count));
            }
        });

        draw_item(slot, resp.rect, ui.painter(), items)
    }

    if resp.clicked() {
        if let Ok(mut hold) = UI_HOLDING_ITEM.lock() {
            ItemStack::swap(&mut hold, slot);
        }
    } else if resp.secondary_clicked() {
        slot.count += 1;
        slot.item_id += 1;
    }
}

pub fn ui_inventory(ui: &mut egui::Ui, inv: &mut Inventory, items: &Items) -> InnerResponse<()> {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), |ui| {
        ui.style_mut().spacing.item_spacing = vec2(4., 4.);

        for item in inv.items.iter_mut() {
            ui_item_stack(ui, item, items);
        }
    })
}
