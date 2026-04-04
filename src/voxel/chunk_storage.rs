
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
            return PathBuf::from(home).join("ethertia").join("saves");
        }
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
}