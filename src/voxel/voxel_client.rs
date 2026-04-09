use bevy::{
    color::palettes::css, pbr::{ExtendedMaterial, MaterialExtension}, prelude::*, render::{
        render_resource::PrimitiveTopology,
    }, tasks::AsyncComputeTaskPool, platform::collections::{HashMap, HashSet},
    asset::{RenderAssetUsages},
    image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
};
use avian3d::prelude::*;
use bevy_renet::renet::RenetClient;
use leafwing_input_manager::action_state::ActionState;

use super::{meshgen, render::{self, FoliageMaterial, LiquidMaterial, TerrainMaterial}, ChannelRx, ChannelTx, Chunk, ChunkPtr, ChunkSystem, VoxShape, WorldGenConfig};
use crate::{
    client::prelude::*,
    net::{CPacket, CellData, RenetClientHelper},
    util::{iter, AsMutRef},
};

#[derive(Resource)]
struct AssetDebug {
    albedo: Handle<Image>,//HANDLE是bevy的资源句柄类型，用于引用加载的资源。在这个代码中，albedo 是一个 Handle<Image> 类型的字段，表示一个图像资源的句柄。通过这个句柄，可以在 Bevy 的资源系统中访问和使用这个图像资源。Handle 是 Bevy 中用于管理和访问资源的一种方式，它提供了对资源的引用，而不需要直接持有资源的数据。这使得资源的管理更加高效和灵活。
    normal: Handle<Image>,
    dram: Handle<Image>,
    foliage_diff: Handle<Image>,
    water_normals: Handle<Image>,
}

// UI button removed to avoid requiring bevy UI on all targets

pub struct ClientVoxelPlugin;

#[derive(Component)]
pub struct VoxelChunkRenderMesh;

#[derive(Resource, Debug, Default)]
pub struct VoxelMeshingStats {
    pub remesh_queue: usize,
    pub meshing_inflight: usize,
    pub fast_pending_upgrade: usize,
    pub submitted_surface_this_frame: usize,
    pub submitted_full_this_frame: usize,
    pub completed_surface_total: u64,
    pub completed_full_total: u64,
    pub surface_first_enabled: bool,
    pub surface_only_enabled: bool,
}

#[derive(Resource, Debug, Default)]
pub struct VoxelWorldGenStats {
    pub gpu_enabled: bool,
    pub batch_size: usize,
    pub force_cpu_persisted_world: bool,
    pub loading_queue: usize,
    pub loading_inflight: usize,
    pub submitted_gpu_this_frame: usize,
    pub submitted_cpu_this_frame: usize,
    pub completed_gpu_total: u64,
    pub completed_cpu_total: u64,
    pub gpu_fallback_total: u64,
    pub last_backend_label: &'static str,
}

impl Plugin for ClientVoxelPlugin {
    fn build(&self, app: &mut App) {

        render::init(app);
        
        {
            let (tx, rx) = crate::channel_impl::unbounded::<ChunkRemeshData>();
            app.insert_resource(ChannelTx(tx));
            app.insert_resource(ChannelRx(rx));

            let (tx, rx) = crate::channel_impl::unbounded::<ChunkLoadingData>();
            app.insert_resource(ChannelTx(tx));
            app.insert_resource(ChannelRx(rx));
        }

        app.add_systems(First, on_world_init.run_if(condition::load_world));
        // Register periodic file-export debug writer (portable across targets)
        app.add_systems(Update, write_debug_file_system.run_if(condition::in_world));
        app.add_systems(Last, on_world_exit.run_if(condition::unload_world()));

        app.insert_resource(VoxelBrush::default());
        app.register_type::<VoxelBrush>();

        app.insert_resource(VoxelMeshingStats::default());
        app.insert_resource(VoxelWorldGenStats::default());

        app.insert_resource(HitResult::default());
        app.register_type::<HitResult>();

        // app.add_systems(PreUpdate, raycast.run_if(condition::in_world));

        app.add_systems(
            Update,
            (
                raycast,
                chunks_detect_load_and_unload,
                chunks_remesh_enqueue,
                draw_gizmos,
                draw_crosshair_cube.in_set(PhysicsSet::Writeback),
            )
            .chain()
            .run_if(condition::in_world),
        );

        // Draw Crosshair
        // app.add_systems(PostUpdate, draw_crosshair_cube.after(bevy_xpbd_3d::PhysicsSet::Sync).before(bevy::transform::TransformSystem::TransformPropagate));
    }
}

fn on_world_init(
    mut cmds: Commands,
    worldinfo: Res<WorldInfo>,
    asset_server: Res<AssetServer>,
    mut mtls_terrain: ResMut<Assets<ExtendedMaterial<StandardMaterial, TerrainMaterial>>>,
    mut mtls_foliage: ResMut<Assets<ExtendedMaterial<StandardMaterial, FoliageMaterial>>>,
    mut mtls_liquid: ResMut<Assets<ExtendedMaterial<StandardMaterial, LiquidMaterial>>>,
    // mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Init ClientChunkSystem");
    let mut chunk_sys = ClientChunkSystem::new();
    chunk_sys.world_config = worldinfo.world_config.clone();
    // Load images first and keep handles for debug/inspection on device
    let albedo_h = asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        "baked/atlas_diff.png",
        |settings| {
            settings.is_srgb = true;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        },
    );
    let normal_h = asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        "baked/atlas_norm.png",
        |settings| {
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        },
    );
    let dram_h = asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        "baked/atlas_dram.png",
        |settings| {
            settings.is_srgb = true;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        },
    );

    chunk_sys.mtl_terrain = mtls_terrain.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color_texture: Some(albedo_h.clone()),
            normal_map_texture: Some(normal_h.clone()),
            alpha_mode: AlphaMode::Opaque,
            ..default()
        },
        extension: TerrainMaterial {
            dram_texture: Some(dram_h.clone()),
            ..default()
        }
    });

    let foliage_h = asset_server.load("baked/atlas_diff_foli.png");
    chunk_sys.mtl_foliage = mtls_foliage.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: css::BLACK.into(),
            base_color_texture: Some(foliage_h.clone()),
            perceptual_roughness: 0.0,
            ..default()
        },
        extension: FoliageMaterial {
            ..default()
        }
    });

    let water_norm_h = asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        "water_normals.png",
        |settings| {
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        },
    );

    chunk_sys.mtl_liquid = mtls_liquid.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: css::BLACK.into(),
            perceptual_roughness: 0.0,
            ..default()
        },
        extension: LiquidMaterial {
            normals: water_norm_h.clone(),
        },
    });

    // store debug handles (no UI; write debug file to disk)
    cmds.insert_resource(AssetDebug {
        albedo: albedo_h,
        normal: normal_h,
        dram: dram_h,
        foliage_diff: foliage_h,
        water_normals: water_norm_h,
    });

    // UI button removed; debug export will be file-based and periodic so it is portable

    // ChunkSystem entity. all chunk entities will be spawn as children. (for almost no reason. just for editor hierarchy)
    chunk_sys.entity = cmds
        .spawn((
            Name::new("ChunkSystem"),
            InheritedVisibility::VISIBLE,
            GlobalTransform::IDENTITY,
            Transform::IDENTITY,
            DespawnOnWorldUnload,
        ))
        .id();

    cmds.insert_resource(chunk_sys);
}

fn on_world_exit(mut cmds: Commands) {
    info!("Clear ClientChunkSystem");
    cmds.remove_resource::<ClientChunkSystem>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkGenBackend {
    Cpu,
    GpuFastPath,
}

type ChunkLoadingData = (Chunk, ChunkGenBackend);

fn chunks_detect_load_and_unload(
    query_cam: Query<&Transform, With<CharacterControllerCamera>>,
    mut chunk_sys: ResMut<ClientChunkSystem>,
    mut chunks_loading: Local<HashSet<IVec3>>, // for detect/skip if is loading
    mut loading_started_frame: Local<HashMap<IVec3, u64>>,
    mut unload_cooldown_ticks: Local<HashMap<IVec3, u16>>,
    mut last_valid_viewer_cp: Local<Option<IVec3>>,
    mut frame_counter: Local<u64>,
    cfg: Res<ClientSettings>,
    worldinfo: Res<WorldInfo>,
    mut worldgen_stats: ResMut<VoxelWorldGenStats>,

    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,

    tx_chunk_load: Res<ChannelTx<ChunkLoadingData>>,
    rx_chunk_load: Res<ChannelRx<ChunkLoadingData>>,
    mut fallback_seen_total: Local<u64>,
) {
    const UNLOAD_MARGIN_XY: i32 = 1;
    const UNLOAD_MARGIN_Y: i32 = 1;
    const UNLOAD_COOLDOWN_TICKS: u16 = 20;
    const LOADING_TIMEOUT_FRAMES: u64 = 600;

    *frame_counter = frame_counter.saturating_add(1);

    let vp = if let Ok(cam_transform) = query_cam.single() {
        if cam_transform.translation.is_finite() {
            let cp = Chunk::as_chunkpos(cam_transform.translation.as_ivec3());
            *last_valid_viewer_cp = Some(cp);
            cp
        } else if let Some(cp) = *last_valid_viewer_cp {
            cp
        } else {
            return;
        }
    } else if let Some(cp) = *last_valid_viewer_cp {
        cp
    } else {
        return;
    };
    #[cfg(target_arch = "wasm32")]
    let vd = IVec2::new(cfg.chunks_load_distance.x.max(2), cfg.chunks_load_distance.y.max(1));
    #[cfg(not(target_arch = "wasm32"))]
    let vd = IVec2::new(cfg.chunks_load_distance.x.max(2), cfg.chunks_load_distance.y.max(1));
    let world_config = worldinfo.world_config.clone();
    let world_seed = worldinfo.seed;

    // Recover from stalled async generation entries so these chunk positions can be retried.
    let mut stale = Vec::new();
    for (cp, start_frame) in loading_started_frame.iter() {
        if frame_counter.saturating_sub(*start_frame) > LOADING_TIMEOUT_FRAMES {
            stale.push(*cp);
        }
    }
    for cp in stale {
        loading_started_frame.remove(&cp);
        if chunks_loading.remove(&cp) {
            warn!("Worldgen loading timeout recovered at {}", cp);
        }
    }

    let has_persisted_chunks = crate::voxel::world_has_persisted_chunks(&worldinfo.name);
    let force_cpu_persisted = has_persisted_chunks && !cfg.gpu_worldgen_allow_persisted_world;
    let backend_allows_gpu = match world_config.worldgen_backend {
        crate::voxel::WorldGenBackendPreference::Auto => true,
        crate::voxel::WorldGenBackendPreference::CpuCompatible => false,
        crate::voxel::WorldGenBackendPreference::GpuFast => true,
    };
    let force_cpu_backend_pref = !backend_allows_gpu;
    let allow_gpu_worldgen = cfg.gpu_worldgen && !force_cpu_persisted && backend_allows_gpu;

    let batch_size = cfg.gpu_worldgen_batch_size.max(1) as usize;
    let cpu_max_loading = cfg.cpu_worldgen_max_loading.clamp(1, 64) as usize;
    let gpu_max_loading = cfg.gpu_worldgen_max_loading.clamp(16, 1024) as usize;
    let backlog_mid = cfg.gpu_worldgen_adaptive_backlog_mid.clamp(1, 10_000) as usize;
    let backlog_high = cfg
        .gpu_worldgen_adaptive_backlog_high
        .clamp(cfg.gpu_worldgen_adaptive_backlog_mid.max(1), 20_000) as usize;
    let mult_low = cfg.gpu_worldgen_adaptive_mult_low.clamp(1, 32) as usize;
    let mult_mid = cfg.gpu_worldgen_adaptive_mult_mid.clamp(mult_low as i32, 64) as usize;
    let mult_high = cfg.gpu_worldgen_adaptive_mult_high.clamp(mult_mid as i32, 128) as usize;
    let adaptive_batch_min = cfg.gpu_worldgen_adaptive_batch_min.clamp(1, 2048) as usize;
    let adaptive_batch_max = cfg
        .gpu_worldgen_adaptive_batch_max
        .clamp(cfg.gpu_worldgen_adaptive_batch_min.max(1), 4096) as usize;

    let max_loading = if cfg!(target_arch = "wasm32") {
        2
    } else if allow_gpu_worldgen {
        // GPU path can process wider batches efficiently; size is user-configurable.
        gpu_max_loading
    } else {
        cpu_max_loading
    };

    worldgen_stats.gpu_enabled = allow_gpu_worldgen;
    worldgen_stats.batch_size = cfg.gpu_worldgen_batch_size.max(1) as usize;
    worldgen_stats.force_cpu_persisted_world = cfg.gpu_worldgen && force_cpu_persisted;
    worldgen_stats.loading_inflight = chunks_loading.len();
    worldgen_stats.submitted_gpu_this_frame = 0;
    worldgen_stats.submitted_cpu_this_frame = 0;
    worldgen_stats.gpu_fallback_total = super::worldgen::gpu_worldgen_fallback_total();

    if worldgen_stats.force_cpu_persisted_world {
        worldgen_stats.last_backend_label = "CPU (Persisted Save Compat)";
    } else if force_cpu_backend_pref {
        worldgen_stats.last_backend_label = "CPU (World Pref: Compatible)";
    }

    let mut pending_positions = Vec::new();

    // Chunks Detect Load/Gen

    iter::iter_center_spread(vd.x, vd.y, |rp| {
        if chunks_loading.len() > max_loading {
            //chunk_sys.max_concurrent_loading {
            return;
        }
        let chunkpos = rp * Chunk::LEN + vp;

        // the chunk already exists, skip.
        if chunk_sys.has_chunk(chunkpos) || chunks_loading.contains(&chunkpos) {
            return;
        }

        pending_positions.push(chunkpos);
    });

    while !pending_positions.is_empty() && chunks_loading.len() < max_loading {
        let remaining = max_loading.saturating_sub(chunks_loading.len());
        if remaining == 0 {
            break;
        }

        let can_use_gpu = allow_gpu_worldgen && pending_positions.len() >= 2;
        if can_use_gpu {
            // Adaptive batching: increase batch size when queue backlog is large.
            let adaptive_batch_size = if pending_positions.len() > backlog_high {
                (batch_size * mult_high).clamp(adaptive_batch_min, adaptive_batch_max)
            } else if pending_positions.len() > backlog_mid {
                (batch_size * mult_mid).clamp(adaptive_batch_min, adaptive_batch_max)
            } else {
                (batch_size * mult_low).clamp(adaptive_batch_min, adaptive_batch_max)
            };
            let take_n = pending_positions.len().min(adaptive_batch_size).min(remaining);
            let positions: Vec<IVec3> = pending_positions.drain(0..take_n).collect();
            let submitted_gpu = positions.len();
            for &chunkpos in &positions {
                chunks_loading.insert(chunkpos);
                loading_started_frame.insert(chunkpos, *frame_counter);
            }

            let tx = tx_chunk_load.clone();
            let world_config = world_config.clone();
            let world_seed = world_seed;
            let positions_for_task = positions;
            let task = AsyncComputeTaskPool::get().spawn(async move {
                let chunks = super::worldgen::generate_chunks_gpu_batched(&positions_for_task, &world_config, world_seed);
                for chunk in chunks {
                    if tx.send((chunk, ChunkGenBackend::GpuFastPath)).is_err() {
                        warn!("Chunk loading channel closed");
                        break;
                    }
                }
            });
            task.detach();
            worldgen_stats.last_backend_label = "GPU";
            worldgen_stats.submitted_gpu_this_frame += submitted_gpu;
            continue;
        }

        let chunkpos = pending_positions.remove(0);

        let tx = tx_chunk_load.clone();
        let world_config = world_config.clone();
        let world_seed = world_seed;
        let task = AsyncComputeTaskPool::get().spawn(async move {
            // info!("Load Chunk: {:?}", chunkpos);
            let mut chunk = Chunk::new(chunkpos);

            super::worldgen::generate_chunk(&mut chunk, &world_config, world_seed);

            if tx.send((chunk, ChunkGenBackend::Cpu)).is_err() {
                warn!("Chunk loading channel closed");
            }
        });
        task.detach();
        chunks_loading.insert(chunkpos);
        loading_started_frame.insert(chunkpos, *frame_counter);
        if worldgen_stats.force_cpu_persisted_world {
            worldgen_stats.last_backend_label = "CPU (Persisted Save Compat)";
        } else if force_cpu_backend_pref {
            worldgen_stats.last_backend_label = "CPU (World Pref: Compatible)";
        } else {
            worldgen_stats.last_backend_label = "CPU";
        }
        worldgen_stats.submitted_cpu_this_frame += 1;
    }

    while let Ok((chunk, backend)) = rx_chunk_load.try_recv() {
        chunks_loading.remove(&chunk.chunkpos);
        loading_started_frame.remove(&chunk.chunkpos);

        match backend {
            ChunkGenBackend::Cpu => {
                worldgen_stats.completed_cpu_total += 1;
                if worldgen_stats.force_cpu_persisted_world {
                    worldgen_stats.last_backend_label = "CPU (Persisted Save Compat)";
                } else if force_cpu_backend_pref {
                    worldgen_stats.last_backend_label = "CPU (World Pref: Compatible)";
                } else {
                    worldgen_stats.last_backend_label = "CPU";
                }
            }
            ChunkGenBackend::GpuFastPath => {
                worldgen_stats.completed_gpu_total += 1;
                worldgen_stats.last_backend_label = "GPU";
            }
        }

        chunk_sys.spawn_chunk(chunk, &mut cmds, &mut meshes);
    }

    worldgen_stats.loading_queue = pending_positions.len();
    worldgen_stats.loading_inflight = chunks_loading.len();
    worldgen_stats.gpu_fallback_total = super::worldgen::gpu_worldgen_fallback_total();
    if worldgen_stats.gpu_fallback_total > *fallback_seen_total {
        worldgen_stats.last_backend_label = "GPU->CPU FALLBACK";
        *fallback_seen_total = worldgen_stats.gpu_fallback_total;
    }

    // Chunks Unload

    let chunkpos_all = Vec::from_iter(chunk_sys.get_chunks().keys().cloned());
    for chunkpos in chunkpos_all {
        if crate::voxel::is_chunk_in_unload_distance(vp, chunkpos, vd, UNLOAD_MARGIN_XY, UNLOAD_MARGIN_Y) {
            unload_cooldown_ticks.remove(&chunkpos);
            continue;
        }

        let cooldown = unload_cooldown_ticks.entry(chunkpos).or_insert(0);
        *cooldown = cooldown.saturating_add(1);
        if *cooldown >= UNLOAD_COOLDOWN_TICKS {
            chunk_sys.despawn_chunk(chunkpos, &mut cmds);
            unload_cooldown_ticks.remove(&chunkpos);
        }
    }

    unload_cooldown_ticks.retain(|cp, _| chunk_sys.has_chunk(*cp));
    loading_started_frame.retain(|cp, _| chunks_loading.contains(cp));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkMeshStage {
    SurfaceFast,
    FullQuality,
}

type ChunkRemeshData = (IVec3, Entity, Mesh, Handle<Mesh>, Option<Collider>, Mesh, Handle<Mesh>, Mesh, Handle<Mesh>, ChunkMeshStage);

use once_cell::sync::Lazy;
use std::{cell::RefCell, sync::Arc};
use thread_local::ThreadLocal;
use crate::util::vtx::VertexBuffer;

static THREAD_LOCAL_VERTEX_BUFFERS: Lazy<ThreadLocal<RefCell<(VertexBuffer, VertexBuffer, VertexBuffer)>>> = Lazy::new(ThreadLocal::default);

fn chunks_remesh_enqueue(
    mut commands: Commands,

    query_cam: Query<&Transform, With<CharacterControllerCamera>>,
    mut chunk_sys: ResMut<ClientChunkSystem>,
    cfg: Res<ClientSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshing_stats: ResMut<VoxelMeshingStats>,

    tx_chunks_meshing: Res<ChannelTx<ChunkRemeshData>>,
    rx_chunks_meshing: Res<ChannelRx<ChunkRemeshData>>,
    mut fast_ready_chunks: Local<HashSet<IVec3>>,

    // mut foliage_mtls: ResMut<Assets<FoliageMaterial>>,
    // time: Res<Time>,
) {
    // foliage_mtls.get_mut(chunk_sys.mtl_foliage.id()).unwrap().time = time.elapsed_seconds();

    if !cfg.surface_first_meshing && !cfg.surface_only_meshing {
        fast_ready_chunks.clear();
    } else {
        fast_ready_chunks.retain(|cp| chunk_sys.has_chunk(*cp));
    }

    meshing_stats.surface_first_enabled = cfg.surface_first_meshing;
    meshing_stats.surface_only_enabled = cfg.surface_only_meshing;
    meshing_stats.remesh_queue = chunk_sys.chunks_remesh.len();
    meshing_stats.meshing_inflight = chunk_sys.chunks_meshing.len();
    meshing_stats.submitted_surface_this_frame = 0;
    meshing_stats.submitted_full_this_frame = 0;

    let mut chunks_remesh = Vec::from_iter(chunk_sys.chunks_remesh.iter().cloned());

    // Sort by Distance from the Camera.
    let Ok(cam_transform) = query_cam.single() else {
        return;
    };
    let cam_pos = cam_transform.translation;
    let cam_forward = *cam_transform.forward();
    let half_fov_cos = (cfg.fov.to_radians() * 0.5).cos();
    let cam_cp = Chunk::as_chunkpos(cam_pos.as_ivec3());
    chunks_remesh.sort_unstable_by_key(|cp: &IVec3| {
        let center = cp.as_vec3() + Vec3::splat(Chunk::LEN as f32 * 0.5);
        let view_dir = (center - cam_pos).normalize_or_zero();
        let in_view = view_dir.dot(cam_forward) >= half_fov_cos;

        (
            if in_view { 0 } else { 1 },
            bevy::math::FloatOrd(cp.distance_squared(cam_cp) as f32),
        )
    });

    for chunkpos in chunks_remesh {
        if chunk_sys.chunks_meshing.len() >= chunk_sys.max_concurrent_meshing {
            break;
        }
        if chunk_sys.chunks_meshing.contains(&chunkpos) {
            continue;
        }

        let mut has = false;
        if let Some(chunkptr) = chunk_sys.get_chunk(chunkpos) {
            has = true;

            let chunkptr = chunkptr.clone();
            let tx = tx_chunks_meshing.clone();
            let stage = if cfg.surface_only_meshing {
                ChunkMeshStage::SurfaceFast
            } else if cfg.surface_first_meshing && !fast_ready_chunks.contains(&chunkpos) {
                ChunkMeshStage::SurfaceFast
            } else {
                ChunkMeshStage::FullQuality
            };

            match stage {
                ChunkMeshStage::SurfaceFast => meshing_stats.submitted_surface_this_frame += 1,
                ChunkMeshStage::FullQuality => meshing_stats.submitted_full_this_frame += 1,
            }

            let task = AsyncComputeTaskPool::get().spawn(async move {
                let mut _vbuf = THREAD_LOCAL_VERTEX_BUFFERS
                    .get_or(|| RefCell::new((VertexBuffer::default(), VertexBuffer::default(), VertexBuffer::default())))
                    .borrow_mut();
                // 0: vbuf_terrain, 1: vbuf_foliage, 2: vbuf_liquid

                // let dbg_time = Instant::now();
                let entity;
                let mesh_handle_terrain;
                let mesh_handle_foliage;
                let mesh_handle_liquid;
                {
                    let chunk = chunkptr.as_ref();

                    // Generate Mesh
                    if stage == ChunkMeshStage::SurfaceFast {
                        meshgen::generate_chunk_mesh_surface_fast(&mut _vbuf.0, chunk);
                    } else {
                        meshgen::generate_chunk_mesh(&mut _vbuf.0, chunk);
                    }

                    meshgen::generate_chunk_mesh_foliage(&mut _vbuf.1, chunk);

                    meshgen::generate_chunk_mesh_liquid(&mut _vbuf.2, chunk);

                    entity = chunk.entity;
                    mesh_handle_terrain = chunk.mesh_handle_terrain.clone();
                    mesh_handle_foliage = chunk.mesh_handle_foliage.clone();
                    mesh_handle_liquid = chunk.mesh_handle_liquid.clone();
                }
                // let dbg_time = Instant::now() - dbg_time;

                // vbuf.compute_flat_normals();
                // _vbuf.0.compute_smooth_normals();

                // let nv = vbuf.vertices.len();
                // vbuf.compute_indexed();  // save 70%+ vertex data space!
                // todo: Cannot use Real IndexedBuffer, it caused WGSL @builtin(vertex_index) produce invalid Barycentric Coordinate, fails material interpolation.
                // vulkan also have this issue, but extension
                //   #extension GL_EXT_fragment_shader_barycentric : enable
                //   layout(location = 2) pervertexEXT in int in_MtlIds[];  gl_BaryCoordEXT
                // would fix the problem in vulkan.
                _vbuf.0.compute_indexed_naive();

                // if nv != 0 {
                //     info!("Generated ReMesh verts: {} before: {} after {}, saved: {}%",
                //     vbuf.vertex_count(), nv, vbuf.vertices.len(), (1.0 - vbuf.vertices.len() as f32/nv as f32) * 100.0);
                // }

                let mut mesh_terrain = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                _vbuf.0.to_mesh(&mut mesh_terrain);
                _vbuf.0.clear();

                // Build Collider of TriMesh
                let collider = Collider::trimesh_from_mesh(&mesh_terrain);

                // Foliage
                _vbuf.1.compute_indexed_naive();

                let mut mesh_foliage = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                _vbuf.1.to_mesh(&mut mesh_foliage);
                _vbuf.1.clear();

                // Liquid
                _vbuf.2.compute_indexed_naive();

                let mut mesh_liquid = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                _vbuf.2.to_mesh(&mut mesh_liquid);
                _vbuf.2.clear();

                if tx
                    .send((
                        chunkpos,
                        entity,
                        mesh_terrain,
                        mesh_handle_terrain,
                        collider,
                        mesh_foliage,
                        mesh_handle_foliage,
                        mesh_liquid,
                        mesh_handle_liquid,
                        stage,
                    ))
                    .is_err()
                {
                    warn!("Chunk meshing channel closed");
                }
            });
            task.detach();

            // info!("[ReMesh Enqueued] Pos: {}; ReMesh: {}, Meshing: {}: tx: {}, rx: {}", chunkpos, chunk_sys.chunks_remesh.len(), cli.chunks_meshing.len(), tx_chunks_meshing.len(), rx_chunks_meshing.len());
        }
        if has {
            chunk_sys.chunks_meshing.insert(chunkpos);
        }
        chunk_sys.chunks_remesh.remove(&chunkpos);
    }

    while let Ok((
        chunkpos,
        entity,
        mesh_terrain,
        mesh_handle_terrain,
        collider,
        mesh_foliage,
        mesh_handle_foliage,
        mesh_liquid,
        mesh_handle_liquid,
        stage,
    )) = rx_chunks_meshing.try_recv()
    {
        // Update Mesh Asset
        if let Some(dst) = meshes.get_mut(mesh_handle_terrain.id()) {
            *dst = mesh_terrain;
        }

        if let Some(dst) = meshes.get_mut(mesh_handle_foliage.id()) {
            *dst = mesh_foliage;
        }

        if let Some(dst) = meshes.get_mut(mesh_handle_liquid.id()) {
            *dst = mesh_liquid;
        }

        // Ensure the chunk entity is visible now that meshes were uploaded
        if let Ok(mut ent_cmds) = commands.get_entity(entity) {
            ent_cmds.try_insert(Visibility::Visible);
        }
        // Update Phys Collider TriMesh
        if let Some(collider) = collider {
            if let Ok(mut cmds) = commands.get_entity(entity) {
                // note: use try_insert cuz the entity may already been unloaded when executing the cmds (?)
                cmds.remove::<Collider>().try_insert(collider).try_insert(Visibility::Visible);
            }
        }

        chunk_sys.chunks_meshing.remove(&chunkpos);

        if cfg.surface_only_meshing {
            if stage == ChunkMeshStage::SurfaceFast {
                fast_ready_chunks.insert(chunkpos);
                meshing_stats.completed_surface_total += 1;
            }
        } else if cfg.surface_first_meshing {
            if stage == ChunkMeshStage::SurfaceFast {
                fast_ready_chunks.insert(chunkpos);
                chunk_sys.chunks_remesh.insert(chunkpos);
                meshing_stats.completed_surface_total += 1;
            } else {
                fast_ready_chunks.remove(&chunkpos);
                meshing_stats.completed_full_total += 1;
            }
        } else if stage == ChunkMeshStage::FullQuality {
            meshing_stats.completed_full_total += 1;
        }
        // info!("[ReMesh Completed] Pos: {}; ReMesh: {}, Meshing: {}: tx: {}, rx: {}", chunkpos, chunk_sys.chunks_remesh.len(), cli.chunks_meshing.len(), tx_chunks_meshing.len(), rx_chunks_meshing.len());
    }

    meshing_stats.fast_pending_upgrade = if cfg.surface_first_meshing && !cfg.surface_only_meshing {
        fast_ready_chunks.len()
    } else {
        0
    };
    meshing_stats.remesh_queue = chunk_sys.chunks_remesh.len();
    meshing_stats.meshing_inflight = chunk_sys.chunks_meshing.len();
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct VoxelBrush {
    pub shape: VoxShape,
    pub tex: u16,
    pub size: f32,
    pub strength: f32,
}
impl Default for VoxelBrush {
    fn default() -> Self {
        Self {
            size: 4.,
            strength: 0.8,
            shape: VoxShape::Isosurface,
            tex: 10,
        }
    }
}

#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct HitResult {
    pub is_hit: bool,
    pub position: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    // entity: Entity,
    pub is_voxel: bool,
    pub voxel_pos: IVec3,
}

fn raycast(
    spatial_query: SpatialQuery,
    query_cam: Query<&GlobalTransform, With<CharacterControllerCamera>>, // ray
    query_player: Query<Entity, With<CharacterController>>,              // exclude collider
    key: Res<ButtonInput<KeyCode>>,
    mut hit_result: ResMut<HitResult>,
    touches: Res<Touches>,
    touch_buttons: Res<TouchButtonState>,

    query_input: Query<&ActionState<InputAction>>,
    mut chunk_sys: ResMut<ClientChunkSystem>,
    mut net_client: Option<ResMut<RenetClient>>,
    cli: Res<ClientInfo>,
    cfg: Res<ClientSettings>,
    vox_brush: Res<VoxelBrush>,
) {
    let Ok(cam_trans) = query_cam.single() else {
        return;
    };
    let ray_pos = cam_trans.translation();
    let ray_dir = cam_trans.forward();

    let player_entity = match query_player.single() {
        Ok(entity) => entity,
        Err(_) => Entity::PLACEHOLDER,
    };

    if let Some(hit) = spatial_query.cast_ray(
        ray_pos,
        ray_dir,
        100.,
        true,
        &SpatialQueryFilter::default().with_excluded_entities(vec![player_entity]),
    ) {
        hit_result.is_hit = true;
        hit_result.normal = hit.normal;
        // hit_result.entity = hit.entity;
        let dist = hit.distance;
        hit_result.distance = dist;
        hit_result.position = ray_pos + ray_dir.as_vec3() * dist;

        // commands.entity(hit.entity)

        hit_result.voxel_pos = (hit_result.position + -0.01 * hit_result.normal).floor().as_ivec3();
    } else {
        hit_result.is_hit = false;
    }

    // ############ Break & Place ############

    let in_world_editor_ui = cli.curr_ui == CurrentUI::WorldEditor;

    if cli.curr_ui != CurrentUI::None && !in_world_editor_ui {
        // todo: cli.is_manipulating()
        return;
    }

    if !cli.is_admin {
        return;
    }

    let Ok(action_state) = query_input.single() else {
        return;
    };

    #[cfg(target_os = "android")]
    let touch_count_just_pressed = touches.iter_just_pressed().count();

    #[cfg(target_os = "android")]
    let do_break = if cfg.touch_ui {
        action_state.just_pressed(&InputAction::Attack) || touch_buttons.attack_just_pressed
    } else {
        action_state.just_pressed(&InputAction::Attack) || touch_count_just_pressed == 1
    };
    #[cfg(not(target_os = "android"))]
    let do_break = {
        let trigger = action_state.just_pressed(&InputAction::Attack);
        if in_world_editor_ui {
            trigger && (key.pressed(KeyCode::ControlLeft) || key.pressed(KeyCode::ControlRight))
        } else {
            trigger
        }
    };

    #[cfg(target_os = "android")]
    let do_place = if cfg.touch_ui {
        action_state.just_pressed(&InputAction::UseItem) || touch_buttons.use_just_pressed
    } else {
        action_state.just_pressed(&InputAction::UseItem) || touch_count_just_pressed >= 2
    };
    #[cfg(not(target_os = "android"))]
    let do_place = {
        let trigger = action_state.just_pressed(&InputAction::UseItem);
        if in_world_editor_ui {
            trigger && (key.pressed(KeyCode::ControlLeft) || key.pressed(KeyCode::ControlRight))
        } else {
            trigger
        }
    };

    if hit_result.is_hit && (do_break || do_place) {
        let brush = &*vox_brush;
        let n = brush.size as i32;
        let mut changed_cells: HashMap<IVec3, Vec<CellData>> = HashMap::new();

        // These code is Horrible

        iter::iter_aabb(n, n, |lp| {
            // +0.01*norm: for placing cube like MC.
            let p = hit_result.voxel_pos + lp + if do_place { 1 } else { 0 } * hit_result.normal.normalize_or_zero().as_ivec3();

            if let Some(v) = chunk_sys.get_voxel(p) {
                let v = v.as_mut();
                let f = (n as f32 - lp.as_vec3().length()).max(0.) * brush.strength;

                v.set_isovalue(v.isovalue() + if do_break { -f } else { f });

                if f > 0.0 || (n == 0 && f == 0.0) {
                    // placing single
                    if do_place {
                        // && c.tex_id == 0 {
                        v.tex_id = brush.tex;
                        v.shape_id = brush.shape;

                        // placing Block
                        if brush.shape != VoxShape::Isosurface {
                            v.set_isovalue(0.0);
                        }
                    } else if v.is_isoval_empty() {
                        v.tex_id = 0;
                    }
                }

                let chunkpos = Chunk::as_chunkpos(p);
                let local_idx = Chunk::local_idx(Chunk::as_localpos(p)) as u16;
                let cell_data = CellData::from_cell(local_idx, v);

                chunk_sys.mark_chunk_remesh(chunkpos); // CLIS

                changed_cells
                    .entry(chunkpos)
                    .or_insert_with(Vec::new)
                    .push(cell_data);
            }
        });

        if let Some(net_client) = net_client.as_mut() {
            for (chunkpos, voxel) in changed_cells {
                net_client.send_packet(&CPacket::ChunkModify { chunkpos, voxel });
            }
        }
    }
}

fn draw_crosshair_cube(mut gizmos: Gizmos, hit_result: Res<HitResult>, vbrush: Res<VoxelBrush>) {
    if hit_result.is_hit {
        if vbrush.shape == VoxShape::Isosurface {
            //gizmos.sphere(hit_result.position, Quat::IDENTITY, vbrush.size, Color::BLACK);
            gizmos.sphere(Isometry3d::IDENTITY, vbrush.size, Color::BLACK);
        } else {
            let trans = Transform::from_translation(hit_result.voxel_pos.as_vec3() + 0.5).with_scale(Vec3::ONE * vbrush.size.floor());

            gizmos.cuboid(trans, Color::BLACK);
        }
    }
}

fn draw_gizmos(mut gizmos: Gizmos, chunk_sys: Res<ClientChunkSystem>, cli: Res<ClientInfo>, query_cam: Query<&Transform, With<CharacterController>>) {
    if !cli.dbg_gizmo_all_loaded_chunks {
        return;
    }

    // // chunks loading
    // for cp in chunk_sys.chunks_loading.keys() {
    //     gizmos.cuboid(
    //         Transform::from_translation(cp.as_vec3()).with_scale(Vec3::splat(Chunk::LEN as f32)),
    //         Color::GREEN,
    //     );
    // }

    // all loaded chunks
    if cli.dbg_gizmo_all_loaded_chunks {
        for cp in chunk_sys.get_chunks().keys() {
            gizmos.cuboid(
                Transform::from_translation(cp.as_vec3() + 0.5 * Chunk::LEN as f32).with_scale(Vec3::splat(Chunk::LEN as f32)),
                Srgba::gray(0.25),
            );
        }
    }

    if cli.dbg_gizmo_curr_chunk {
        if let Ok(trans) = query_cam.single() {
            let cp = Chunk::as_chunkpos(trans.translation.as_ivec3());
            gizmos.cuboid(
                Transform::from_translation(cp.as_vec3() + 0.5 * Chunk::LEN as f32).with_scale(Vec3::splat(Chunk::LEN as f32)),
                Srgba::gray(0.7),
            );
        }
    }

    if cli.dbg_gizmo_remesh_chunks {
        // chunks remesh
        for cp in chunk_sys.chunks_remesh.iter() {
            gizmos.cuboid(
                Transform::from_translation(cp.as_vec3() + 0.5 * Chunk::LEN as f32).with_scale(Vec3::splat(Chunk::LEN as f32)),
                css::ORANGE,
            );
        }

        // chunks meshing
        for cp in chunk_sys.chunks_meshing.iter() {
            gizmos.cuboid(
                Transform::from_translation(cp.as_vec3() + 0.5 * Chunk::LEN as f32).with_scale(Vec3::splat(Chunk::LEN as f32)),
                css::RED,
            );
        }
    }
}

///////////////////////////////////////////////////
//////////////// ClientChunkSystem ////////////////
///////////////////////////////////////////////////

#[derive(Resource)]
pub struct ClientChunkSystem {
    pub chunks: HashMap<IVec3, ChunkPtr>,

    // mark to ReMesh
    pub chunks_remesh: HashSet<IVec3>,

    pub mtl_terrain: Handle<ExtendedMaterial<StandardMaterial, TerrainMaterial>>,
    pub mtl_foliage: Handle<ExtendedMaterial<StandardMaterial, FoliageMaterial>>,
    pub mtl_liquid: Handle<ExtendedMaterial<StandardMaterial, LiquidMaterial>>,
    // pub mtl_std: Handle<StandardMaterial>,
    pub entity: Entity,

    pub max_concurrent_meshing: usize,
    pub chunks_meshing: HashSet<IVec3>,
    pub world_config: WorldGenConfig,
    // pub chunks_load_distance: IVec2, // not real, but send to server,
}

impl ChunkSystem for ClientChunkSystem {
    fn get_chunks(&self) -> &HashMap<IVec3, ChunkPtr> {
        &self.chunks
    }
}

impl Default for ClientChunkSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientChunkSystem {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::default(),
            chunks_remesh: HashSet::default(),

            mtl_terrain: Handle::default(),
            mtl_foliage: Handle::default(),
            mtl_liquid: Handle::default(),
            // mtl_std: Handle::default(),
            entity: Entity::PLACEHOLDER,

            max_concurrent_meshing: 8,
            chunks_meshing: HashSet::default(),
            world_config: WorldGenConfig::default(),
        }
    }

    pub fn mark_chunk_remesh(&mut self, chunkpos: IVec3) {
        self.chunks_remesh.insert(chunkpos);
    }

    pub fn spawn_chunk(&mut self, mut chunk: Chunk, cmds: &mut Commands, meshes: &mut Assets<Mesh>) {
        let chunkpos = chunk.chunkpos;

        let aabb = bevy::camera::primitives::Aabb::from_min_max(Vec3::ZERO, Vec3::ONE * (Chunk::LEN as f32));

        chunk.mesh_handle_terrain = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD));
        chunk.mesh_handle_foliage = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD));
        chunk.mesh_handle_liquid  = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD));

        chunk.entity = cmds
            .spawn((
                // ChunkComponent::new(*chunkpos),
                (
                    Mesh3d(chunk.mesh_handle_terrain.clone()),
                    MeshMaterial3d(self.mtl_terrain.clone()), //materials.add(Color::rgb(0.8, 0.7, 0.6)),
                    Transform::from_translation(chunkpos.as_vec3()),
                    Visibility::Hidden, // Hidden is required since Mesh is empty. or WGPU will crash. even if use default Inherite
                    VoxelChunkRenderMesh,
                ),
                aabb,
                avian3d::prelude::RigidBody::Static,
            ))
            .with_children(|parent| {
                parent.spawn(((
                        Mesh3d(chunk.mesh_handle_foliage.clone()),
                        MeshMaterial3d(self.mtl_foliage.clone()),
                        Visibility::Visible, // Hidden is required since Mesh is empty. or WGPU will crash
                        VoxelChunkRenderMesh,
                    ),
                    aabb,
                ));
                parent.spawn(((
                        Mesh3d(chunk.mesh_handle_liquid.clone()),
                        MeshMaterial3d(self.mtl_liquid.clone()),
                        Visibility::Visible,
                        VoxelChunkRenderMesh,
                    ),
                    aabb,
                ));
            })
            .set_parent_in_place(self.entity)
            .id();

        let chunkptr = Arc::new(chunk);

        let chunkpos;
        {
            let chunk = chunkptr.as_mut();
            chunkpos = chunk.chunkpos;
            chunk.chunkptr_weak = Arc::downgrade(&chunkptr);

            // let mut neighbors_completed = Vec::new();

            for neib_idx in 0..Chunk::NEIGHBOR_DIR.len() {
                let neib_dir = Chunk::NEIGHBOR_DIR[neib_idx];
                let neib_chunkpos = chunkpos + neib_dir * Chunk::LEN;

                // set neighbor_chunks cache
                chunk.neighbor_chunks[neib_idx] = {
                    if let Some(neib_chunkptr) = self.get_chunk(neib_chunkpos).cloned() {
                        let neib_chunk = neib_chunkptr.as_mut();

                        // update neighbor's `neighbor_chunk`
                        neib_chunk.neighbor_chunks[Chunk::neighbor_idx_opposite(neib_idx)] = Some(Arc::downgrade(&chunkptr));

                        if neib_chunk.is_neighbors_all_loaded() && !neib_chunk.is_populated {
                            // neighbors_completed.push(neib_chunk.chunkpos);
                            neib_chunk.is_populated = true;
                            super::worldgen::populate_chunk(neib_chunk, &self.world_config); // todo: ChunkGen Thread

                            self.mark_chunk_remesh(neib_chunk.chunkpos);

                            // fixed: chunk border mesh outdated issue due to population update.
                            for (idx, nneib) in neib_chunk.neighbor_chunks.iter().enumerate() {
                                if nneib.is_some() {
                                    self.mark_chunk_remesh(neib_chunk.chunkpos + Chunk::NEIGHBOR_DIR[idx] * Chunk::LEN);
                                }
                            }
                        }
                        
                        Some(Arc::downgrade(&neib_chunkptr))
                    } else {
                        None
                    }
                }
            }

            // if chunk.is_neighbors_complete() {
            self.mark_chunk_remesh(chunkpos);
            // }
            // for cp in neighbors_completed {
            //     self.mark_chunk_remesh(cp);
            // }
        }

        self.chunks.insert(chunkpos, chunkptr);

        // // There is no need to cast shadows for chunks below the surface.
        // if chunkpos.y <= 64 {
        //     entity_commands.insert(NotShadowCaster);
        // }
    }

    pub fn despawn_chunk(&mut self, chunkpos: IVec3, cmds: &mut Commands) -> Option<ChunkPtr> {
        let chunk = self.chunks.remove(&chunkpos)?;

        // update neighbors' `neighbors_chunk`
        for neib_idx in 0..Chunk::NEIGHBOR_DIR.len() {
            if let Some(neib_chunkptr) = chunk.get_chunk_neib(neib_idx) {
                let neib_chunk = neib_chunkptr.as_mut(); // problematic: may cause data tiring

                neib_chunk.neighbor_chunks[Chunk::neighbor_idx_opposite(neib_idx)] = None;
            }
        }

        cmds.entity(chunk.entity).despawn();

        Some(chunk)
    }
}

fn asset_load_ui_system(
    asset_server: Res<AssetServer>,
    assets: Option<Res<AssetDebug>>,
    mut texts: Query<&mut Text>,
) {
    // no-op placeholder when UI is not available
    let _ = (asset_server, assets, texts);

}


fn write_debug_file_system(
    asset_server: Res<AssetServer>,
    asset_debug: Option<Res<AssetDebug>>,
    chunk_sys: Option<Res<ClientChunkSystem>>,
    meshes: Option<Res<Assets<Mesh>>>,
    mtls_terrain: Option<Res<Assets<ExtendedMaterial<StandardMaterial, TerrainMaterial>>>>,
    images: Option<Res<Assets<Image>>>,
    time: Res<Time>,
    mut last: Local<f32>,
) {
    let assets = match asset_debug {
        Some(a) => a,
        None => return,
    };

    // write at most once every 2 seconds
    *last += time.delta_secs();
    if *last < 2.0 {
        return;
    }
    *last = 0.0;

    let mut s = String::new();
    s.push_str(&format!("Timestamp: {:?}\n", std::time::SystemTime::now()));

    let items = vec![
        ("albedo", assets.albedo.clone()),
        ("normal", assets.normal.clone()),
        ("dram", assets.dram.clone()),
        ("foliage", assets.foliage_diff.clone()),
        ("water_norm", assets.water_normals.clone()),
    ];
    s.push_str("Asset States:\n");
    for (name, handle) in items.iter() {
        let handle_id = handle.id();
        let st = asset_server.get_load_state(handle_id);
        s.push_str(&format!("  {}: {:?}\n", name, st));
        if matches!(st, Some(bevy::asset::LoadState::Loaded)) {
            if let Some(imgs) = &images {
                if let Some(img) = imgs.get(handle_id) {
                    s.push_str(&format!("    size: {:?}, texture_format: {:?}\n", img.texture_descriptor.size, img.texture_descriptor.format));
                }
            }
        }
    }

    if let Some(chunks) = chunk_sys {
        s.push_str(&format!("Chunks Loaded: {}\n", chunks.chunks.len()));
    }
    if let Some(ms) = meshes {
        s.push_str(&format!("Meshes: {}\n", ms.len()));
    }
    if let Some(mt) = mtls_terrain {
        s.push_str(&format!("Terrain Materials: {}\n", mt.len()));
    }

    let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("eth_debug.txt");
    if let Err(e) = std::fs::write(&path, s.as_bytes()) {
        error!("Failed to write debug file: {}", e);
    } else {
        info!("Wrote debug file: {:?}", path);
    }
}
