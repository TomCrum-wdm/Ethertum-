use bevy::prelude::{Res, ResMut, Resource};
use bevy_egui::egui::Painter;

use crate::{
    client::prelude::CurrentUI,
    item::{Inventory, ItemStack, Items},
    net::{CPacket, RenetClientHelper},
    ui::prelude::*,
};

#[derive(Resource, Default)]
pub struct InventoryUiState {
    pub holding_slot: Option<usize>,
    pub pending_swaps: Vec<(u16, u16)>,
}

pub fn draw_ui_holding_item(
    mut ctx: EguiContexts,
    items: Option<Res<Items>>,
    player: Option<Res<crate::client::prelude::ClientPlayerInfo>>,
    ui_state: Option<Res<InventoryUiState>>,
) {
    let Some(items) = items else {
        return;
    };
    let Some(player) = player else {
        return;
    };
    let Some(ui_state) = ui_state else {
        return;
    };

    let Some(slot_idx) = ui_state.holding_slot else {
        return;
    };

    let Some(hold) = player.inventory.items.get(slot_idx) else {
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

        draw_item(hold, Rect::from_min_size(curpos - size / 2., size), &ctx_mut.debug_painter(), &items);
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

pub fn ui_item_stack(ui: &mut egui::Ui, slot: &mut ItemStack, slot_idx: usize, items: &Items, ui_state: &mut InventoryUiState) {
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
        if let Some(hold_idx) = ui_state.holding_slot {
            if hold_idx != slot_idx {
                ui_state.pending_swaps.push((hold_idx as u16, slot_idx as u16));
            }
            ui_state.holding_slot = None;
        } else {
            ui_state.holding_slot = Some(slot_idx);
        }
    } else if resp.secondary_clicked() {
        // Right click currently reserved; do not mutate local inventory in authoritative mode.
    }
}

pub fn ui_inventory(ui: &mut egui::Ui, inv: &mut Inventory, items: &Items, ui_state: &mut InventoryUiState) -> InnerResponse<()> {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), |ui| {
        ui.style_mut().spacing.item_spacing = vec2(4., 4.);

        for (idx, item) in inv.items.iter_mut().enumerate() {
            ui_item_stack(ui, item, idx, items, ui_state);
        }
    })
}

pub fn flush_inventory_ui_ops(
    mut ui_state: ResMut<InventoryUiState>,
    mut net_client: Option<ResMut<bevy_renet::renet::RenetClient>>,
    cli: Res<crate::client::prelude::ClientInfo>,
) {
    if cli.curr_ui == CurrentUI::MainMenu {
        ui_state.holding_slot = None;
        ui_state.pending_swaps.clear();
        return;
    }

    let Some(net_client) = net_client.as_mut() else {
        return;
    };

    for (a, b) in ui_state.pending_swaps.drain(..) {
        net_client.send_packet(&CPacket::InventorySwap { a, b });
    }
}
