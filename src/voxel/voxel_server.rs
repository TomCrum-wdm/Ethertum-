use bevy::{
    prelude::*,
    tasks::AsyncComputeTaskPool,
    platform::collections::{HashMap, HashSet},
};
use bevy_renet::renet::{ClientId, DefaultChannel, RenetServer};
use avian3d::prelude::*;
use std::{collections::VecDeque, sync::Arc};

use super::{ChannelRx, ChannelTx, Chunk, ChunkPtr, ChunkSystem};
use crate::{
    net::{CellData, RenetServerHelper, SPacket},
    server::prelude::{ServerInfo, ServerSettings},
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
    server_settings: Res<ServerSettings>,
    mut cmds: Commands,
    active_world: Res<ActiveWorld>,
    mut save_req: ResMut<WorldSaveRequest>,

    mut chunks_loading: Local<HashSet<IVec3>>, // for detect/skip if is loading
    mut unload_cooldown_ticks: Local<HashMap<IVec3, u16>>,
    mut chunk_send_cooldown_sec: Local<HashMap<bevy_renet::renet::ClientId, f32>>,
    mut pending_chunk_send: Local<HashMap<ClientId, VecDeque<IVec3>>>,
    mut pending_chunk_send_set: Local<HashMap<ClientId, HashSet<IVec3>>>,
    mut last_world: Local<Option<ActiveWorld>>,
    tx_chunks_loading: Res<ChannelTx<ChunkLoadingData>>,
    rx_chunks_loading: Res<ChannelRx<ChunkLoadingData>>,
    time: Res<Time>,
) {
    const UNLOAD_MARGIN_XY: i32 = 1;
    const UNLOAD_MARGIN_Y: i32 = 1;
    const UNLOAD_COOLDOWN_TICKS: u16 = 60;
    let chunk_send_interval_sec = if server_settings.local_mode { 0.01 } else { 0.05 };
    let chunk_send_burst = if server_settings.local_mode { 6usize } else { 2usize };
    let chunk_send_budget_bytes = if server_settings.local_mode { 1_500_000usize } else { 400_000usize };
    let pending_queue_cap = if server_settings.local_mode { 4096usize } else { 1024usize };

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
        unload_cooldown_ticks.clear();
        pending_chunk_send.clear();
        pending_chunk_send_set.clear();
        for player in server.online_players.values_mut() {
            player.chunks_loaded.clear();
        }

        *last_world = Some(active_world.clone());
        info!("Switched active world to '{}' (seed={})", active_world.name, active_world.seed);
    }

    // Dispatch Chunk Load
    for player in server.online_players.values() {
        let vd = player.chunks_load_distance;
        let cp = player.last_valid_chunkpos;

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
                            super::worldgen::generate_chunk_with_seed(&mut chunk, &world.config, world.seed);
                            chunk
                        }
                        Err(err) => {
                            warn!("Failed to load chunk {} from world '{}': {}", chunkpos, world.name, err);
                            let mut chunk = Chunk::new(chunkpos);
                            super::worldgen::generate_chunk_with_seed(&mut chunk, &world.config, world.seed);
                            chunk
                        }
                    },
                    Err(err) => {
                        warn!("Failed to open world store '{}': {}", world.name, err);
                        let mut chunk = Chunk::new(chunkpos);
                        super::worldgen::generate_chunk_with_seed(&mut chunk, &world.config, world.seed);
                        chunk
                    }
                };

                chunk.is_populated = true;

                let chunkptr = Arc::new(chunk);
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
            let chunk = chunkptr.as_mut();

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
    let mut unload_budget = 8usize;
    for chunkpos in chunkpos_all {
        if unload_budget == 0 {
            break;
        }

        let mut any_desire = false;

        for player in server.online_players.values() {
            if crate::voxel::is_chunk_in_unload_distance(
                player.last_valid_chunkpos,
                chunkpos,
                player.chunks_load_distance,
                UNLOAD_MARGIN_XY,
                UNLOAD_MARGIN_Y,
            ) {
                any_desire = true;
                break;
            }
        }

        if any_desire {
            unload_cooldown_ticks.remove(&chunkpos);
            continue;
        }

        let cooldown = unload_cooldown_ticks.entry(chunkpos).or_insert(0);
        *cooldown = cooldown.saturating_add(1);
        if *cooldown < UNLOAD_COOLDOWN_TICKS {
            continue;
        }

        {
            let chunkptr = chunk_sys.despawn_chunk(chunkpos);
            let Some(chunkptr) = chunkptr else {
                continue;
            };

            // Avoid sync disk IO in the hot unload path.
            // Chunk persistence is handled by save_world on world switch/exit/manual save.

            let entity = chunkptr.as_ref().entity;
            cmds.entity(entity).despawn();

            // Do not broadcast ChunkDel from server hot path.
            // Clients already run local unload logic and this avoids reliable-channel pressure spikes.

            for player in server.online_players.values_mut() {
                player.chunks_loaded.remove(&chunkpos);
            }

            unload_cooldown_ticks.remove(&chunkpos);

            info!("Chunk Unloaded {}", chunk_sys.num_chunks());
            unload_budget -= 1;
        }
    }

    unload_cooldown_ticks.retain(|cp, _| chunk_sys.has_chunk(*cp));

    if save_req.save_now {
        save_req.save_now = false;
        match ChunkStore::new(&active_world).and_then(|store| store.save_world(chunk_sys.get_chunks())) {
            Ok(()) => info!("Manual world save completed for '{}'", active_world.name),
            Err(err) => warn!("Manual world save failed for '{}': {}", active_world.name, err),
        }
    }

    // Send Chunk to Players with queue + backpressure.
    pending_chunk_send.retain(|client_id, _| server.online_players.contains_key(client_id));
    pending_chunk_send_set.retain(|client_id, _| server.online_players.contains_key(client_id));

    for player in server.online_players.values_mut() {
        let queue = pending_chunk_send.entry(player.client_id).or_default();
        let queue_set = pending_chunk_send_set.entry(player.client_id).or_default();

        let vd = player.chunks_load_distance;
        let cp = player.last_valid_chunkpos;
        iter::iter_center_spread(vd.x, vd.y, |rp| {
            let chunkpos = rp * Chunk::LEN + cp;
            if queue.len() >= pending_queue_cap {
                return;
            }
            if player.chunks_loaded.contains(&chunkpos) || queue_set.contains(&chunkpos) {
                return;
            }
            if chunk_sys.has_chunk(chunkpos) {
                queue.push_back(chunkpos);
                queue_set.insert(chunkpos);
            }
        });

        let cooldown = chunk_send_cooldown_sec.entry(player.client_id).or_insert(0.0);
        if *cooldown > 0.0 {
            *cooldown = (*cooldown - time.delta_secs()).max(0.0);
            continue;
        }

        let mut num_sent = 0;
        let mut sent_bytes = 0usize;
        while num_sent < chunk_send_burst {
            let Some(chunkpos) = queue.pop_front() else {
                break;
            };
            queue_set.remove(&chunkpos);

            if player.chunks_loaded.contains(&chunkpos) {
                continue;
            }
            if !crate::voxel::is_chunk_in_unload_distance(cp, chunkpos, vd, UNLOAD_MARGIN_XY, UNLOAD_MARGIN_Y) {
                continue;
            }

            if let Some(chunkptr) = chunk_sys.get_chunk(chunkpos) {
                let data = CellData::from_chunk(chunkptr.as_ref());
                let approx_bytes = data.len() * std::mem::size_of::<CellData>() + 64;
                if sent_bytes + approx_bytes > chunk_send_budget_bytes && num_sent > 0 {
                    queue.push_front(chunkpos);
                    queue_set.insert(chunkpos);
                    break;
                }

                player.chunks_loaded.insert(chunkpos);
                num_sent += 1;
                sent_bytes += approx_bytes;

                net_server.send_packet_on(
                    player.client_id,
                    DefaultChannel::ReliableUnordered,
                    &SPacket::ChunkNew {
                        chunkpos,
                        voxel: data,
                    },
                );
            }
        }

        if num_sent > 0 {
            *chunk_send_cooldown_sec.entry(player.client_id).or_insert(0.0) = chunk_send_interval_sec;
        }
    }

    chunk_send_cooldown_sec.retain(|client_id, _| server.online_players.contains_key(client_id));
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
        let cp = chunkptr.as_ref().chunkpos;
        self.chunks.insert(cp, chunkptr);
    }

    fn despawn_chunk(&mut self, chunkpos: IVec3) -> Option<ChunkPtr> {
        self.chunks.remove(&chunkpos)
    }
}
