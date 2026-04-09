use bevy::prelude::{Res, ResMut, Resource};
use bevy_egui::egui::Painter;

use crate::{
    client::prelude::CurrentUI,
    item::{Inventory, ItemCategory, ItemStack, Items},
    net::{CPacket, RenetClientHelper},
    voxel::{VoxShape, VoxTex},
    ui::prelude::*,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryOperation {
    Place,
    Mine,
    Weapon,
    Food,
    Inspect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceVoxelDef {
    pub name: &'static str,
    pub tex: u16,
    pub shape: VoxShape,
}

pub fn placeable_voxel_defs() -> &'static [PlaceVoxelDef] {
    &[
        PlaceVoxelDef { name: "stone", tex: VoxTex::Stone, shape: VoxShape::Cube },
        PlaceVoxelDef { name: "dirt", tex: VoxTex::Dirt, shape: VoxShape::Cube },
        PlaceVoxelDef { name: "grass", tex: VoxTex::Grass, shape: VoxShape::Grass },
        PlaceVoxelDef { name: "sand", tex: VoxTex::Sand, shape: VoxShape::Cube },
        PlaceVoxelDef { name: "log", tex: VoxTex::Log, shape: VoxShape::Cube },
        PlaceVoxelDef { name: "leaves", tex: VoxTex::Leaves, shape: VoxShape::Leaves },
        PlaceVoxelDef { name: "water", tex: VoxTex::Water, shape: VoxShape::Cube },
        PlaceVoxelDef { name: "shortgrass", tex: VoxTex::ShortGrass, shape: VoxShape::Grass },
        PlaceVoxelDef { name: "bush", tex: VoxTex::Bush, shape: VoxShape::Grass },
        PlaceVoxelDef { name: "rose", tex: VoxTex::Rose, shape: VoxShape::Grass },
        PlaceVoxelDef { name: "fern", tex: VoxTex::Fern, shape: VoxShape::Grass },
    ]
}

impl InventoryOperation {
    pub const ALL: [InventoryOperation; 5] = [
        InventoryOperation::Place,
        InventoryOperation::Mine,
        InventoryOperation::Weapon,
        InventoryOperation::Food,
        InventoryOperation::Inspect,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InventoryOperation::Place => "Place",
            InventoryOperation::Mine => "Mine",
            InventoryOperation::Weapon => "Weapon",
            InventoryOperation::Food => "Food",
            InventoryOperation::Inspect => "Inspect",
        }
    }
}

#[derive(Resource)]
pub struct InventoryUiState {
    pub holding_slot: Option<usize>,
    pub pending_swaps: Vec<(u16, u16)>,
    pub operation_filters: Vec<InventoryOperation>,
    pub active_operation: InventoryOperation,
    pub op_selected_items: Vec<(InventoryOperation, Vec<u8>)>,
    pub bind_left_click: Vec<InventoryOperation>,
    pub bind_right_click: Vec<InventoryOperation>,
    pub bind_short_press: Vec<InventoryOperation>,
    pub bind_long_press: Vec<InventoryOperation>,
    pub catalog_search: String,
}

impl Default for InventoryUiState {
    fn default() -> Self {
        Self {
            holding_slot: None,
            pending_swaps: Vec::new(),
            operation_filters: vec![InventoryOperation::Place],
            active_operation: InventoryOperation::Place,
            op_selected_items: Vec::new(),
            bind_left_click: vec![InventoryOperation::Mine],
            bind_right_click: vec![InventoryOperation::Place],
            bind_short_press: vec![InventoryOperation::Weapon],
            bind_long_press: vec![InventoryOperation::Inspect],
            catalog_search: String::new(),
        }
    }
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

    let item_idx = (slot.item_id - 1) as usize;
    if let Some(Some(texture_id)) = items.icon_egui.get(item_idx) {
        painter.image(
            *texture_id,
            rect.shrink(3.),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let atlas_slot = items.atlas_slot_order.iter().position(|id| *id as usize == slot.item_id as usize);
        if let Some(slot_idx) = atlas_slot {
            let atlas_len = items.atlas_slot_count.max(1) as f32;
            let uv_siz = 1. / atlas_len;
            let uv_x = uv_siz * slot_idx as f32;
            painter.image(
                items.atlas_egui,
                rect.shrink(3.),
                Rect::from_min_size(pos2(uv_x, 0.), vec2(uv_siz, 1.)),
                Color32::WHITE,
            );
        } else if let Some(name) = items.reg.at((slot.item_id - 1) as u16) {
            if let Some(place_def) = placeable_voxel_defs().iter().find(|def| def.name == name) {
                draw_place_voxel(place_def, rect, painter, items);
            }
        }
    }
    // Item Count
    painter.text(
        rect.max - vec2(4., 2.),
        Align2::RIGHT_BOTTOM,
        slot.count.to_string(),
        egui::FontId::proportional(12.),
        Color32::from_gray(190),
    );
}

pub fn draw_place_voxel(def: &PlaceVoxelDef, rect: Rect, painter: &Painter, items: &Items) {
    if items.terrain_atlas_egui == bevy_egui::egui::TextureId::default() {
        return;
    }

    let uv_min = VoxTex::map_uv(bevy::prelude::Vec2::new(0.0, 0.0), def.tex);
    let uv_max = VoxTex::map_uv(bevy::prelude::Vec2::new(1.0, 1.0), def.tex);
    painter.image(
        items.terrain_atlas_egui,
        rect.shrink(3.),
        Rect::from_min_max(pos2(uv_min.x, uv_min.y), pos2(uv_max.x, uv_max.y)),
        Color32::WHITE,
    );
}

pub fn item_sort_key(items: &Items, item_id: u8) -> (u8, u8) {
    if item_id == 0 {
        return (u8::MAX, u8::MAX);
    }

    let idx = (item_id - 1) as usize;
    let atlas_rank = items.atlas_slot_order.iter().position(|id| *id as usize == item_id as usize);
    let def = items.defs.get(idx);
    let is_main = def.is_some_and(|def| matches!(def.category, ItemCategory::Main));

    let rank = if atlas_rank.is_some() {
        0
    } else if is_main {
        1
    } else {
        2
    };

    (rank, item_id)
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

fn op_selected_items_mut(ui_state: &mut InventoryUiState, op: InventoryOperation) -> &mut Vec<u8> {
    if let Some(idx) = ui_state.op_selected_items.iter().position(|(k, _)| *k == op) {
        return &mut ui_state.op_selected_items[idx].1;
    }
    ui_state.op_selected_items.push((op, Vec::new()));
    let idx = ui_state.op_selected_items.len() - 1;
    &mut ui_state.op_selected_items[idx].1
}

fn op_selected_items_ref(ui_state: &InventoryUiState, op: InventoryOperation) -> &[u8] {
    ui_state
        .op_selected_items
        .iter()
        .find(|(k, _)| *k == op)
        .map(|(_, items)| items.as_slice())
        .unwrap_or(&[])
}

fn vec_toggle<T: PartialEq + Copy>(v: &mut Vec<T>, val: T, enabled: bool) {
    if enabled {
        if !v.contains(&val) {
            v.push(val);
        }
    } else {
        v.retain(|x| *x != val);
    }
}

fn op_matches_item(op: InventoryOperation, item_name: &str) -> bool {
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

fn matches_filters(filters: &[InventoryOperation], item_name: &str) -> bool {
    if filters.is_empty() {
        return true;
    }
    filters.iter().any(|op| op_matches_item(*op, item_name))
}

fn find_inventory_slot_by_item_id(inv: &Inventory, item_id: u8) -> Option<usize> {
    inv.items
        .iter()
        .enumerate()
        .find(|(_, s)| !s.is_empty() && s.item_id == item_id)
        .map(|(idx, _)| idx)
}

pub fn ui_inventory_operation_first(
    ui: &mut egui::Ui,
    inv: &mut Inventory,
    items: &Items,
    ui_state: &mut InventoryUiState,
    mut vox_brush: Option<ResMut<crate::voxel::VoxelBrush>>,
) {
    ui.columns(2, |cols| {
        cols[0].set_min_width(210.0);
        cols[0].vertical(|ui| {
            ui.heading("Operations");
            ui.small("Select operation(s) first, then pick matching items.");
            ui.add_space(4.0);

            for op in InventoryOperation::ALL {
                let mut enabled = ui_state.operation_filters.contains(&op);
                if ui.checkbox(&mut enabled, op.label()).changed() {
                    vec_toggle(&mut ui_state.operation_filters, op, enabled);
                }
                if ui
                    .selectable_label(ui_state.active_operation == op, format!("Active: {}", op.label()))
                    .clicked()
                {
                    ui_state.active_operation = op;
                }
            }

            if ui_state.operation_filters.is_empty() {
                ui_state.operation_filters.push(ui_state.active_operation);
            }

            ui.separator();
            ui.label(format!("Binding: {}", ui_state.active_operation.label()));

            let active = ui_state.active_operation;
            let mut b_left = ui_state.bind_left_click.contains(&active);
            let mut b_right = ui_state.bind_right_click.contains(&active);
            let mut b_short = ui_state.bind_short_press.contains(&active);
            let mut b_long = ui_state.bind_long_press.contains(&active);

            if ui.checkbox(&mut b_left, "Left Click").changed() {
                vec_toggle(&mut ui_state.bind_left_click, active, b_left);
            }
            if ui.checkbox(&mut b_right, "Right Click").changed() {
                vec_toggle(&mut ui_state.bind_right_click, active, b_right);
            }
            if ui.checkbox(&mut b_short, "Short Press").changed() {
                vec_toggle(&mut ui_state.bind_short_press, active, b_short);
            }
            if ui.checkbox(&mut b_long, "Long Press").changed() {
                vec_toggle(&mut ui_state.bind_long_press, active, b_long);
            }

            ui.separator();
            let selected_cnt = op_selected_items_ref(ui_state, active).len();
            ui.label(format!("Selected items: {}", selected_cnt));
            if ui.button("Clear Active Selection").clicked() {
                op_selected_items_mut(ui_state, active).clear();
            }
        });

        cols[1].vertical(|ui| {
            let is_place = ui_state.active_operation == InventoryOperation::Place;
            ui.heading(if is_place { "Place Catalog" } else { "Item Catalog" });
            ui.small(if is_place {
                "Place uses voxel atlas contents, not the item atlas."
            } else {
                "Right side shows atlas-backed items first."
            });
            ui.text_edit_singleline(&mut ui_state.catalog_search);
            ui.add_space(4.0);

            let search = ui_state.catalog_search.trim().to_ascii_lowercase();
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), |ui| {
                    ui.style_mut().spacing.item_spacing = vec2(4.0, 4.0);

                    let max_id = items.reg.len().min(u8::MAX as usize);
                    let mut item_ids = if is_place {
                        placeable_voxel_defs()
                            .iter()
                            .copied()
                            .filter(|def| search.is_empty() || def.name.contains(&search))
                            .map(|def| def.tex as u8)
                            .collect::<Vec<_>>()
                    } else {
                        let mut ids = Vec::new();
                        for item_id in 1..=max_id {
                            let item_u8 = item_id as u8;
                            let Some(name) = items.reg.at((item_u8 - 1) as u16) else {
                                continue;
                            };

                            if !matches_filters(&ui_state.operation_filters, name) {
                                continue;
                            }
                            if !search.is_empty() && !name.to_ascii_lowercase().contains(&search) {
                                continue;
                            }

                            ids.push(item_u8);
                        }
                        ids
                    };

                    if !is_place {
                        item_ids.sort_by_key(|item_id| item_sort_key(items, *item_id));
                    }

                    for item_u8 in item_ids {
                        let place_def = if is_place {
                            placeable_voxel_defs().iter().copied().find(|def| def.tex as u8 == item_u8)
                        } else {
                            None
                        };
                        let label = place_def
                            .map(|def| def.name)
                            .or_else(|| items.reg.at((item_u8 - 1) as u16).map(|name| name.as_str()))
                            .unwrap_or("unknown");

                        let selected = if is_place {
                            vox_brush.as_ref().is_some_and(|brush| brush.tex == item_u8 as u16)
                                || op_selected_items_ref(ui_state, ui_state.active_operation).contains(&item_u8)
                        } else {
                            op_selected_items_ref(ui_state, ui_state.active_operation).contains(&item_u8)
                        };
                        let slot_btn = egui::Button::new("").fill(if selected {
                            Color32::from_rgba_premultiplied(120, 210, 255, 120)
                        } else {
                            Color32::from_black_alpha(90)
                        });

                        let mut resp = ui.add_sized([50.0, 50.0], slot_btn);
                        resp = resp.on_hover_text(format!("{} [{}]", label, item_u8));

                        if let Some(def) = place_def {
                            draw_place_voxel(&def, resp.rect, ui.painter(), items);
                        } else {
                            let fake = ItemStack::new(1, item_u8);
                            draw_item(&fake, resp.rect, ui.painter(), items);
                        }

                        if resp.clicked() {
                            let selected_items = op_selected_items_mut(ui_state, ui_state.active_operation);
                            if selected_items.contains(&item_u8) {
                                selected_items.retain(|v| *v != item_u8);
                            } else {
                                selected_items.push(item_u8);
                            }

                            if is_place {
                                if let Some(vox_brush) = vox_brush.as_mut() {
                                    if let Some(def) = place_def {
                                        vox_brush.tex = def.tex;
                                        vox_brush.shape = def.shape;
                                    }
                                }
                            } else if let Some(inv_slot) = find_inventory_slot_by_item_id(inv, item_u8) {
                                ui_state.holding_slot = Some(inv_slot);
                            }
                        }
                    }
                });
            });

            ui.separator();
            ui.label("Owned Inventory (authoritative swap)");
            ui_inventory(ui, inv, items, ui_state);
        });
    });
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
