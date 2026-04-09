use std::ops::Div;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::{math::{ivec3, DVec2, DVec3}, prelude::*};
use noise::{Fbm, NoiseFn, Perlin};


use super::*;
use crate::util::{hash, iter};
// use crate::client::settings::ClientSettings;

static GPU_WORLDGEN_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);

pub fn gpu_worldgen_fallback_total() -> u64 {
    GPU_WORLDGEN_FALLBACK_TOTAL.load(Ordering::Relaxed)
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn fold_seed_u32(seed: u64) -> u32 {
    let mixed = splitmix64(seed ^ 0xA5A5_5A5A_DEAD_BEEFu64);
    ((mixed >> 32) as u32) ^ (mixed as u32)
}

fn seed_offsets(seed: u64) -> (DVec2, DVec3) {
    let s0 = splitmix64(seed ^ 0x4D59_5DF4_D0F3_3173);
    let s1 = splitmix64(seed ^ 0xD2B7_4407_B1CE_6E93);
    let s2 = splitmix64(seed ^ 0xCA5A_8263_9512_1157);
    let s3 = splitmix64(seed ^ 0x8FCA_2B6C_83B7_11AB);
    let to_unit = |v: u64| ((v as f64) / (u64::MAX as f64)) * 2.0 - 1.0;
    let ofs2 = DVec2::new(to_unit(s0), to_unit(s1)) * 4096.0;
    let ofs3 = DVec3::new(to_unit(s1), to_unit(s2), to_unit(s3)) * 4096.0;
    (ofs2, ofs3)
}

pub fn generate_chunk(chunk: &mut Chunk, config: &WorldGenConfig, seed: u64) {
    generate_chunk_with_seed(chunk, config, seed);
}

pub fn generate_chunks_gpu_batched(chunk_positions: &[IVec3], config: &WorldGenConfig, seed: u64) -> Vec<Chunk> {
    match super::worldgen_gpu::generate_chunks_gpu_batched(chunk_positions, config, seed) {
        Ok(chunks) => {
            let mut non_nil_voxels = 0usize;
            let total_voxels = chunks.len() * Chunk::LEN3;
            for chunk in &chunks {
                chunk.for_voxels(|v, _| {
                    if !v.is_nil() {
                        non_nil_voxels += 1;
                    }
                });
            }

            // Compatibility guard: only fallback when output is fully empty.
            if non_nil_voxels == 0 {
                warn!(
                    "GPU worldgen produced empty output ({non_nil_voxels}/{total_voxels}), falling back to CPU fast kernel"
                );
                let mut out = Vec::with_capacity(chunk_positions.len());
                for &chunkpos in chunk_positions {
                    let mut chunk = Chunk::new(chunkpos);
                    generate_chunk_fast_kernel(&mut chunk, config, seed);
                    out.push(chunk);
                }
                out
            } else {
                chunks
            }
        }
        Err(err) => {
            warn!("GPU worldgen failed, falling back to CPU fast kernel: {err}");
            GPU_WORLDGEN_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
            let mut out = Vec::with_capacity(chunk_positions.len());
            for &chunkpos in chunk_positions {
                let mut chunk = Chunk::new(chunkpos);
                generate_chunk_fast_kernel(&mut chunk, config, seed);
                out.push(chunk);
            }
            out
        }
    }
}

pub fn generate_chunk_fast_kernel(chunk: &mut Chunk, config: &WorldGenConfig, seed: u64) {
    let mut config = config.clone();
    config.sanitize();

    let mut fbm = Fbm::<Perlin>::new(fold_seed_u32(seed));
    fbm.octaves = config.fbm_octaves as usize;
    let (seed_ofs2, seed_ofs3) = seed_offsets(seed);

    let terrain_mode = config.terrain_mode;
    let planet_center = config.planet_center;
    let planet_radius = config.planet_radius;
    let shell_thickness = config.planet_shell_thickness.max(1.0);
    let noise_scale_2d = config.noise_scale_2d.max(1.0);
    let noise_scale_3d = config.noise_scale_3d.max(1.0);

    let mut terr_2d = [[0.0_f32; Chunk::LEN as usize]; Chunk::LEN as usize];
    for lz in 0..Chunk::LEN {
        for lx in 0..Chunk::LEN {
            let p = chunk.chunkpos + IVec3::new(lx, 0, lz);
            let f = match terrain_mode {
                WorldTerrainMode::Planet => {
                    let safe = p.as_vec3().clamp(Vec3::splat(-100_000.0), Vec3::splat(100_000.0));
                    let mut sample = (safe / noise_scale_2d).xz().as_dvec2();
                    sample += seed_ofs2;
                    fbm.get(sample.to_array()) as f32
                }
                WorldTerrainMode::Flat => {
                    let mut sample = p.xz().as_dvec2().div(noise_scale_2d as f64);
                    sample += seed_ofs2;
                    fbm.get(sample.to_array()) as f32
                }
                WorldTerrainMode::SuperFlat => 0.0,
            };
            terr_2d[lz as usize][lx as usize] = f;
        }
    }

    for ly in 0..Chunk::LEN {
        for lz in 0..Chunk::LEN {
            for lx in 0..Chunk::LEN {
                let lp = IVec3::new(lx, ly, lz);
                let p = chunk.chunkpos + lp;

                let f_terr = terr_2d[lz as usize][lx as usize];
                let f_3d = match terrain_mode {
                    WorldTerrainMode::Planet => {
                        let safe = p.as_vec3().clamp(Vec3::splat(-100_000.0), Vec3::splat(100_000.0));
                        fbm.get((safe / noise_scale_3d).to_array().map(|v| v as f64)) as f32
                    }
                    WorldTerrainMode::Flat => {
                        let mut sample = p.as_dvec3().div(noise_scale_3d as f64);
                        sample += seed_ofs3;
                        fbm.get(sample.to_array()) as f32
                    }
                    WorldTerrainMode::SuperFlat => 0.0,
                };

                let (val, tex) = match terrain_mode {
                    WorldTerrainMode::Planet => {
                        let d = (p.as_vec3() - planet_center.as_vec3()).length();
                        let mut val =
                            f_terr - ((d - planet_radius) / shell_thickness) + f_3d * config.planet_3d_noise_strength;
                        if !val.is_finite() {
                            val = -1.0;
                        }
                        let mut tex = VoxTex::Nil;
                        if val > 0.0 {
                            tex = VoxTex::Stone;
                        } else if config.planet_inner_water && d < planet_radius && val < 0.0 {
                            val = -0.1;
                            tex = VoxTex::Water;
                        }
                        (val, tex)
                    }
                    WorldTerrainMode::Flat => {
                        let mut val =
                            f_terr - (p.y as f32) / config.flat_height_divisor + f_3d * config.flat_3d_noise_strength;
                        let mut tex = VoxTex::Nil;
                        if val > 0.0 {
                            tex = VoxTex::Stone;
                        } else if p.y < config.flat_water_level && val < 0.0 {
                            val = -0.1;
                            tex = VoxTex::Water;
                        }
                        (val, tex)
                    }
                    WorldTerrainMode::SuperFlat => {
                        let stone_top = config.superflat_ground_level - config.superflat_dirt_depth;
                        if p.y < stone_top {
                            (1.0, VoxTex::Stone)
                        } else if p.y < config.superflat_ground_level {
                            (1.0, VoxTex::Dirt)
                        } else if p.y == config.superflat_ground_level {
                            (1.0, VoxTex::Grass)
                        } else if p.y <= config.superflat_water_level {
                            (-0.1, VoxTex::Water)
                        } else {
                            (-1.0, VoxTex::Nil)
                        }
                    }
                };

                *chunk.at_voxel_mut(lp) = Vox::new(tex, VoxShape::Isosurface, val);
            }
        }
    }
}

pub fn generate_chunk_with_seed(chunk: &mut Chunk, config: &WorldGenConfig, seed: u64) {
    let mut config = config.clone();
    config.sanitize();

    // let perlin = Perlin::new(seed);
    let mut fbm = Fbm::<Perlin>::new(fold_seed_u32(seed));
    let (seed_ofs2, seed_ofs3) = seed_offsets(seed);
    // fbm.frequency = 0.2;
    // fbm.lacunarity = 0.2;
    fbm.octaves = config.fbm_octaves as usize;
    // fbm.persistence = 2;

    let terrain_mode = config.terrain_mode;

    let planet_center = config.planet_center;
    let planet_radius = config.planet_radius;
    let shell_thickness = config.planet_shell_thickness.max(1.0);
    let noise_scale_2d = config.noise_scale_2d.max(1.0);
    let noise_scale_3d = config.noise_scale_3d.max(1.0);

    for ly in 0..Chunk::LEN {
        for lz in 0..Chunk::LEN {
            for lx in 0..Chunk::LEN {
                let lp = IVec3::new(lx, ly, lz);
                let p = chunk.chunkpos + lp;

                let (val, mut tex) = match terrain_mode {
                    WorldTerrainMode::Planet => {
                        let d = (p.as_vec3() - planet_center.as_vec3()).length();
                        // clamp采样参数，防止极大坐标导致NaN/inf
                        let safe_vec3 = p.as_vec3().clamp(
                            Vec3::splat(-100_000.0),
                            Vec3::splat(100_000.0),
                        );
                        let arr2 = ((safe_vec3 / noise_scale_2d).xz().as_dvec2() + seed_ofs2).to_array();
                        let arr3 = ((safe_vec3 / noise_scale_3d).as_dvec3() + seed_ofs3).to_array();
                        let f_terr = fbm.get(arr2) as f32;
                        let f_3d = fbm.get(arr3) as f32;
                        let mut val = f_terr - ((d - planet_radius) / shell_thickness) + f_3d * config.planet_3d_noise_strength;
                        // 检查val非法
                        if !val.is_finite() { val = -1.0; }
                        let mut tex = VoxTex::Nil;
                        if val > 0.0 {
                            tex = VoxTex::Stone;
                        } else if config.planet_inner_water && d < planet_radius && val < 0.0 {
                            val = -0.1;
                            tex = VoxTex::Water;
                        }
                        (val, tex)
                    }
                    WorldTerrainMode::Flat => {
                        let f_terr = fbm.get((p.xz().as_dvec2().div(noise_scale_2d as f64) + seed_ofs2).to_array()) as f32;
                        let f_3d = fbm.get((p.as_dvec3().div(noise_scale_3d as f64) + seed_ofs3).to_array()) as f32;
                        let mut val = f_terr - (p.y as f32) / config.flat_height_divisor + f_3d * config.flat_3d_noise_strength;
                        let mut tex = VoxTex::Nil;
                        if val > 0.0 {
                            tex = VoxTex::Stone;
                        } else if p.y < config.flat_water_level && val < 0. {
                            val = -0.1;
                            tex = VoxTex::Water;
                        }
                        (val, tex)
                    }
                    WorldTerrainMode::SuperFlat => {
                        let stone_top = config.superflat_ground_level - config.superflat_dirt_depth;
                        if p.y < stone_top {
                            (1.0, VoxTex::Stone)
                        } else if p.y < config.superflat_ground_level {
                            (1.0, VoxTex::Dirt)
                        } else if p.y == config.superflat_ground_level {
                            (1.0, VoxTex::Grass)
                        } else if p.y <= config.superflat_water_level {
                            (-0.1, VoxTex::Water)
                        } else {
                            (-1.0, VoxTex::Nil)
                        }
                    }
                };
                *chunk.at_voxel_mut(lp) = Vox::new(tex, VoxShape::Isosurface, val);
            }
        }
    }
}

pub fn populate_chunk(chunk: &mut Chunk, config: &WorldGenConfig) {
    let chunkpos = chunk.chunkpos;
    let perlin = Perlin::new(123);

    for lx in 0..Chunk::LEN {
        for lz in 0..Chunk::LEN {
            // distance to air in top direction.
            let mut air_dist = 0;

            // check top air_dist. for CubicChunk system, otherwise the chunk-top will be surface/grass
            for i in 0..3 {
                if !chunk.get_voxel_rel_or_default(ivec3(lx, Chunk::LEN + i, lz)).is_nil() {
                    air_dist += 1;
                }
            }

            for ly in (0..Chunk::LEN).rev() {
                let lp = IVec3::new(lx, ly, lz);
                let c = chunk.at_voxel_mut(lp);

                if c.is_nil() {
                    air_dist = 0;
                } else {
                    air_dist += 1;
                }

                let p = chunk.chunkpos + lp;
                if c.tex_id == VoxTex::Stone {
                    let mut replace = c.tex_id;
                    if p.y < 2 && air_dist <= 2 && perlin.get([p.x as f64 / 32., p.z as f64 / 32.]) > 0.1 {
                        replace = VoxTex::Sand;
                    } else if air_dist <= 1 {
                        replace = VoxTex::Grass;
                    } else if air_dist < 3 {
                        replace = VoxTex::Dirt;
                    }
                    c.tex_id = replace;
                }
            }
        }
    }

    for lx in 0..Chunk::LEN {
        for lz in 0..Chunk::LEN {
            let x = chunkpos.x + lx;
            let z = chunkpos.z + lz;

            // TallGrass
            // hash(x * z * 100) < 0.23
            let g = perlin.get([x as f64 / 18., z as f64 / 18.]);
            if g > 0.0 {
                for ly in 0..Chunk::LEN - 1 {
                    let lp = ivec3(lx, ly, lz);

                    if chunk.at_voxel(lp).tex_id == VoxTex::Grass && chunk.at_voxel(lp + IVec3::Y).is_nil() {
                        let c = chunk.at_voxel_mut(lp + IVec3::Y);
                        c.tex_id = if g > 0.94 {
                            VoxTex::Rose
                        } else if g > 0.8 {
                            VoxTex::Fern
                        } else if g > 0.24 {
                            VoxTex::Bush
                        } else {
                            VoxTex::ShortGrass
                        };
                        c.shape_id = VoxShape::Grass;
                        break;
                    }
                }
            }

            // Vines
            if hash(x ^ (z * 7384)) < (18.0 / 256.0) {
                for ly in 0..Chunk::LEN - 1 {
                    let lp = ivec3(lx, ly, lz);

                    if chunk.at_voxel(lp).is_nil() && chunk.at_voxel(lp + IVec3::Y).tex_id == VoxTex::Stone {
                        for i in 0..(12.0 * hash(x ^ (z * 121))) as i32 {
                            let lp = lp + IVec3::NEG_Y * i;
                            if lp.y < 0 {
                                break;
                            }
                            let c = chunk.at_voxel_mut(lp);
                            if !c.is_nil() {
                                break;
                            }
                            c.tex_id = VoxTex::Leaves;
                            c.shape_id = VoxShape::Leaves;
                        }
                        break;
                    }
                }
            }

            let allow_trees = match config.terrain_mode {
                WorldTerrainMode::SuperFlat => config.superflat_generate_trees,
                _ => true,
            };

            // Trees
            if allow_trees && hash(x ^ (z * 9572)) < (3.0 / 256.0) {
                for ly in 0..Chunk::LEN {
                    let lp = ivec3(lx, ly, lz);

                    if chunk.at_voxel(lp).tex_id != VoxTex::Grass {
                        continue;
                    }
                    let siz = hash(x ^ ly ^ z);
                    gen_tree(chunk, lp, siz);
                }
            }
        }
    }
}

pub fn gen_tree(chunk: &mut Chunk, lp: IVec3, siz: f32) {
    let trunk_height = 3 + (siz * 6.0) as i32;
    let leaves_rad = 2 + (siz * 5.0) as i32;

    // Leaves
    iter::iter_aabb(leaves_rad, leaves_rad, |rp| {
        if rp.length_squared() >= leaves_rad * leaves_rad {
            return;
        }
        let lp = lp + IVec3::Y * trunk_height + rp;

        // if let Some(chunkptr) = chunk.get_chunk_rel(lp) {
        //     let vox = chunkptr.at_voxel_mut(Chunk::as_localpos(lp));
        //     vox .tex_id =VoxTex::Leaves;
        //     vox.shape_id = VoxShape::Leaves;
        // }

        chunk.set_voxel_rel(lp, |vox| {
            vox.tex_id =VoxTex::Leaves;
            vox.shape_id = VoxShape::Leaves;
        });
    });

    // Trunk
    for i in 0..trunk_height {
        if i + lp.y > 15 {
            break;
        }
        let c = chunk.at_voxel_mut(lp + IVec3::Y * i);
        c.tex_id = VoxTex::Log;
        c.shape_id = VoxShape::Isosurface;
        c.set_isovalue(2.0 * (1.2 - i as f32 / trunk_height as f32));
    }
}
