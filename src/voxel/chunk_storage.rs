use crate::prelude::*;
use super::{Chunk, Vox, VoxShape};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::VecDeque;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use zip::write::FileOptions as ZipFileOptions;
use std::io::{Seek, Write};
use std::thread;

<<<<<<< HEAD
#[derive(Clone, Serialize, Deserialize)]
struct SimpleVox {
    tex_id: u16,
    shape_id: u8,
    light: u16,
    isoval: u8,
}

impl From<Vox> for SimpleVox {
    fn from(v: Vox) -> Self {
        // pack light channels into u16
        let light = ((v.light.sky() as u16) << 12)
            | ((v.light.red() as u16 & 0xF) << 8)
            | ((v.light.green() as u16 & 0xF) << 4)
            | (v.light.blue() as u16 & 0xF);
        SimpleVox {
            tex_id: v.tex_id,
            shape_id: v.shape_id as u8,
            light,
            isoval: v.isoval,
        }
    }
}

impl Into<Vox> for SimpleVox {
    fn into(self) -> Vox {
        let mut vx = Vox::default();
        #[derive(Clone, Serialize, Deserialize)]
        struct SimpleVox {
            tex_id: u16,
            shape_id: u8,
            light: u16,
            isoval: u8,
        }

        impl From<Vox> for SimpleVox {
            fn from(v: Vox) -> Self {
                // pack light channels into u16
                let light = ((v.light.sky() as u16) << 12)
                    | ((v.light.red() as u16 & 0xF) << 8)
                    | ((v.light.green() as u16 & 0xF) << 4)
                    | (v.light.blue() as u16 & 0xF);
                SimpleVox {
                    tex_id: v.tex_id,
                    shape_id: v.shape_id as u8,
                    light,
                    isoval: v.isoval,
                }
            }
        }

        impl Into<Vox> for SimpleVox {
            fn into(self) -> Vox {
                let mut vx = Vox::default();
                vx.tex_id = self.tex_id;
                vx.shape_id = match self.shape_id {
                    x if x == VoxShape::Isosurface as u8 => VoxShape::Isosurface,
                    x if x == VoxShape::Cube as u8 => VoxShape::Cube,
                    x if x == VoxShape::Leaves as u8 => VoxShape::Leaves,
                    x if x == VoxShape::Grass as u8 => VoxShape::Grass,
                    x if x == VoxShape::SlabYMin as u8 => VoxShape::SlabYMin,
                    x if x == VoxShape::SlabYMax as u8 => VoxShape::SlabYMax,
                    x if x == VoxShape::SlabXMin as u8 => VoxShape::SlabXMin,
                    x if x == VoxShape::SlabXMax as u8 => VoxShape::SlabXMax,
                    x if x == VoxShape::SlabZMin as u8 => VoxShape::SlabZMin,
                    x if x == VoxShape::SlabZMax as u8 => VoxShape::SlabZMax,
                    x if x == VoxShape::Fence as u8 => VoxShape::Fence,
                    _ => VoxShape::Isosurface,
                };
                // unpack light
                let sky = (self.light >> 12) as u16;
                let r = ((self.light >> 8) & 0xF) as u16;
                let g = ((self.light >> 4) & 0xF) as u16;
                let b = (self.light & 0xF) as u16;
                vx.light.set_sky(sky);
                vx.light.set_red(r);
                vx.light.set_green(g);
                vx.light.set_blue(b);
                vx.isoval = self.isoval;
                vx
            }
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct ChunkSave {
            chunkpos: [i32; 3],
            voxels: Vec<SimpleVox>,
            populated: bool,
            chunk_format_version: u32,
        }

            Ok(save) => {
            use std::fs;

            let name = match world_name {
                // apply voxels
                let mut idx = 0usize;
                for i in 0..Chunk::LEN3 {
                    if idx >= save.voxels.len() { break; }
                    let sv = save.voxels[idx].clone();
                    let v: Vox = sv.into();
                    let local = Chunk::local_idx_pos(i as i32);
                    let vp = IVec3::new(local.x, local.y, local.z);
                    *chunk.at_voxel_mut(vp) = v;
                    idx += 1;
                }
                chunk.is_populated = save.populated;
                info!("Loaded chunk from {:?}", path);
                true
            }
            Err(err) => {
                warn!("Failed to deserialize chunk {:?}: {}", path, err);
                false
            }
        },
        Err(err) => {
            warn!("Failed to read chunk {:?}: {}", path, err);
            false
        }
    }
}


fn apply_saved_to_chunk(chunk: &mut Chunk, saved: ChunkSave) {
    // apply voxels
    let mut idx = 0usize;
    for i in 0..Chunk::LEN3 {
        if idx >= saved.voxels.len() { break; }
        let sv = &saved.voxels[idx];
        let v: Vox = sv.clone().into();
        let local = Chunk::local_idx_pos(i as i32);
        let vp = IVec3::new(local.x, local.y, local.z);
        *chunk.at_voxel_mut(vp) = v;
        idx += 1;
    }
    chunk.is_populated = saved.populated;
}

/// Simple in-memory LRU-ish cache for recently accessed saved chunks.
struct ChunkCache {
    map: std::collections::HashMap<IVec3, ChunkSave>,
    order: VecDeque<IVec3>,
    capacity: usize,
}

impl ChunkCache {
    fn new(cap: usize) -> Self {
        Self { map: std::collections::HashMap::new(), order: VecDeque::new(), capacity: cap }
    }

    fn get(&mut self, k: &IVec3) -> Option<ChunkSave> {
        if let Some(v) = self.map.get(k) {
            // refresh order
            if let Some(pos) = self.order.iter().position(|x| x == k) {
                self.order.remove(pos);
                self.order.push_back(*k);
            }
            return Some(v.clone());
        }
        None
    }

    fn put(&mut self, k: IVec3, v: ChunkSave) {
        if self.map.contains_key(&k) {
            // refresh order
            if let Some(pos) = self.order.iter().position(|x| x == &k) {
                self.order.remove(pos);
            }
        }
        self.order.push_back(k);
        self.map.insert(k, v);
        while self.order.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (&IVec3, &ChunkSave)> {
        self.map.iter()
    }
}

static CHUNK_CACHE: Lazy<Mutex<ChunkCache>> = Lazy::new(|| Mutex::new(ChunkCache::new(1024)));

/// Spawn a background thread to load a chunk from disk into the in-memory `CHUNK_CACHE`.
/// The loader will read and deserialize the chunk file if present and insert it into the cache.
/// This is intended to be called from non-blocking contexts (UI / main thread) so disk I/O
/// happens off-thread and the main thread can later pick up the chunk from the cache.
pub fn spawn_load_chunk_into_cache(world_name: String, pos: IVec3, seed: u64) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        use std::fs;

        let name = if !world_name.trim().is_empty() { world_name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_") } else { format!("world_{:016x}", seed) };
        let save_dir = crate::util::saves_root().join(&name).join("chunks");
        let fname = format!("chunk_{}_{}_{}.bin", pos.x, pos.y, pos.z);
        let path = save_dir.join(&fname);

        if !path.exists() {
            return;
        }

        match fs::read(&path) {
            Ok(bytes) => {
                if let Ok(save) = bincode::deserialize::<ChunkSave>(&bytes) {
                    if let Ok(mut cache) = CHUNK_CACHE.lock() {
                        cache.put(Chunk::as_chunkpos(pos), save);
                    } else {
                        warn!("CHUNK_CACHE mutex poisoned when background-loading chunk {:?}", pos);
                    }
                } else {
                    warn!("Failed to deserialize chunk during background load: {:?}", path);
                }
            }
            Err(err) => {
                warn!("Failed to read chunk during background load {:?}: {}", path, err);
            }
        }
    })
}

// update save function to populate cache on successful save
pub fn save_chunk_to_world(chunk: &Chunk, world_name: Option<&str>, seed: u64) {
    use std::fs::{self, File};
    use std::io::Write;

    let name = match world_name {
        Some(s) if !s.trim().is_empty() => s.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_"),
        _ => format!("world_{:016x}", seed),
    };

    let save_dir = crate::util::saves_root().join(&name).join("chunks");
    if let Err(err) = fs::create_dir_all(&save_dir) {
        warn!("Failed to create save dir {:?}: {}", save_dir, err);
        return;
    }

    let cx = chunk.chunkpos.x;
    let cy = chunk.chunkpos.y;
    let cz = chunk.chunkpos.z;
    let fname = format!("chunk_{}_{}_{}.bin", cx, cy, cz);
    let path = save_dir.join(&fname);

    // build simple vox vec
    let mut voxels: Vec<SimpleVox> = Vec::with_capacity(Chunk::LEN3);
    chunk.for_voxels(|v, _i| voxels.push(SimpleVox::from(*v)));

    let save = ChunkSave {
        chunkpos: [cx, cy, cz],
        voxels,
        populated: chunk.is_populated,
        chunk_format_version: 1,
    };

    match bincode::serialize(&save) {
        Ok(bytes) => {
            let tmp = path.with_extension("bin.tmp");
            match File::create(&tmp) {
                Ok(mut f) => {
                    if let Err(err) = f.write_all(&bytes) {
                        warn!("Failed to write chunk tmp {:?}: {}", tmp, err);
                        let _ = fs::remove_file(&tmp);
                        return;
                    }
                    if let Err(err) = fs::rename(&tmp, &path) {
                        warn!("Failed to rename chunk tmp to final {:?}: {}", path, err);
                    } else {
                        info!("Saved chunk {:?}", path);
                        // populate cache
                        if let Ok(mut cache) = CHUNK_CACHE.lock() {
                            cache.put(Chunk::as_chunkpos(IVec3::new(cx, cy, cz)), save.clone());
                        }
                    }
                }
                Err(err) => warn!("Failed to create chunk tmp {:?}: {}", tmp, err),
            }
        }
        Err(err) => warn!("Failed to serialize chunk: {}", err),
    }
}

/// Export a save folder as zip. If `include_cache` is true, cached chunks will be included
/// (they will be written into the chunks/ folder inside the archive if not already present on disk).
pub fn export_save_as_zip(save_name: &str, include_cache: bool) -> anyhow::Result<std::path::PathBuf> {
    use std::fs::{self, File};
    use std::io::Read;
    use std::path::PathBuf;

    let save_dir = crate::util::saves_root().join(save_name);
    if !save_dir.exists() {
        anyhow::bail!("Save not found: {}", save_name);
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let out_name = format!("{}_export_{}.zip", save_name, timestamp);
    let out_path = crate::util::saves_root().join(&out_name);

    let tmp_file = File::create(&out_path)?;
    let mut zip = zip::ZipWriter::new(tmp_file);
    let opts = ZipFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Add meta.json if exists
    let meta_path = save_dir.join("meta.json");
    if meta_path.exists() {
        let mut f = File::open(&meta_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        zip.start_file("meta.json", opts)?;
        zip.write_all(&buf)?;
    }

    // Prepare chunk list from disk
    let chunks_dir = save_dir.join("chunks");
    let mut existing_chunks = std::collections::HashSet::new();
    if chunks_dir.exists() {
        for e in fs::read_dir(&chunks_dir)? {
            let e = e?;
            if e.file_type()?.is_file() {
                let fname = e.file_name().to_string_lossy().into_owned();
                let rel = format!("chunks/{}", fname);
                let mut f = File::open(e.path())?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                zip.start_file(rel, opts)?;
                zip.write_all(&buf)?;
                existing_chunks.insert(fname);
            }
        }
    }

    // If include_cache, include cached chunks not on disk
    if include_cache {
        if let Ok(cache) = CHUNK_CACHE.lock() {
            for (pos, save) in cache.iter() {
                let fname = format!("chunk_{}_{}_{}.bin", pos.x, pos.y, pos.z);
                if existing_chunks.contains(&fname) { continue; }
                let bytes = bincode::serialize(save)?;
                let rel = format!("chunks/{}", fname);
                zip.start_file(rel, opts)?;
                zip.write_all(&bytes)?;
            }
        } else {
            warn!("CHUNK_CACHE mutex poisoned when exporting save {}", save_name);
        }
    }

    zip.finish()?;
    Ok(out_path)
}
/// Export the entire world save directory into a zip file under `exports/` for easy sharing.
pub fn export_world_save(world_name: Option<&str>, seed: u64) -> Option<std::path::PathBuf> {
    use std::fs::{self, File};
    use std::io::Write;
    use zip::write::ZipWriter;
    use zip::CompressionMethod;

    let name = match world_name {
        Some(s) if !s.trim().is_empty() => s.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_"),
        _ => format!("world_{:016x}", seed),
    };

    let src_dir = crate::util::saves_root().join(&name);
    if !src_dir.exists() {
        warn!("No save directory to export: {:?}", src_dir);
        return None;
    }

    let exports_dir = crate::util::saves_root().join("exports");
    if let Err(err) = fs::create_dir_all(&exports_dir) {
        warn!("Failed to create exports dir {:?}: {}", exports_dir, err);
        return None;
    }

    let out_name = format!("{}_{}.zip", name, chrono::Utc::now().format("%Y%m%d%H%M%S"));
    let out_path = exports_dir.join(out_name);

    match File::create(&out_path) {
        Ok(f) => {
            let mut zip = ZipWriter::new(f);

            fn add_dir_recursively<W: Write + std::io::Seek>(zip: &mut ZipWriter<W>, base: &Path, path: &Path) -> Result<(), anyhow::Error> {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let p = entry.path();
                    if p.is_dir() {
                        add_dir_recursively(zip, base, &p)?;
                    } else if p.is_file() {
                        let rel = p.strip_prefix(base)?.to_string_lossy().replace("\\", "/");
                        let data = std::fs::read(&p)?;
                        let options = zip::write::FileOptions::default().compression_method(CompressionMethod::Deflated);
                        zip.start_file(rel, options)?;
                        zip.write_all(&data)?;
                    }
                }
                Ok(())
            }

            match add_dir_recursively(&mut zip, &src_dir, &src_dir) {
                Ok(()) => {
                    if let Err(e) = zip.finish() {
                        warn!("Failed finalize zip {:?}: {}", out_path, e);
                        return None;
                    }
                    info!("Exported world save to {:?}", out_path);
                    Some(out_path)
                }
                Err(err) => {
                    warn!("Failed to zip save dir {:?}: {}", src_dir, err);
                    let _ = std::fs::remove_file(&out_path);
                    None
                }
            }
        }
        Err(err) => {
            warn!("Failed to create export file {:?}: {}", out_path, err);
            None
        }
    }
}

/// Spawn a background thread to run `export_save_as_zip` and return a JoinHandle.
/// Callers (UI) should use this to avoid blocking the main thread during export.
pub fn spawn_export_save_as_zip(save_name: String, include_cache: bool) -> std::thread::JoinHandle<anyhow::Result<std::path::PathBuf>> {
    thread::spawn(move || export_save_as_zip(&save_name, include_cache))
}

/// Spawn a background thread to run `export_world_save` and return a JoinHandle.
/// This avoids performing potentially large synchronous I/O on the caller's thread.
pub fn spawn_export_world_save(world_name: Option<String>, seed: u64) -> std::thread::JoinHandle<Option<std::path::PathBuf>> {
    thread::spawn(move || export_world_save(world_name.as_deref(), seed))
}

/// Spawn a background thread that serializes and writes a chunk to disk.
/// The caller provides a reference to the `Chunk`; this function will clone the necessary
/// data into a `ChunkSave` and perform the file write off-thread, then update `CHUNK_CACHE`.
pub fn spawn_save_chunk_from_chunk(chunk: &Chunk, world_name: Option<String>, seed: u64) -> std::thread::JoinHandle<()> {
    // Build the ChunkSave here on the caller thread to avoid borrowing across threads.
    let cx = chunk.chunkpos.x;
    let cy = chunk.chunkpos.y;
    let cz = chunk.chunkpos.z;
    let mut voxels: Vec<SimpleVox> = Vec::with_capacity(Chunk::LEN3);
    chunk.for_voxels(|v, _i| voxels.push(SimpleVox::from(*v)));

    let save = ChunkSave {
        chunkpos: [cx, cy, cz],
        voxels,
        populated: chunk.is_populated,
        chunk_format_version: 1,
    };

    let world_name_owned = world_name.unwrap_or_default();

    std::thread::spawn(move || {
        use std::fs::{self, File};
        use std::io::Write;

        let name = if !world_name_owned.trim().is_empty() { world_name_owned.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_") } else { format!("world_{:016x}", seed) };
        let save_dir = crate::util::saves_root().join(&name).join("chunks");
        if let Err(err) = fs::create_dir_all(&save_dir) {
            warn!("Failed to create save dir {:?}: {}", save_dir, err);
            return;
        }

        let fname = format!("chunk_{}_{}_{}.bin", cx, cy, cz);
        let path = save_dir.join(&fname);

        match bincode::serialize(&save) {
            Ok(bytes) => {
                let tmp = path.with_extension("bin.tmp");
                match File::create(&tmp) {
                    Ok(mut f) => {
                        if let Err(err) = f.write_all(&bytes) {
                            warn!("Failed to write chunk tmp {:?}: {}", tmp, err);
                            let _ = fs::remove_file(&tmp);
                            return;
                        }
                        if let Err(err) = fs::rename(&tmp, &path) {
                            warn!("Failed to rename chunk tmp to final {:?}: {}", path, err);
                        } else {
                            info!("Saved chunk {:?}", path);
                            if let Ok(mut cache) = CHUNK_CACHE.lock() {
                                cache.put(Chunk::as_chunkpos(IVec3::new(cx, cy, cz)), save.clone());
                            }
                        }
                    }
                    Err(err) => warn!("Failed to create chunk tmp {:?}: {}", tmp, err),
                }
            }
            Err(err) => warn!("Failed to serialize chunk: {}", err),
        }
    })
=======
use bevy::{
    math::IVec3,
    platform::collections::HashMap,
    prelude::Resource,
};
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

use super::{Chunk, ChunkPtr, Vox, VoxShape};

const META_FILE: &str = "meta.json";
const CHUNK_DIR: &str = "chunks";

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct WorldMeta {
    pub name: String,
    pub seed: u64,
    pub created: i64,
    pub last_played: i64,
}

#[derive(Resource, Debug, Clone)]
pub struct ActiveWorld {
    pub name: String,
    pub seed: u64,
}

impl Default for ActiveWorld {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            seed: 100,
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
            });
        }
    }

    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(worlds)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_world(name: &str, seed: u64) -> io::Result<WorldMeta> {
    let name = sanitize_world_name(name);
    let dir = world_dir(&name);
    let now = unix_ts();
    let meta = WorldMeta {
        name,
        seed,
        created: now,
        last_played: now,
    };
    write_world_meta(&dir, &meta)?;
    fs::create_dir_all(dir.join(CHUNK_DIR))?;
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
            create_world(&name, active.seed)?
        };
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
        })
        .collect::<Vec<_>>();
    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(worlds)
}

#[cfg(target_arch = "wasm32")]
pub fn create_world(name: &str, seed: u64) -> io::Result<WorldMeta> {
    let clean = sanitize_world_name(name);
    let now = unix_ts();
    let meta = WorldMeta {
        name: clean.clone(),
        seed,
        created: now,
        last_played: now,
    };

    let mut state = lock_wasm_state()?;
    state.worlds.insert(clean.clone(), meta.clone());
    state.chunks.entry(clean).or_default();
    Ok(meta)
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
                name: world_name.clone(),
                seed: active.seed,
                created: now,
                last_played: now,
            });
        meta.seed = active.seed;
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
>>>>>>> feature/world-persistence-8073199
}