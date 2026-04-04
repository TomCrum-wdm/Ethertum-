
use bevy::{
    math::IVec3,
    platform::collections::HashMap,
    prelude::Resource,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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

#[derive(Serialize, Deserialize)]
struct StoredCell {
    local_idx: u16,
    tex_id: u16,
    shape_id: VoxShape,
    isoval: u8,
}

#[derive(Serialize, Deserialize)]
struct StoredChunk {
    chunkpos: [i32; 3],
    is_populated: bool,
    cells: Vec<StoredCell>,
}

pub struct ChunkStore {
    world_dir: PathBuf,
    chunks_dir: PathBuf,
}

fn unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
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

fn read_world_meta(path: &Path) -> io::Result<WorldMeta> {
    let bytes = fs::read(path.join(META_FILE))?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_world_meta(path: &Path, meta: &WorldMeta) -> io::Result<()> {
    fs::create_dir_all(path)?;
    let bytes = serde_json::to_vec_pretty(meta).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(path.join(META_FILE), bytes)
}

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

pub fn delete_world(name: &str) -> io::Result<()> {
    let dir = world_dir(name);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

impl ChunkStore {
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

    pub fn save_chunk(&self, chunk: &Chunk) -> io::Result<()> {
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

        let data = StoredChunk {
            chunkpos: [chunk.chunkpos.x, chunk.chunkpos.y, chunk.chunkpos.z],
            is_populated: chunk.is_populated,
            cells,
        };

        let bytes = bincode::serialize(&data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(self.chunk_file_path(chunk.chunkpos), bytes)
    }

    pub fn load_chunk(&self, chunkpos: IVec3) -> io::Result<Option<Chunk>> {
        let path = self.chunk_file_path(chunkpos);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)?;
        let data: StoredChunk = bincode::deserialize(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

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

        Ok(Some(chunk))
    }

    pub fn save_world(&self, chunks: &HashMap<IVec3, ChunkPtr>) -> io::Result<()> {
        for chunk in chunks.values() {
            self.save_chunk(chunk.as_ref())?;
        }

        let mut meta = read_world_meta(&self.world_dir).unwrap_or_default();
        meta.last_played = unix_ts();
        write_world_meta(&self.world_dir, &meta)
    }
}