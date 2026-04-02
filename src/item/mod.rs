use crate::util::registry::{RegId, Registry};

use crate::prelude::*;

// pub struct Item {
//     // tab_category
//     max_stacksize: u32,

//     max_damage: u32,
//     // name
// }


#[derive(Default, Clone, Copy)]
pub struct ItemStack {
    pub count: u8,
    pub item_id: u8,
    // pub durability
}
impl ItemStack {
    pub fn new(count: u8, item: u8) -> Self {
        Self { count, item_id: item }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.item_id == 0
    }

    pub fn clear(&mut self) {
        *self = ItemStack::default();
    }

    pub fn swap(a: &mut Self, b: &mut Self) {
        std::mem::swap(a, b);
    }
}

/// 物理属性结构体
#[derive(Debug, Clone, Copy, Default)]
pub struct MaterialProps {
    pub mass: f32,        // 单位物品质量(kg)
    pub volume: f32,      // 单位物品体积(m^3)
    pub density: f32,     // 密度(kg/m^3)
    pub molar_mass: f32,  // 摩尔质量(g/mol)
}

/// 物品定义结构体（可扩展）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemCategory {
    Main,
    Secondary,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub name: String,
    pub props: MaterialProps,
    pub category: ItemCategory,
}

/// 物品注册表，附带物理属性
// Items 结构体只定义一次，见下方

#[derive(Default)]
pub struct Inventory {
    pub items: Vec<ItemStack>,
}

impl Inventory {
    pub fn new(len: usize) -> Self {
        let mut items = Vec::new();
        items.resize(len, ItemStack::default());

        Self { items }
    }
}

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Items::default());
        // app.insert_resource(Registry::default());

        // On Android, defer heavy startup registration work by a couple of frames
        // to ensure the first frame can be presented and system splash can exit.
        app.add_systems(Update, setup_items_deferred);
        app.add_systems(bevy_egui::EguiPrimaryContextPass, setup_items_egui);

        // app.add_systems(PostStartup, bake_items);
    }
}

fn setup_items_deferred(
    mut initialized: Local<bool>,
    mut defer_frames: Local<u8>,
    items: ResMut<Items>,
    asset_server: Res<AssetServer>,
) {
    if *initialized {
        return;
    }

    if cfg!(target_os = "android") && *defer_frames < 2 {
        *defer_frames += 1;
        return;
    }

    setup_items(items, asset_server);
    *initialized = true;
}

#[derive(Resource, Default)]
pub struct Items {
    pub reg: Registry,
    pub defs: Vec<ItemDef>,
    pub atlas: Handle<Image>,
    pub atlas_egui: bevy_egui::egui::TextureId,

    pub apple: RegId,

    pub coal: RegId,
    pub stick: RegId,

    pub frame: RegId,
    pub lantern: RegId,

    pub pickaxe: RegId,
    pub shears: RegId,
    pub grapple: RegId,
    pub iron_ingot: RegId,
}

fn setup_items(
    mut items: ResMut<Items>,
    asset_server: Res<AssetServer>,
) {
    let reg = &mut items.reg;
    let mut defs = Vec::new();

    // 方块类物品注册及物理属性
    let _stone = reg.insert("stone");
    defs.push(ItemDef {
        name: "stone".to_string(),
        props: MaterialProps { mass: 2.6, volume: 0.001, density: 2600.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _dirt = reg.insert("dirt");
    defs.push(ItemDef {
        name: "dirt".to_string(),
        props: MaterialProps { mass: 1.2, volume: 0.001, density: 1200.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _grass = reg.insert("grass");
    defs.push(ItemDef {
        name: "grass".to_string(),
        props: MaterialProps { mass: 1.1, volume: 0.001, density: 1100.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _sand = reg.insert("sand");
    defs.push(ItemDef {
        name: "sand".to_string(),
        props: MaterialProps { mass: 1.6, volume: 0.001, density: 1600.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _log = reg.insert("log");
    defs.push(ItemDef {
        name: "log".to_string(),
        props: MaterialProps { mass: 0.7, volume: 0.001, density: 700.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _leaves = reg.insert("leaves");
    defs.push(ItemDef {
        name: "leaves".to_string(),
        props: MaterialProps { mass: 0.3, volume: 0.001, density: 300.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let _water = reg.insert("water");
    defs.push(ItemDef {
        name: "water".to_string(),
        props: MaterialProps { mass: 1.0, volume: 0.001, density: 1000.0, molar_mass: 18.0 },
        category: ItemCategory::Main,
    });

    // 注册物品及其物理属性
    let apple = reg.insert("apple");
    defs.push(ItemDef {
        name: "apple".to_string(),
        props: MaterialProps { mass: 0.15, volume: 0.0002, density: 750.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    reg.insert("avocado"); // tmp

    let coal = reg.insert("coal");
    defs.push(ItemDef {
        name: "coal".to_string(),
        props: MaterialProps { mass: 0.05, volume: 0.00005, density: 1400.0, molar_mass: 12.0 },
        category: ItemCategory::Main,
    });
    let stick = reg.insert("stick");
    defs.push(ItemDef {
        name: "stick".to_string(),
        props: MaterialProps { mass: 0.02, volume: 0.00004, density: 500.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });

    let frame = reg.insert("frame");
    defs.push(ItemDef {
        name: "frame".to_string(),
        props: MaterialProps { mass: 0.5, volume: 0.001, density: 800.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });
    let lantern = reg.insert("lantern");
    defs.push(ItemDef {
        name: "lantern".to_string(),
        props: MaterialProps { mass: 0.3, volume: 0.0008, density: 1200.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });

    let pickaxe = reg.insert("pickaxe");
    defs.push(ItemDef {
        name: "pickaxe".to_string(),
        props: MaterialProps { mass: 1.2, volume: 0.002, density: 7800.0, molar_mass: 0.0 },
        category: ItemCategory::Secondary,
    });
    let shears = reg.insert("shears");
    defs.push(ItemDef {
        name: "shears".to_string(),
        props: MaterialProps { mass: 0.6, volume: 0.001, density: 7800.0, molar_mass: 0.0 },
        category: ItemCategory::Secondary,
    });
    let grapple = reg.insert("grapple");
    defs.push(ItemDef {
        name: "grapple".to_string(),
        props: MaterialProps { mass: 0.8, volume: 0.0015, density: 7800.0, molar_mass: 0.0 },
        category: ItemCategory::Secondary,
    });

    let _circuit_board = reg.insert("circuit_board");
    defs.push(ItemDef {
        name: "circuit_board".to_string(),
        props: MaterialProps { mass: 0.05, volume: 0.0001, density: 1200.0, molar_mass: 0.0 },
        category: ItemCategory::Main,
    });

    let iron_ingot = reg.insert("iron_ingot");
    defs.push(ItemDef {
        name: "iron_ingot".to_string(),
        props: MaterialProps { mass: 0.25, volume: 0.00003, density: 7800.0, molar_mass: 55.85 },
        category: ItemCategory::Main,
    });

    reg.build_num_id();
    info!("Registered {} items: {:?}", reg.len(), reg.vec);

    items.apple = apple;
    items.coal = coal;
    items.stick = stick;
    items.frame = frame;
    items.lantern = lantern;
    items.pickaxe = pickaxe;
    items.shears = shears;
    items.grapple = grapple;
    items.iron_ingot = iron_ingot;

    items.atlas = asset_server.load("baked/items.png");
    items.defs = defs;
    // 可选：如需全局引用circuit_board，可加 pub circuit_board: RegId
}

fn setup_items_egui(
    mut items: ResMut<Items>,
    mut egui_ctx: bevy_egui::EguiContexts,
    mut initialized: Local<bool>,
) {
    if *initialized {
        return;
    }
    if items.atlas.id() == Handle::<Image>::default().id() {
        return;
    }

    items.atlas_egui = egui_ctx.add_image(bevy_egui::EguiTextureHandle::Strong(items.atlas.clone()));
    *initialized = true;
}

// use image::{self, GenericImageView, RgbaImage};

// fn bake_items(
//     mut items: ResMut<Items>,
//     asset_server: Res<AssetServer>,
// ) -> anyhow::Result<()> {

// // Generate Items Atlas Image
// let cache_file = std::env::current_dir()?.join("baked/items.png");
// let resolution = 64;

// if let Err(_) = std::fs::metadata(&cache_file) {
//     info!("Items Atlas Image cache not found, Generating...");

//     let n = items.registry.len() as u32;

//     let mut atlas = RgbaImage::new(n * resolution, resolution);

//     for (idx, str_id) in items.registry.vec.iter().enumerate() {
//         let idx = idx as u32;

//         let imgloc = if false {
//             // todo: ASSET_ROOT_PATH
//             format!("assets/textures/{str_id}/view.png")
//         } else {
//             format!("assets/items/{str_id}/view.png")
//         };

//         let img = image::open(imgloc)?;
//         let img = img.resize_exact(resolution, resolution, image::imageops::FilterType::Triangle);

//         // copy to
//         for y in 0..resolution {
//             for x in 0..resolution {
//                 atlas.put_pixel(idx*resolution + x, y, img.get_pixel(x, y));
//             }
//         }
//     }

//     std::fs::create_dir_all(&cache_file.parent().ok_or(crate::err_opt_is_none!())?)?;
//     atlas.save(&cache_file)?;
// }

// items.atlas = asset_server.load(cache_file);
// Ok(())
// }

// fn gen_items_atlas_image(cache_file: &str, resolution: u32) {

// }
