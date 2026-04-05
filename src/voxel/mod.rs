mod vox;
mod chunk;
pub mod chunk_storage;
pub mod meshgen;
pub mod worldgen;
pub mod lighting;
mod voxel_client;
mod voxel_server;

mod render;

pub use chunk::Chunk;
pub use chunk_storage::{
    ActiveWorld,
    ChunkStore,
    LocalWorldInfo,
    WorldStorage,
    WorldMeta,
    WorldSaveRequest,
    create_world,
    delete_world,
    list_worlds,
    sanitize_world_name,
    saves_root_dir,
};
pub use render::{TerrainMaterial};
pub use vox::{Vox, VoxShape, VoxTex, VoxLight,};
pub use voxel_client::{ClientChunkSystem, ClientVoxelPlugin, HitResult, VoxelBrush};
pub use voxel_server::{ServerChunkSystem, ServerVoxelPlugin};

use std::sync::Mutex;
pub type ChunkPtr = Arc<Mutex<Chunk>>;

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

    fn get_voxel(&self, p: IVec3) -> Option<Vox> {
        let chunkptr = self.get_chunk(Chunk::as_chunkpos(p))?;
        let guard = crate::util::lock_arc(chunkptr);
        Some(*guard.at_voxel(Chunk::as_localpos(p)))
    }

    fn get_voxel_mut(&self, p: IVec3) -> Option<Vox> {
        // Return a copy of the voxel for callers that previously took a mutable
        // reference. To mutate the chunk, callers should lock the chunk and use
        // `at_voxel_mut` or `set_voxel_rel`.
        self.get_voxel(p)
    }

    /// Set voxel at global position `p` to value `v`. Returns the previous voxel if present.
    fn set_voxel(&self, p: IVec3, v: Vox) -> Option<Vox> {
        let chunkptr = self.get_chunk(Chunk::as_chunkpos(p))?;
        let mut guard = crate::util::lock_arc(chunkptr);
        Some(guard.set_voxel_rel(Chunk::as_localpos(p), |vox| { *vox = v; })?)
    }
}

