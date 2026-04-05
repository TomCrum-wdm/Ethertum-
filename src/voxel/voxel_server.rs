use bevy::{
    prelude::*,
    tasks::AsyncComputeTaskPool,
    platform::collections::{HashMap, HashSet},
};
use bevy_renet::renet::RenetServer;
use avian3d::prelude::*;
use std::sync::Arc;

use super::{ChannelRx, ChannelTx, Chunk, ChunkPtr, ChunkSystem};
use crate::{
    net::{CellData, RenetServerHelper, SPacket},
    server::prelude::ServerInfo,
    util::{iter, AsMutRef},
    voxel::{ActiveWorld, ChunkStore, WorldSaveRequest},
};

type ChunkLoadingData = (IVec3, ChunkPtr);

pub struct ServerVoxelPlugin;

impl Plugin for ServerVoxelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ServerChunkSystem::new());
        app.init_resource::<ActiveWorld>();
        app.init_resource::<WorldSaveRequest>();

        {
            let (tx, rx) = crate::channel_impl::unbounded::<ChunkLoadingData>();
            app.insert_resource(ChannelTx(tx));
            app.insert_resource(ChannelRx(rx));
        }

        app.add_systems(Update, chunks_load);
        app.add_systems(Last, save_chunks_on_exit);
    }
}

fn save_chunks_on_exit(
    mut exit_events: EventReader<bevy::app::AppExit>,
    chunk_sys: Res<ServerChunkSystem>,
    active_world: Res<ActiveWorld>,
) {
    for _ in exit_events.read() {
        match ChunkStore::new(&active_world).and_then(|store| store.save_world(chunk_sys.get_chunks())) {
            Ok(()) => info!("Saved world '{}' on app exit", active_world.name),
            Err(err) => warn!("Failed to save world '{}' on app exit: {}", active_world.name, err),
        }
    }
}

fn chunks_load(
    mut chunk_sys: ResMut<ServerChunkSystem>,
    mut net_server: ResMut<RenetServer>,
    mut server: ResMut<ServerInfo>,
    mut cmds: Commands,
    active_world: Res<ActiveWorld>,
    mut save_req: ResMut<WorldSaveRequest>,

    mut chunks_loading: Local<HashSet<IVec3>>, // for detect/skip if is loading
    mut last_world: Local<Option<ActiveWorld>>,
    tx_chunks_loading: Res<ChannelTx<ChunkLoadingData>>,
    rx_chunks_loading: Res<ChannelRx<ChunkLoadingData>>,
) {
    // todo
    // 优化: 仅当某玩家 进入/退出 移动过区块边界时，才针对更新
    // 待改进: 这里可能有多种加载方法，包括Inner-Outer近距离优先加载，填充IVec3待加载列表并排序方法

    if last_world
        .as_ref()
        .is_none_or(|w| w.name != active_world.name || w.seed != active_world.seed)
    {
        if let Some(prev_world) = last_world.as_ref() {
            match ChunkStore::new(prev_world).and_then(|store| store.save_world(chunk_sys.get_chunks())) {
                Ok(()) => info!("Saved previous world '{}' before switch", prev_world.name),
                Err(err) => warn!("Failed to save previous world '{}' before switch: {}", prev_world.name, err),
            }
        }

        let entities: Vec<Entity> = chunk_sys
            .get_chunks()
            .values()
            .map(|chunk| chunk.entity)
            .collect();
        for entity in entities {
            cmds.entity(entity).despawn();
        }
        chunk_sys.chunks.clear();
        chunks_loading.clear();
        for player in server.online_players.values_mut() {
            player.chunks_loaded.clear();
        }

        *last_world = Some(active_world.clone());
        info!("Switched active world to '{}' (seed={})", active_world.name, active_world.seed);
    }

    // Dispatch Chunk Load
    for player in server.online_players.values() {
        let vd = player.chunks_load_distance;
        let cp = Chunk::as_chunkpos(player.position.as_ivec3());

        iter::iter_center_spread(vd.x, vd.y, |rp| {
            let chunkpos = rp * Chunk::LEN + cp;
            if chunks_loading.len() > 8 {
                // max_concurrent_loading_chunks
                return;
            }
            if chunk_sys.has_chunk(chunkpos) || chunks_loading.contains(&chunkpos) {
                return;
            }

            let tx = tx_chunks_loading.clone();
            let world = active_world.clone();
            let task = AsyncComputeTaskPool::get().spawn(async move {
                let mut chunk = match ChunkStore::new(&world) {
                    Ok(store) => match store.load_chunk(chunkpos) {
                        Ok(Some(chunk)) => {
                            info!("Load Chunk from disk {} ({})", chunkpos, world.name);
                            chunk
                        }
                        Ok(None) => {
                            let mut chunk = Chunk::new(chunkpos);
                            let settings = crate::client::settings::ClientSettings::default();
                            super::worldgen::generate_chunk_with_seed(&mut chunk, &settings, world.seed as u32);
                            chunk
                        }
                        Err(err) => {
                            warn!("Failed to load chunk {} from world '{}': {}", chunkpos, world.name, err);
                            let mut chunk = Chunk::new(chunkpos);
                            let settings = crate::client::settings::ClientSettings::default();
                            super::worldgen::generate_chunk_with_seed(&mut chunk, &settings, world.seed as u32);
                            chunk
                        }
                    },
                    Err(err) => {
                        warn!("Failed to open world store '{}': {}", world.name, err);
                        let mut chunk = Chunk::new(chunkpos);
                        let settings = crate::client::settings::ClientSettings::default();
                        super::worldgen::generate_chunk_with_seed(&mut chunk, &settings, world.seed as u32);
                        chunk
                    }
                };

                chunk.is_populated = true;

                let chunkptr = Arc::new(std::sync::Mutex::new(chunk));
                if tx.send((chunkpos, chunkptr)).is_err() {
                    warn!("Server chunk loading channel closed");
                }
            });

            task.detach();
            chunks_loading.insert(chunkpos);

            info!("ChunkLoad Enqueue {} / {}", chunk_sys.num_chunks(), chunkpos);
        });
    }

    // Complete Chunk Load
        while let Ok((chunkpos, chunkptr)) = rx_chunks_loading.try_recv() {
        chunks_loading.remove(&chunkpos);

        {
            let mut chunk = crate::util::lock_arc(&chunkptr);

            chunk.entity = cmds
                .spawn((
                    // ChunkComponent::new(chunkpos),
                    Transform::from_translation(chunkpos.as_vec3()),
                    GlobalTransform::IDENTITY, // really?
                    RigidBody::Static,
                ))
                .id();
        }

        chunk_sys.spawn_chunk(chunkptr);

        info!("ChunkLoad Completed {} / {}", chunk_sys.num_chunks(), chunkpos);
    }

    // Unload Chunks
    // 野蛮区块卸载检测
    let chunkpos_all = Vec::from_iter(chunk_sys.get_chunks().keys().cloned());
    for chunkpos in chunkpos_all {
        let mut any_desire = false;

        for player in server.online_players.values_mut() {
            if crate::voxel::is_chunk_in_load_distance(Chunk::as_chunkpos(player.position.as_ivec3()), chunkpos, player.chunks_load_distance) {
                any_desire = true;
            }
        }

        if !any_desire {
            let chunkptr = chunk_sys.despawn_chunk(chunkpos);
            let Some(chunkptr) = chunkptr else {
                continue;
            };
            if let Ok(store) = ChunkStore::new(&active_world) {
                if let Err(err) = store.save_chunk(chunkptr.as_ref()) {
                    warn!("Failed to save chunk {} in world '{}': {}", chunkpos, active_world.name, err);
                }
            }
            let entity = chunkptr.as_ref().entity;
            cmds.entity(entity).despawn();

            net_server.broadcast_packet(&SPacket::ChunkDel { chunkpos });

            for player in server.online_players.values_mut() {
                player.chunks_loaded.remove(&chunkpos);
            }

            info!("Chunk Unloaded {}", chunk_sys.num_chunks());
        }
    }

    if save_req.save_now {
        save_req.save_now = false;
        match ChunkStore::new(&active_world).and_then(|store| store.save_world(chunk_sys.get_chunks())) {
            Ok(()) => info!("Manual world save completed for '{}'", active_world.name),
            Err(err) => warn!("Manual world save failed for '{}': {}", active_world.name, err),
        }
    }

    // Send Chunk to Players
    // 野蛮的方法 把所有区块发给所有玩家
    for player in server.online_players.values_mut() {
        let vd = player.chunks_load_distance;
        let cp = Chunk::as_chunkpos(player.position.as_ivec3());

        let mut num_sent = 0;

        iter::iter_center_spread(vd.x, vd.y, |rp| {
            let chunkpos = rp * Chunk::LEN + cp;
            if num_sent > 4 {
                // 不能一次性给玩家发送太多数据包 否则会溢出缓冲区 "send channel 2 with error: reliable channel memory usage was exausted"
                return;
            }
            if player.chunks_loaded.contains(&chunkpos) {
                return;
            }
                if let Some(chunkptr) = chunk_sys.get_chunk(chunkpos) {
                player.chunks_loaded.insert(chunkpos);
                num_sent += 1;

                info!(
                    "Send Chunk {}/{} {} to Player {}",
                    player.chunks_loaded.len(),
                    chunk_sys.num_chunks(),
                    num_sent,
                    player.username
                );
                let data = {
                    let guard = crate::util::lock_arc(&chunkptr);
                    CellData::from_chunk(&*guard)
                };
                net_server.send_packet(player.client_id, &SPacket::ChunkNew { chunkpos, voxel: data });
            }
        });
    }
}

#[derive(Resource)]
pub struct ServerChunkSystem {
    pub chunks: HashMap<IVec3, ChunkPtr>,
}

impl ChunkSystem for ServerChunkSystem {
    fn get_chunks(&self) -> &HashMap<IVec3, ChunkPtr> {
        &self.chunks
    }
}

impl ServerChunkSystem {
    fn new() -> Self {
        Self { chunks: HashMap::default() }
    }

    fn spawn_chunk(&mut self, chunkptr: ChunkPtr) {
        let cp = {
            let guard = crate::util::lock_arc(&chunkptr);
            guard.chunkpos
        };
        self.chunks.insert(cp, chunkptr);
    }

    fn despawn_chunk(&mut self, chunkpos: IVec3) -> Option<ChunkPtr> {
        self.chunks.remove(&chunkpos)
    }
}
