use bevy::math::{IVec2, IVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::voxel::{Chunk, Vox, VoxShape};
use crate::voxel::WorldGenConfig;

use super::EntityId;

// Compressed Cell data.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CellData {
    pub local_idx: u16, // 12 bits
    pub tex_id: u16,
    pub shape_id: VoxShape,
    pub isoval: u8,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NetItemStack {
    pub count: u8,
    pub item_id: u8,
    // 可选：同步物理属性（兼容旧协议，暂不强制使用）
    // pub mass: Option<f32>,
    // pub volume: Option<f32>,
    // pub density: Option<f32>,
    // pub molar_mass: Option<f32>,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InventoryDeltaEntry {
    pub slot: u16,
    pub stack: NetItemStack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AdminRequest {
    RequestState,
    ToggleGod,
    SetGod { enabled: bool },
    ToggleNoclip,
    SetNoclip { enabled: bool },
    SaveWorld,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AdminStateSnapshot {
    pub is_owner: bool,
    pub is_admin: bool,
    pub god_enabled: bool,
    pub noclip_enabled: bool,
}

impl CellData {
    pub fn from_cell(local_idx: u16, c: &Vox) -> Self {
        Self {
            local_idx,
            tex_id: c.tex_id,
            shape_id: c.shape_id,
            isoval: c.isoval,
        }
    }

    pub fn from_chunk(chunk: &Chunk) -> Vec<CellData> {
        let mut data = Vec::new();
        for i in 0..Chunk::LEN3 {
            let c = chunk.at_voxel(Chunk::local_idx_pos(i as i32));
            if !c.is_nil() {
                // FIXED: Dont use {isovalue() > -0.5} as condition, because Non-Isosurface voxels e.g. Leaves should always be transmit regardless it's isovalue
                // dens: ((c.value + 0.5).clamp(0.0, 1.0) * 255.0) as u8
                data.push(CellData::from_cell(i as u16, c));
            }
        }
        data
    }
    pub fn to_chunk(data: &Vec<CellData>, chunk: &mut Chunk) {
        for c in data {
            let mut a = Vox::new(c.tex_id, c.shape_id, 0.0);
            a.isoval = c.isoval;
            *chunk.at_voxel_mut(Chunk::local_idx_pos(c.local_idx as i32)) = a;
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CPacket {
    // Handshake & Server Query & Login
    Handshake { protocol_version: u64 },
    ServerQuery {},
    Ping { client_time: u64, last_rtt: u32 }, // last_rtt is a temporary solution to let server know the client's ping

    Login { uuid: u64, access_token: u64, username: String },

    // Play
    ChatMessage { message: String },

    PlayerPos { position: Vec3 },

    PlayerList, // RequestPlayerList

    InventorySwap {
        a: u16,
        b: u16,
    },

    AdminRequest { request: AdminRequest },

    ChunkModify { chunkpos: IVec3, voxel: Vec<CellData> },

    LoadDistance { load_distance: IVec2 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SPacket {
    // Handshake & Server Query & Login
    Disconnect {
        reason: String,
    },
    // ServerInfo {
    //     motd: String,
    //     num_players_limit: u32,
    //     num_players_online: u32,
    //     // online_players: Vec<(u64 uuid, String name)>
    //     protocol_version: u64,
    //     favicon: String,
    // },
    Pong {
        client_time: u64,
        server_time: u64,
    },
    LoginSuccess {
        // uuid, username
        player_entity: EntityId,
        spawn_position: Vec3,
    },

    AdminState {
        state: AdminStateSnapshot,
    },

    WorldInit {
        world_name: String,
        seed: u64,
        world_config: WorldGenConfig,
    },

    // Play
    Chat {
        message: String,
    },

    EntityNew {
        entity_id: EntityId,
        name: String, // temporary way.
                      // type: {Player}
        position: Vec3,
    },
    EntityDel {
        entity_id: EntityId,
    },
    EntityPos {
        entity_id: EntityId,
        position: Vec3,
    },

    PlayerList {
        // name, ping
        playerlist: Vec<(String, u32)>,
    },

    InventorySync {
        slots: Vec<NetItemStack>,
    },

    InventoryDelta {
        changes: Vec<InventoryDeltaEntry>,
    },

    ChunkNew {
        chunkpos: IVec3,
        voxel: Vec<CellData>, // or use full-chunk fixed array?
    },
    ChunkDel {
        chunkpos: IVec3,
    },
    ChunkModify {
        chunkpos: IVec3,
        voxel: Vec<CellData>,
    },

    WorldTime {
        daytime: f32,
    },
}
