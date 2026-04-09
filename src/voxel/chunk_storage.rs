
use bevy::{
    math::{DVec2, DVec3, IVec3, Vec3, Vec3Swizzles},
    platform::collections::HashMap,
    reflect::Reflect,
    prelude::Resource,
};
use noise::{Fbm, NoiseFn, Perlin};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    path::Path,
};

#[cfg(target_arch = "wasm32")]
use once_cell::sync::Lazy;
#[cfg(target_arch = "wasm32")]
use std::sync::{Mutex, MutexGuard};

use super::{Chunk, ChunkPtr, Vox, VoxShape, VoxTex};

const META_FILE: &str = "meta.json";
const CHUNK_DIR: &str = "chunks";

pub const WORLD_META_SCHEMA_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum WorldTerrainMode {
    #[default]
    Planet,
    Flat,
    SuperFlat,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum WorldGenBackendPreference {
    #[default]
    Auto,
    CpuCompatible,
    GpuFast,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Reflect)]
#[serde(default)]
pub struct WorldGenConfig {
    pub terrain_mode: WorldTerrainMode,
    pub worldgen_backend: WorldGenBackendPreference,
    pub fbm_octaves: u8,
    pub noise_scale_2d: f32,
    pub noise_scale_3d: f32,

    pub flat_height_divisor: f32,
    pub flat_3d_noise_strength: f32,
    pub flat_water_level: i32,

    pub superflat_ground_level: i32,
    pub superflat_dirt_depth: i32,
    pub superflat_water_level: i32,
    pub superflat_generate_trees: bool,

    pub planet_center: IVec3,
    pub planet_radius: f32,
    pub planet_shell_thickness: f32,
    pub planet_3d_noise_strength: f32,
    pub planet_inner_water: bool,

    pub gravity_acceleration: f32,
    pub spawn_surface_offset: f32,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            terrain_mode: WorldTerrainMode::Planet,
            worldgen_backend: WorldGenBackendPreference::Auto,
            fbm_octaves: 5,
            noise_scale_2d: 130.0,
            noise_scale_3d: 90.0,

            flat_height_divisor: 18.0,
            flat_3d_noise_strength: 4.5,
            flat_water_level: 0,

            superflat_ground_level: 8,
            superflat_dirt_depth: 3,
            superflat_water_level: -32,
            superflat_generate_trees: true,

            planet_center: IVec3::new(0, 512, 0),
            planet_radius: 512.0,
            planet_shell_thickness: 96.0,
            planet_3d_noise_strength: 1.2,
            planet_inner_water: true,

            gravity_acceleration: 19.62,
            spawn_surface_offset: 64.0,
        }
    }
}

impl WorldGenConfig {
    pub fn sanitize(&mut self) {
        self.fbm_octaves = self.fbm_octaves.clamp(1, 12);

        self.noise_scale_2d = sanitize_f32(self.noise_scale_2d, 130.0).clamp(1.0, 100_000.0);
        self.noise_scale_3d = sanitize_f32(self.noise_scale_3d, 90.0).clamp(1.0, 100_000.0);

        self.flat_height_divisor = sanitize_f32(self.flat_height_divisor, 18.0).clamp(1.0, 10_000.0);
        self.flat_3d_noise_strength = sanitize_f32(self.flat_3d_noise_strength, 4.5).clamp(0.0, 64.0);
        self.superflat_dirt_depth = self.superflat_dirt_depth.clamp(1, 32);
        self.superflat_ground_level = self.superflat_ground_level.clamp(-2048, 4096);
        self.superflat_water_level = self.superflat_water_level.clamp(-4096, 4096);

        self.planet_radius = sanitize_f32(self.planet_radius, 512.0).clamp(16.0, 500_000.0);
        self.planet_shell_thickness = sanitize_f32(self.planet_shell_thickness, 96.0)
            .clamp(1.0, self.planet_radius.max(1.0));
        self.planet_3d_noise_strength = sanitize_f32(self.planet_3d_noise_strength, 1.2).clamp(0.0, 16.0);

        self.gravity_acceleration = sanitize_f32(self.gravity_acceleration, 19.62).clamp(0.0, 200.0);
        self.spawn_surface_offset = sanitize_f32(self.spawn_surface_offset, 64.0).clamp(0.0, 20_000.0);
    }

    pub fn world_up_at(&self, pos: Vec3) -> Vec3 {
        match self.terrain_mode {
            WorldTerrainMode::Flat => Vec3::Y,
            WorldTerrainMode::SuperFlat => Vec3::Y,
            WorldTerrainMode::Planet => {
                let dir = pos - self.planet_center.as_vec3();
                let up = dir.normalize_or_zero();
                if up.length_squared() <= 1e-6 || !up.is_finite() {
                    Vec3::Y
                } else {
                    up
                }
            }
        }
    }

    pub fn default_spawn_position_with_seed(&self, seed: u64) -> Vec3 {
        let (anchor, up, clearance) = match self.terrain_mode {
            WorldTerrainMode::Planet => {
                let anchor = self.planet_center.as_vec3() + Vec3::Y * self.planet_radius.max(16.0);
                let up = self.world_up_at(anchor);
                let clearance = self.spawn_surface_offset.clamp(8.0, 256.0);
                (anchor, up, clearance)
            }
            WorldTerrainMode::Flat => {
                let anchor = Vec3::new(0.0, self.flat_water_level as f32 + 96.0, 0.0);
                (anchor, Vec3::Y, self.spawn_surface_offset.clamp(6.0, 256.0))
            }
            WorldTerrainMode::SuperFlat => {
                let anchor = Vec3::new(0.0, self.superflat_ground_level as f32 + 64.0, 0.0);
                (anchor, Vec3::Y, self.spawn_surface_offset.clamp(6.0, 256.0))
            }
        };

        if let Some(pos) = self.find_surface_spawn(seed, anchor, up, clearance) {
            return pos;
        }

        match self.terrain_mode {
            WorldTerrainMode::Flat => Vec3::new(0.0, self.flat_water_level as f32 + self.spawn_surface_offset.max(64.0), 0.0),
            WorldTerrainMode::SuperFlat => Vec3::new(0.0, self.superflat_ground_level as f32 + self.spawn_surface_offset.max(32.0), 0.0),
            WorldTerrainMode::Planet => {
                self.planet_center.as_vec3()
                    + Vec3::Y * (self.planet_radius + self.spawn_surface_offset.max(64.0))
            }
        }
    }

    pub fn default_spawn_position(&self) -> Vec3 {
        self.default_spawn_position_with_seed(0)
    }

    fn find_surface_spawn(&self, seed: u64, anchor: Vec3, up_hint: Vec3, clearance: f32) -> Option<Vec3> {
        let up = up_hint.normalize_or_zero();
        if !up.is_finite() || up.length_squared() <= 1e-6 {
            return None;
        }

        let ref_axis = if up.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
        let tangent = up.cross(ref_axis).normalize_or_zero();
        let bitangent = up.cross(tangent).normalize_or_zero();
        if !tangent.is_finite() || tangent.length_squared() <= 1e-6 {
            return None;
        }

        let offsets = [
            Vec3::ZERO,
            tangent * 32.0,
            -tangent * 32.0,
            bitangent * 32.0,
            -bitangent * 32.0,
            (tangent + bitangent) * 24.0,
            (tangent - bitangent) * 24.0,
            (-tangent + bitangent) * 24.0,
            (-tangent - bitangent) * 24.0,
            tangent * 64.0,
            -tangent * 64.0,
            bitangent * 64.0,
            -bitangent * 64.0,
        ];

        for offset in offsets {
            let start = anchor + offset;
            if let Some(spawn) = self.surface_probe(seed, start, up, clearance) {
                return Some(spawn);
            }
        }

        None
    }

    fn surface_probe(&self, seed: u64, start: Vec3, up: Vec3, clearance: f32) -> Option<Vec3> {
        if !start.is_finite() {
            return None;
        }

        let step = 4.0;
        let max_steps = match self.terrain_mode {
            WorldTerrainMode::Planet => 1024,
            _ => 768,
        };

        let (start_density, _) = self.sample_density(seed, start);
        if !start_density.is_finite() {
            return None;
        }

        let mut solid = start;
        let mut air = start;
        let mut found = false;

        if start_density > 0.0 {
            // Start inside ground: march outward along local up until entering air.
            let mut p = start;
            for _ in 0..max_steps {
                let next = p + up * step;
                let (d, _) = self.sample_density(seed, next);
                if !d.is_finite() {
                    return None;
                }
                if d <= 0.0 {
                    solid = p;
                    air = next;
                    found = true;
                    break;
                }
                p = next;
            }
        } else {
            // Start in air/water: march downward until hitting solid.
            let mut p = start;
            for _ in 0..max_steps {
                let next = p - up * step;
                let (d, _) = self.sample_density(seed, next);
                if !d.is_finite() {
                    return None;
                }
                if d > 0.0 {
                    solid = next;
                    air = p;
                    found = true;
                    break;
                }
                p = next;
            }
        }

        if !found {
            return None;
        }

        for _ in 0..7 {
            let mid = (solid + air) * 0.5;
            let (d, _) = self.sample_density(seed, mid);
            if d > 0.0 {
                solid = mid;
            } else {
                air = mid;
            }
        }

        let mut spawn = air + up * clearance;
        for _ in 0..32 {
            let (d, tex) = self.sample_density(seed, spawn);
            if d <= 0.0 && tex != VoxTex::Water {
                return Some(spawn);
            }
            spawn += up * 2.0;
        }

        None
    }

    fn sample_density(&self, seed: u64, pos: Vec3) -> (f32, u16) {
        let mut fbm = Fbm::<Perlin>::new(fold_seed_u32(seed));
        fbm.octaves = self.fbm_octaves.clamp(1, 12) as usize;
        let (ofs2, ofs3) = seed_offsets(seed);

        let noise_scale_2d = self.noise_scale_2d.max(1.0);
        let noise_scale_3d = self.noise_scale_3d.max(1.0);
        let p = pos.round().as_ivec3();

        match self.terrain_mode {
            WorldTerrainMode::Planet => {
                let safe = pos.clamp(Vec3::splat(-100_000.0), Vec3::splat(100_000.0));
                let f_terr = fbm.get(((safe / noise_scale_2d).xz().as_dvec2() + ofs2).to_array()) as f32;
                let f_3d = fbm.get(((safe / noise_scale_3d).as_dvec3() + ofs3).to_array()) as f32;
                let d = (safe - self.planet_center.as_vec3()).length();
                let mut val = f_terr
                    - ((d - self.planet_radius.max(16.0)) / self.planet_shell_thickness.max(1.0))
                    + f_3d * self.planet_3d_noise_strength;
                if !val.is_finite() {
                    val = -1.0;
                }

                if val > 0.0 {
                    (val, VoxTex::Stone)
                } else if self.planet_inner_water && d < self.planet_radius && val < 0.0 {
                    (-0.1, VoxTex::Water)
                } else {
                    (val, VoxTex::Nil)
                }
            }
            WorldTerrainMode::Flat => {
                let f_terr = fbm.get((p.xz().as_dvec2() / noise_scale_2d as f64 + ofs2).to_array()) as f32;
                let f_3d = fbm.get((p.as_dvec3() / noise_scale_3d as f64 + ofs3).to_array()) as f32;
                let mut val = f_terr - (p.y as f32) / self.flat_height_divisor.max(1.0) + f_3d * self.flat_3d_noise_strength;
                if val > 0.0 {
                    (val, VoxTex::Stone)
                } else if p.y < self.flat_water_level {
                    val = -0.1;
                    (val, VoxTex::Water)
                } else {
                    (val, VoxTex::Nil)
                }
            }
            WorldTerrainMode::SuperFlat => {
                let stone_top = self.superflat_ground_level - self.superflat_dirt_depth;
                if p.y < stone_top {
                    (1.0, VoxTex::Stone)
                } else if p.y < self.superflat_ground_level {
                    (1.0, VoxTex::Dirt)
                } else if p.y == self.superflat_ground_level {
                    (1.0, VoxTex::Grass)
                } else if p.y <= self.superflat_water_level {
                    (-0.1, VoxTex::Water)
                } else {
                    (-1.0, VoxTex::Nil)
                }
            }
        }
    }
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

fn sanitize_f32(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct WorldMeta {
    pub schema_version: u32,
    pub name: String,
    pub seed: u64,
    pub created: i64,
    pub last_played: i64,
    pub config: WorldGenConfig,
    pub owner_username: Option<String>,
    pub admin_usernames: Vec<String>,
}

impl Default for WorldMeta {
    fn default() -> Self {
        Self {
            schema_version: WORLD_META_SCHEMA_VERSION,
            name: String::new(),
            seed: 0,
            created: 0,
            last_played: 0,
            config: WorldGenConfig::default(),
            owner_username: None,
            admin_usernames: Vec::new(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ActiveWorld {
    pub name: String,
    pub seed: u64,
    pub config: WorldGenConfig,
}

impl Default for ActiveWorld {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            seed: 100,
            config: WorldGenConfig::default(),
        }
    }
}

#[derive(Resource, Debug, Default, Clone)]
pub struct WorldSaveRequest {
    pub save_now: bool,
}

#[derive(Debug, Clone)]
pub struct LocalWorldInfo {
    pub name: String,
    pub seed: u64,
    pub last_played: i64,
    pub schema_version: u32,
    pub config: WorldGenConfig,
}

#[derive(Serialize, Deserialize, Clone)]
struct StoredCell {
    local_idx: u16,
    tex_id: u16,
    shape_id: VoxShape,
    isoval: u8,
}

#[derive(Serialize, Deserialize, Clone)]
struct StoredChunk {
    chunkpos: [i32; 3],
    is_populated: bool,
    cells: Vec<StoredCell>,
}

pub trait WorldStorage {
    fn save_chunk(&self, chunk: &Chunk) -> io::Result<()>;
    fn load_chunk(&self, chunkpos: IVec3) -> io::Result<Option<Chunk>>;
    fn save_world(&self, chunks: &HashMap<IVec3, ChunkPtr>) -> io::Result<()>;
}

#[cfg(not(target_arch = "wasm32"))]
type StorageBackend = FsWorldStorage;
#[cfg(target_arch = "wasm32")]
type StorageBackend = WasmWorldStorage;

pub struct ChunkStore {
    inner: StorageBackend,
}

impl ChunkStore {
    pub fn new(active: &ActiveWorld) -> io::Result<Self> {
        Ok(Self {
            inner: StorageBackend::new(active)?,
        })
    }

    pub fn save_chunk(&self, chunk: &Chunk) -> io::Result<()> {
        self.inner.save_chunk(chunk)
    }

    pub fn load_chunk(&self, chunkpos: IVec3) -> io::Result<Option<Chunk>> {
        self.inner.load_chunk(chunkpos)
    }

    pub fn save_world(&self, chunks: &HashMap<IVec3, ChunkPtr>) -> io::Result<()> {
        self.inner.save_world(chunks)
    }
}

fn io_other<E: std::error::Error + Send + Sync + 'static>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

fn unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn migrate_legacy_meta(meta: &mut WorldMeta, fallback_mode: Option<WorldTerrainMode>) -> bool {
    let mut changed = false;

    if meta.schema_version == 0 {
        if let Some(mode) = fallback_mode {
            meta.config.terrain_mode = mode;
            changed = true;
        }
    }

    let before = meta.config.clone();
    meta.config.sanitize();
    if before != meta.config {
        changed = true;
    }

    if meta.schema_version != WORLD_META_SCHEMA_VERSION {
        meta.schema_version = WORLD_META_SCHEMA_VERSION;
        changed = true;
    }

    let normalized_owner = meta
        .owner_username
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if normalized_owner != meta.owner_username {
        meta.owner_username = normalized_owner.clone();
        changed = true;
    }

    let mut normalized_admins = Vec::new();
    for raw in &meta.admin_usernames {
        let username = raw.trim();
        if username.is_empty() {
            changed = true;
            continue;
        }
        if normalized_owner
            .as_ref()
            .is_some_and(|owner| owner.eq_ignore_ascii_case(username))
        {
            changed = true;
            continue;
        }
        if normalized_admins
            .iter()
            .any(|v: &String| v.eq_ignore_ascii_case(username))
        {
            changed = true;
            continue;
        }
        normalized_admins.push(username.to_string());
    }
    if normalized_admins != meta.admin_usernames {
        meta.admin_usernames = normalized_admins;
        changed = true;
    }

    changed
}

fn set_admin_in_list(admins: &mut Vec<String>, username: &str, enabled: bool) -> bool {
    let username = username.trim();
    if username.is_empty() {
        return false;
    }
    let pos = admins
        .iter()
        .position(|v| v.eq_ignore_ascii_case(username));

    if enabled {
        if pos.is_none() {
            admins.push(username.to_string());
            return true;
        }
        false
    } else if let Some(idx) = pos {
        admins.remove(idx);
        true
    } else {
        false
    }
}

fn chunk_to_stored(chunk: &Chunk) -> StoredChunk {
    let mut cells = Vec::new();
    chunk.for_voxels(|vox, i| {
        if vox.is_nil() {
            return;
        }
        cells.push(StoredCell {
            local_idx: i as u16,
            tex_id: vox.tex_id,
            shape_id: vox.shape_id,
            isoval: vox.isoval,
        });
    });

    StoredChunk {
        chunkpos: [chunk.chunkpos.x, chunk.chunkpos.y, chunk.chunkpos.z],
        is_populated: chunk.is_populated,
        cells,
    }
}

fn stored_to_chunk(data: StoredChunk) -> Chunk {
    let pos = IVec3::new(data.chunkpos[0], data.chunkpos[1], data.chunkpos[2]);
    let mut chunk = Chunk::new(pos);
    chunk.is_populated = data.is_populated;

    for cell in data.cells {
        if cell.local_idx as usize >= Chunk::LEN3 {
            continue;
        }
        let lp = Chunk::local_idx_pos(cell.local_idx as i32);
        let mut vox = Vox::new(cell.tex_id, cell.shape_id, 0.0);
        vox.isoval = cell.isoval;
        *chunk.at_voxel_mut(lp) = vox;
    }

    chunk
}

fn encode_chunk(chunk: &Chunk) -> io::Result<Vec<u8>> {
    bincode::serialize(&chunk_to_stored(chunk)).map_err(io_other)
}

fn decode_chunk(bytes: &[u8]) -> io::Result<Chunk> {
    let data: StoredChunk = bincode::deserialize(bytes)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(stored_to_chunk(data))
}

pub fn sanitize_world_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    if out.is_empty() {
        "world".to_string()
    } else {
        out
    }
}

pub fn saves_root_dir() -> PathBuf {
    if let Ok(p) = std::env::var("ETHERTIA_SAVE_DIR") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        return PathBuf::from("wasm-memory-saves");
    }

    #[cfg(target_os = "android")]
    {
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return PathBuf::from(home).join("ethertia").join("saves");
            }
        }

        for base in [
            "/data/user/0/com.ethertia.client/files",
            "/data/data/com.ethertia.client/files",
        ] {
            let base_path = PathBuf::from(base);
            if base_path.exists() {
                return base_path.join("ethertia").join("saves");
            }
        }

        return PathBuf::from("/data/user/0/com.ethertia.client/files")
            .join("ethertia")
            .join("saves");
    }

    PathBuf::from("saves")
}

pub fn world_dir(name: &str) -> PathBuf {
    saves_root_dir().join(sanitize_world_name(name))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn world_has_persisted_chunks(name: &str) -> bool {
    let chunks_dir = world_dir(name).join(CHUNK_DIR);
    if !chunks_dir.exists() {
        return false;
    }

    match fs::read_dir(chunks_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .any(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "bin")),
        Err(_) => false,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn world_has_persisted_chunks(name: &str) -> bool {
    let clean = sanitize_world_name(name);
    match lock_wasm_state() {
        Ok(state) => state.chunks.get(&clean).is_some_and(|chunks| !chunks.is_empty()),
        Err(_) => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_world_meta(path: &Path) -> io::Result<WorldMeta> {
    let bytes = fs::read(path.join(META_FILE))?;
    serde_json::from_slice(&bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_world_meta(path: &Path, meta: &WorldMeta) -> io::Result<()> {
    fs::create_dir_all(path)?;
    let bytes = serde_json::to_vec_pretty(meta).map_err(io_other)?;
    fs::write(path.join(META_FILE), bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_worlds() -> io::Result<Vec<LocalWorldInfo>> {
    let root = saves_root_dir();
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join(META_FILE);
        if !meta_path.exists() {
            continue;
        }

        if let Ok(meta) = read_world_meta(&path) {
            worlds.push(LocalWorldInfo {
                name: meta.name,
                seed: meta.seed,
                last_played: meta.last_played,
                schema_version: meta.schema_version,
                config: meta.config,
            });
        }
    }

    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(worlds)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_world(name: &str, seed: u64) -> io::Result<WorldMeta> {
    create_world_with_config(name, seed, WorldGenConfig::default())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_world_with_config(name: &str, seed: u64, mut config: WorldGenConfig) -> io::Result<WorldMeta> {
    let name = sanitize_world_name(name);
    let dir = world_dir(&name);
    let now = unix_ts();
    config.sanitize();
    let meta = WorldMeta {
        schema_version: WORLD_META_SCHEMA_VERSION,
        name,
        seed,
        created: now,
        last_played: now,
        config,
        owner_username: None,
        admin_usernames: Vec::new(),
    };
    write_world_meta(&dir, &meta)?;
    fs::create_dir_all(dir.join(CHUNK_DIR))?;
    Ok(meta)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_world_meta(name: &str) -> io::Result<WorldMeta> {
    let dir = world_dir(name);
    let mut meta = read_world_meta(&dir)?;
    let fallback_mode = meta.config.terrain_mode;
    if migrate_legacy_meta(&mut meta, Some(fallback_mode)) {
        write_world_meta(&dir, &meta)?;
    }
    Ok(meta)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_world_meta(meta: &WorldMeta) -> io::Result<()> {
    let mut normalized = meta.clone();
    let fallback_mode = normalized.config.terrain_mode;
    migrate_legacy_meta(&mut normalized, Some(fallback_mode));
    let dir = world_dir(&normalized.name);
    write_world_meta(&dir, &normalized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_world_admin(name: &str, username: &str, enabled: bool) -> io::Result<WorldMeta> {
    let mut meta = load_world_meta(name)?;

    if meta
        .owner_username
        .as_ref()
        .is_some_and(|owner| owner.eq_ignore_ascii_case(username))
    {
        return Ok(meta);
    }

    if set_admin_in_list(&mut meta.admin_usernames, username, enabled) {
        save_world_meta(&meta)?;
    }
    Ok(meta)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn migrate_world_meta(name: &str, fallback_mode: WorldTerrainMode) -> io::Result<WorldMeta> {
    let dir = world_dir(name);
    let mut meta = read_world_meta(&dir)?;
    if migrate_legacy_meta(&mut meta, Some(fallback_mode)) {
        write_world_meta(&dir, &meta)?;
    }
    Ok(meta)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn delete_world(name: &str) -> io::Result<()> {
    let dir = world_dir(name);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FsWorldStorage {
    world_dir: PathBuf,
    chunks_dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FsWorldStorage {
    pub fn new(active: &ActiveWorld) -> io::Result<Self> {
        let name = sanitize_world_name(&active.name);
        let world_dir = world_dir(&name);
        let chunks_dir = world_dir.join(CHUNK_DIR);
        fs::create_dir_all(&chunks_dir)?;

        let mut meta = if world_dir.join(META_FILE).exists() {
            read_world_meta(&world_dir)?
        } else {
            create_world_with_config(&name, active.seed, active.config.clone())?
        };
        if migrate_legacy_meta(&mut meta, Some(active.config.terrain_mode)) {
            write_world_meta(&world_dir, &meta)?;
        }
        meta.last_played = unix_ts();
        write_world_meta(&world_dir, &meta)?;

        Ok(Self { world_dir, chunks_dir })
    }

    fn chunk_file_path(&self, chunkpos: IVec3) -> PathBuf {
        self.chunks_dir
            .join(format!("chunk_{}_{}_{}.bin", chunkpos.x, chunkpos.y, chunkpos.z))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldStorage for FsWorldStorage {
    fn save_chunk(&self, chunk: &Chunk) -> io::Result<()> {
        let bytes = encode_chunk(chunk)?;
        fs::write(self.chunk_file_path(chunk.chunkpos), bytes)
    }

    fn load_chunk(&self, chunkpos: IVec3) -> io::Result<Option<Chunk>> {
        let path = self.chunk_file_path(chunkpos);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)?;
        Ok(Some(decode_chunk(&bytes)?))
    }

    fn save_world(&self, chunks: &HashMap<IVec3, ChunkPtr>) -> io::Result<()> {
        for chunk in chunks.values() {
            self.save_chunk(chunk.as_ref())?;
        }

        let mut meta = read_world_meta(&self.world_dir).unwrap_or_default();
        meta.last_played = unix_ts();
        write_world_meta(&self.world_dir, &meta)
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct WasmMemoryState {
    worlds: HashMap<String, WorldMeta>,
    chunks: HashMap<String, HashMap<[i32; 3], Vec<u8>>>,
}

#[cfg(target_arch = "wasm32")]
static WASM_MEMORY_STATE: Lazy<Mutex<WasmMemoryState>> = Lazy::new(|| Mutex::new(WasmMemoryState::default()));

#[cfg(target_arch = "wasm32")]
fn lock_wasm_state() -> io::Result<MutexGuard<'static, WasmMemoryState>> {
    WASM_MEMORY_STATE
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to lock wasm world storage state"))
}

#[cfg(target_arch = "wasm32")]
pub fn list_worlds() -> io::Result<Vec<LocalWorldInfo>> {
    let state = lock_wasm_state()?;
    let mut worlds = state
        .worlds
        .values()
        .map(|meta| LocalWorldInfo {
            name: meta.name.clone(),
            seed: meta.seed,
            last_played: meta.last_played,
            schema_version: meta.schema_version,
            config: meta.config.clone(),
        })
        .collect::<Vec<_>>();
    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(worlds)
}

#[cfg(target_arch = "wasm32")]
pub fn create_world(name: &str, seed: u64) -> io::Result<WorldMeta> {
    create_world_with_config(name, seed, WorldGenConfig::default())
}

#[cfg(target_arch = "wasm32")]
pub fn create_world_with_config(name: &str, seed: u64, mut config: WorldGenConfig) -> io::Result<WorldMeta> {
    let clean = sanitize_world_name(name);
    let now = unix_ts();
    config.sanitize();
    let meta = WorldMeta {
        schema_version: WORLD_META_SCHEMA_VERSION,
        name: clean.clone(),
        seed,
        created: now,
        last_played: now,
        config,
        owner_username: None,
        admin_usernames: Vec::new(),
    };

    let mut state = lock_wasm_state()?;
    state.worlds.insert(clean.clone(), meta.clone());
    state.chunks.entry(clean).or_default();
    Ok(meta)
}

#[cfg(target_arch = "wasm32")]
pub fn load_world_meta(name: &str) -> io::Result<WorldMeta> {
    let clean = sanitize_world_name(name);
    let mut state = lock_wasm_state()?;
    let Some(meta) = state.worlds.get_mut(&clean) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, "world does not exist"));
    };
    migrate_legacy_meta(meta, Some(meta.config.terrain_mode));
    Ok(meta.clone())
}

#[cfg(target_arch = "wasm32")]
pub fn save_world_meta(meta: &WorldMeta) -> io::Result<()> {
    let mut normalized = meta.clone();
    migrate_legacy_meta(&mut normalized, Some(normalized.config.terrain_mode));
    let clean = sanitize_world_name(&normalized.name);
    let mut state = lock_wasm_state()?;
    state.worlds.insert(clean, normalized);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn set_world_admin(name: &str, username: &str, enabled: bool) -> io::Result<WorldMeta> {
    let mut meta = load_world_meta(name)?;

    if meta
        .owner_username
        .as_ref()
        .is_some_and(|owner| owner.eq_ignore_ascii_case(username))
    {
        return Ok(meta);
    }

    if set_admin_in_list(&mut meta.admin_usernames, username, enabled) {
        save_world_meta(&meta)?;
    }
    Ok(meta)
}

#[cfg(target_arch = "wasm32")]
pub fn migrate_world_meta(name: &str, fallback_mode: WorldTerrainMode) -> io::Result<WorldMeta> {
    let clean = sanitize_world_name(name);
    let mut state = lock_wasm_state()?;
    let Some(meta) = state.worlds.get_mut(&clean) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, "world does not exist"));
    };
    migrate_legacy_meta(meta, Some(fallback_mode));
    Ok(meta.clone())
}

#[cfg(target_arch = "wasm32")]
pub fn delete_world(name: &str) -> io::Result<()> {
    let clean = sanitize_world_name(name);
    let mut state = lock_wasm_state()?;
    state.worlds.remove(&clean);
    state.chunks.remove(&clean);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub struct WasmWorldStorage {
    world_name: String,
}

#[cfg(target_arch = "wasm32")]
impl WasmWorldStorage {
    pub fn new(active: &ActiveWorld) -> io::Result<Self> {
        let world_name = sanitize_world_name(&active.name);
        let now = unix_ts();

        let mut state = lock_wasm_state()?;
        let meta = state
            .worlds
            .entry(world_name.clone())
            .or_insert_with(|| WorldMeta {
                schema_version: WORLD_META_SCHEMA_VERSION,
                name: world_name.clone(),
                seed: active.seed,
                created: now,
                last_played: now,
                config: active.config.clone(),
                owner_username: None,
                admin_usernames: Vec::new(),
            });
        migrate_legacy_meta(meta, Some(active.config.terrain_mode));
        meta.seed = active.seed;
        meta.config = active.config.clone();
        meta.last_played = now;

        state.chunks.entry(world_name.clone()).or_default();
        Ok(Self { world_name })
    }

    fn chunk_key(chunkpos: IVec3) -> [i32; 3] {
        [chunkpos.x, chunkpos.y, chunkpos.z]
    }
}

#[cfg(target_arch = "wasm32")]
impl WorldStorage for WasmWorldStorage {
    fn save_chunk(&self, chunk: &Chunk) -> io::Result<()> {
        let bytes = encode_chunk(chunk)?;
        let mut state = lock_wasm_state()?;
        let chunks = state.chunks.entry(self.world_name.clone()).or_default();
        chunks.insert(Self::chunk_key(chunk.chunkpos), bytes);
        if let Some(meta) = state.worlds.get_mut(&self.world_name) {
            meta.last_played = unix_ts();
        }
        Ok(())
    }

    fn load_chunk(&self, chunkpos: IVec3) -> io::Result<Option<Chunk>> {
        let state = lock_wasm_state()?;
        let chunks = match state.chunks.get(&self.world_name) {
            Some(chunks) => chunks,
            None => return Ok(None),
        };
        let Some(bytes) = chunks.get(&Self::chunk_key(chunkpos)) else {
            return Ok(None);
        };
        Ok(Some(decode_chunk(bytes)?))
    }

    fn save_world(&self, chunks: &HashMap<IVec3, ChunkPtr>) -> io::Result<()> {
        let mut encoded = Vec::with_capacity(chunks.len());
        for chunk in chunks.values() {
            encoded.push((Self::chunk_key(chunk.chunkpos), encode_chunk(chunk.as_ref())?));
        }

        let mut state = lock_wasm_state()?;
        let world_chunks = state.chunks.entry(self.world_name.clone()).or_default();
        for (key, bytes) in encoded {
            world_chunks.insert(key, bytes);
        }
        if let Some(meta) = state.worlds.get_mut(&self.world_name) {
            meta.last_played = unix_ts();
        }
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::{
        fs,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("ethertia-save-test-{}-{}", std::process::id(), nanos))
    }

    fn with_temp_save_root(test: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        let _guard = TEST_ENV_LOCK
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to lock test env"))?;
        let root = unique_temp_dir();

        std::env::set_var("ETHERTIA_SAVE_DIR", &root);
        fs::create_dir_all(&root)?;

        let result = test();

        std::env::remove_var("ETHERTIA_SAVE_DIR");
        let _ = fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn world_meta_create_list_delete() -> io::Result<()> {
        with_temp_save_root(|| {
            let created = create_world("my world", 1234)?;
            assert_eq!(created.name, "my_world");
            assert_eq!(created.schema_version, WORLD_META_SCHEMA_VERSION);

            let listed = list_worlds()?;
            assert!(listed.iter().any(|w| w.name == "my_world" && w.seed == 1234));

            delete_world("my world")?;
            let listed = list_worlds()?;
            assert!(!listed.iter().any(|w| w.name == "my_world"));
            Ok(())
        })
    }

    #[test]
    fn chunk_roundtrip_save_load() -> io::Result<()> {
        with_temp_save_root(|| {
            let active = ActiveWorld {
                name: "roundtrip".to_string(),
                seed: 7,
                config: WorldGenConfig::default(),
            };
            let store = ChunkStore::new(&active)?;

            let chunkpos = IVec3::new(0, 0, 0);
            let mut chunk = Chunk::new(chunkpos);
            chunk.is_populated = true;

            let lp = IVec3::new(1, 2, 3);
            *chunk.at_voxel_mut(lp) = Vox::new(42, VoxShape::Cube, 0.0);

            store.save_chunk(&chunk)?;

            let loaded = store
                .load_chunk(chunkpos)?
                .expect("chunk should be present after save");
            assert!(loaded.is_populated);
            let vox = loaded.at_voxel(lp);
            assert_eq!(vox.tex_id, 42);
            assert_eq!(vox.shape_id, VoxShape::Cube);
            Ok(())
        })
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn wasm_memory_store_roundtrip() {
        let _ = delete_world("wasm_test_world");

        let active = ActiveWorld {
            name: "wasm_test_world".to_string(),
            seed: 77,
            config: WorldGenConfig::default(),
        };

        let store = ChunkStore::new(&active).expect("chunk store should initialize in wasm memory backend");

        let mut chunk = Chunk::new(IVec3::ZERO);
        chunk.is_populated = true;
        let lp = IVec3::new(2, 2, 2);
        *chunk.at_voxel_mut(lp) = Vox::new(11, VoxShape::Cube, 0.0);

        store.save_chunk(&chunk).expect("save chunk should work in wasm memory backend");
        let loaded = store
            .load_chunk(IVec3::ZERO)
            .expect("load should succeed")
            .expect("chunk should exist in wasm memory backend");

        assert!(loaded.is_populated);
        let vox = loaded.at_voxel(lp);
        assert_eq!(vox.tex_id, 11);
        assert_eq!(vox.shape_id, VoxShape::Cube);
    }
}