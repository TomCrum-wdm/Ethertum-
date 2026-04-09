mod vox;
mod chunk;
pub mod chunk_storage;
pub mod meshgen;
pub mod worldgen;
mod worldgen_gpu;
pub mod lighting;
mod voxel_client;
mod voxel_server;

mod render;

pub use chunk::Chunk;
pub use chunk_storage::{
    ActiveWorld,
    ChunkStore,
    LocalWorldInfo,
    WorldGenBackendPreference,
    WorldGenConfig,
    WorldTerrainMode,
    WorldStorage,
    WorldMeta,
    WorldSaveRequest,
    create_world,
    create_world_with_config,
    delete_world,
    list_worlds,
    load_world_meta,
    migrate_world_meta,
    save_world_meta,
    set_world_admin,
    sanitize_world_name,
    saves_root_dir,
    world_has_persisted_chunks,
};
pub use render::{TerrainMaterial};
pub use vox::{Vox, VoxShape, VoxTex, VoxLight,};
pub use voxel_client::{
    ClientChunkSystem,
    ClientVoxelPlugin,
    HitResult,
    VoxelBrush,
    VoxelChunkRenderMesh,
    VoxelMeshingStats,
    VoxelWorldGenStats,
};
pub use voxel_server::{ServerChunkSystem, ServerVoxelPlugin};

pub type ChunkPtr = Arc<Chunk>;

use crate::util::AsMutRef;
use bevy::{prelude::*, platform::collections::HashMap};
use std::sync::Arc;


#[derive(Resource, Deref, Clone)]
struct ChannelTx<T>(crate::channel_impl::Sender<T>);

#[derive(Resource, Deref, Clone)]
struct ChannelRx<T>(crate::channel_impl::Receiver<T>);

pub fn is_chunk_in_load_distance(mid_cp: IVec3, cp: IVec3, vd: IVec2) -> bool {
    (mid_cp.x - cp.x).abs() <= vd.x * Chunk::LEN && (mid_cp.z - cp.z).abs() <= vd.x * Chunk::LEN && (mid_cp.y - cp.y).abs() <= vd.y * Chunk::LEN
}

pub fn is_chunk_in_unload_distance(mid_cp: IVec3, cp: IVec3, vd: IVec2, margin_xy: i32, margin_y: i32) -> bool {
    let expand_xy = margin_xy.max(0);
    let expand_y = margin_y.max(0);
    let unload_vd = IVec2::new(vd.x.saturating_add(expand_xy), vd.y.saturating_add(expand_y));
    is_chunk_in_load_distance(mid_cp, cp, unload_vd)
}

// can_sustain_plant()

pub trait ChunkSystem {
    fn get_chunks(&self) -> &HashMap<IVec3, ChunkPtr>;

    fn get_chunk(&self, chunkpos: IVec3) -> Option<&ChunkPtr> {
        let cp = if Chunk::is_chunkpos(chunkpos) {
            chunkpos
        } else {
            debug!("Invalid chunkpos {}, normalized via as_chunkpos", chunkpos);
            Chunk::as_chunkpos(chunkpos)
        };
        self.get_chunks().get(&cp)
    }

    fn has_chunk(&self, chunkpos: IVec3) -> bool {
        let cp = if Chunk::is_chunkpos(chunkpos) {
            chunkpos
        } else {
            debug!("Invalid chunkpos {}, normalized via as_chunkpos", chunkpos);
            Chunk::as_chunkpos(chunkpos)
        };
        self.get_chunks().contains_key(&cp)
    }

    fn num_chunks(&self) -> usize {
        self.get_chunks().len()
    }

    fn get_voxel(&self, p: IVec3) -> Option<&Vox> {
        let chunkptr = self.get_chunk(Chunk::as_chunkpos(p))?;

        Some(chunkptr.at_voxel(Chunk::as_localpos(p)))
    }

    fn get_voxel_mut(&self, p: IVec3) -> Option<&mut Vox> {
        self.get_voxel(p).map(|v| v.as_mut())
    }
}

