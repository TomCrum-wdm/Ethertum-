use std::sync::mpsc;
use once_cell::sync::OnceCell;
use std::sync::Mutex;

use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};
use futures_lite::future;

use super::{Chunk, Vox, VoxShape, VoxTex, WorldGenConfig, WorldTerrainMode};

const SHADER_SOURCE: &str = include_str!("worldgen_compute.wgsl");

struct GpuWorldgenContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    buffers: Mutex<GpuWorldgenBuffers>,
}

#[derive(Default)]
struct GpuWorldgenBuffers {
    chunk_capacity: usize,
    voxel_capacity: u32,
    chunk_pos: Option<wgpu::Buffer>,
    params: Option<wgpu::Buffer>,
    voxel_out: Option<wgpu::Buffer>,
    readback: Option<wgpu::Buffer>,
}

static GPU_WORLDGEN_CONTEXT: OnceCell<GpuWorldgenContext> = OnceCell::new();

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParams {
    terrain_mode: u32,
    octaves: u32,
    len: u32,
    total_voxels: u32,

    seed_lo: u32,
    seed_hi: u32,
    superflat_ground_level: i32,
    superflat_dirt_depth: i32,

    noise_scale_2d: f32,
    noise_scale_3d: f32,
    flat_height_divisor: f32,
    flat_3d_noise_strength: f32,

    flat_water_level: f32,
    planet_radius: f32,
    planet_shell_thickness: f32,
    planet_3d_noise_strength: f32,

    planet_inner_water: u32,
    superflat_water_level: i32,
    _pad0: [u32; 2],

    planet_center: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ChunkPosGpu {
    x: i32,
    y: i32,
    z: i32,
    _pad: i32,
}

pub fn generate_chunks_gpu_batched(chunk_positions: &[IVec3], config: &WorldGenConfig, seed: u64) -> anyhow::Result<Vec<Chunk>> {
    if chunk_positions.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(target_arch = "wasm32")]
    {
        anyhow::bail!("wgsl compute worldgen is not enabled for wasm in this path")
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        generate_chunks_gpu_native(chunk_positions, config, seed)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn gpu_context() -> anyhow::Result<&'static GpuWorldgenContext> {
    GPU_WORLDGEN_CONTEXT.get_or_try_init(|| {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = future::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| anyhow::anyhow!("wgpu adapter request failed: {e}"))?;

        let (device, queue) = future::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("worldgen_gpu_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| anyhow::anyhow!("wgpu device request failed: {e}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("worldgen_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("worldgen_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("worldgen_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("worldgen_compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        Ok(GpuWorldgenContext {
            device,
            queue,
            bind_group_layout,
            pipeline,
            buffers: Mutex::new(GpuWorldgenBuffers::default()),
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn generate_chunks_gpu_native(chunk_positions: &[IVec3], config: &WorldGenConfig, seed: u64) -> anyhow::Result<Vec<Chunk>> {
    let mut cfg = config.clone();
    cfg.sanitize();
    let terrain_shape = cfg.terrain_solid_shape();
    let ctx = gpu_context()?;
    let device = &ctx.device;
    let queue = &ctx.queue;

    let chunk_positions_gpu: Vec<ChunkPosGpu> = chunk_positions
        .iter()
        .map(|p| ChunkPosGpu {
            x: p.x,
            y: p.y,
            z: p.z,
            _pad: 0,
        })
        .collect();

    let total_voxels = (chunk_positions.len() * Chunk::LEN3) as u32;
    let params = GpuParams {
        terrain_mode: match cfg.terrain_mode {
            WorldTerrainMode::Planet => 0,
            WorldTerrainMode::Flat => 1,
            WorldTerrainMode::SuperFlat => 2,
        },
        octaves: cfg.fbm_octaves as u32,
        len: Chunk::LEN as u32,
        total_voxels,

        seed_lo: seed as u32,
        seed_hi: (seed >> 32) as u32,
        superflat_ground_level: cfg.superflat_ground_level,
        superflat_dirt_depth: cfg.superflat_dirt_depth,

        noise_scale_2d: cfg.noise_scale_2d.max(1.0),
        noise_scale_3d: cfg.noise_scale_3d.max(1.0),
        flat_height_divisor: cfg.flat_height_divisor.max(1.0),
        flat_3d_noise_strength: cfg.flat_3d_noise_strength,

        flat_water_level: cfg.flat_water_level as f32,
        planet_radius: cfg.planet_radius,
        planet_shell_thickness: cfg.planet_shell_thickness.max(1.0),
        planet_3d_noise_strength: cfg.planet_3d_noise_strength,

        planet_inner_water: u32::from(cfg.planet_inner_water),
        superflat_water_level: cfg.superflat_water_level,
        _pad0: [0; 2],

        planet_center: [
            cfg.planet_center.x as f32,
            cfg.planet_center.y as f32,
            cfg.planet_center.z as f32,
            0.0,
        ],
    };

    let voxel_out_size = (total_voxels as u64) * 8;

    let (chunk_pos_buffer, params_buffer, voxel_out_buffer, readback_buffer) = {
        let mut bufs = ctx
            .buffers
            .lock()
            .map_err(|e| anyhow::anyhow!("worldgen gpu buffer lock poisoned: {e}"))?;

        if bufs.chunk_capacity < chunk_positions_gpu.len() || bufs.chunk_pos.is_none() {
            let cap = chunk_positions_gpu.len().max(bufs.chunk_capacity.max(16));
            bufs.chunk_capacity = cap;
            bufs.chunk_pos = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("worldgen_chunk_positions"),
                size: (cap as u64) * std::mem::size_of::<ChunkPosGpu>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        if bufs.params.is_none() {
            bufs.params = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("worldgen_params"),
                size: std::mem::size_of::<GpuParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        if bufs.voxel_capacity < total_voxels || bufs.voxel_out.is_none() || bufs.readback.is_none() {
            let cap = total_voxels.max(bufs.voxel_capacity.max((Chunk::LEN3 as u32) * 16));
            bufs.voxel_capacity = cap;
            let cap_size = (cap as u64) * 8;
            bufs.voxel_out = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("worldgen_voxel_out"),
                size: cap_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            bufs.readback = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("worldgen_readback"),
                size: cap_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }

        (
            bufs.chunk_pos.as_ref().expect("chunk pos buffer").clone(),
            bufs.params.as_ref().expect("params buffer").clone(),
            bufs.voxel_out.as_ref().expect("voxel out buffer").clone(),
            bufs.readback.as_ref().expect("readback buffer").clone(),
        )
    };

    queue.write_buffer(&chunk_pos_buffer, 0, bytemuck::cast_slice(&chunk_positions_gpu));
    queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("worldgen_bind_group"),
        layout: &ctx.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: chunk_pos_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: voxel_out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("worldgen_encoder"),
    });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("worldgen_compute_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&ctx.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups = total_voxels.div_ceil(64);
        pass.dispatch_workgroups(workgroups, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&voxel_out_buffer, 0, &readback_buffer, 0, voxel_out_size);
    queue.submit(Some(encoder.finish()));

    let read_slice = readback_buffer.slice(..);
    let (tx, rx) = mpsc::sync_channel(1);
    read_slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });

    let _ = device.poll(wgpu::PollType::wait());
    rx.recv().map_err(|e| anyhow::anyhow!("gpu readback recv failed: {e}"))??;

    let data = read_slice.get_mapped_range();
    let packed: &[u32] = bytemuck::cast_slice(&data);

    let mut out = Vec::with_capacity(chunk_positions.len());
    let voxels_per_chunk = Chunk::LEN3;
    for (chunk_i, &chunkpos) in chunk_positions.iter().enumerate() {
        let mut chunk = Chunk::new(chunkpos);
        let base = chunk_i * voxels_per_chunk * 2;

        for local_idx in 0..voxels_per_chunk {
            let val = f32::from_bits(packed[base + local_idx * 2]);
            let tex = packed[base + local_idx * 2 + 1] as u16;
            let lp = Chunk::local_idx_pos(local_idx as i32);
            let shape = if tex == VoxTex::Nil { VoxShape::Isosurface } else { terrain_shape };
            *chunk.at_voxel_mut(lp) = Vox::new(tex, shape, val);
        }

        out.push(chunk);
    }

    drop(data);
    readback_buffer.unmap();

    Ok(out)
}
