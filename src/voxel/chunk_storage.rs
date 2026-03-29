use crate::prelude::*;
use super::{Chunk, Vox, VoxShape};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::VecDeque;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use zip::write::FileOptions as ZipFileOptions;
use std::io::{Seek, Write};

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
        vx.shape_id = unsafe { std::mem::transmute::<u8, VoxShape>(self.shape_id) };
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

pub fn load_chunk_from_world(chunk: &mut Chunk, world_name: Option<&str>, seed: u64) -> bool {
    use std::fs;

    let name = match world_name {
        Some(s) if !s.trim().is_empty() => s.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_"),
        _ => format!("world_{:016x}", seed),
    };

    let save_dir = crate::util::saves_root().join(&name).join("chunks");
    let cx = chunk.chunkpos.x;
    let cy = chunk.chunkpos.y;
    let cz = chunk.chunkpos.z;
    let fname = format!("chunk_{}_{}_{}.bin", cx, cy, cz);
    let path = save_dir.join(&fname);

    // Check in-memory cache first (avoid unwrap on poisoned mutex)
    if let Ok(mut cache) = CHUNK_CACHE.lock() {
        if let Some(saved) = cache.get(&chunk.chunkpos) {
            apply_saved_to_chunk(chunk, saved);
            info!("Loaded chunk from cache: {:?}", chunk.chunkpos);
            return true;
        }
    } else {
        warn!("CHUNK_CACHE mutex poisoned when loading chunk {:?}", chunk.chunkpos);
    }

    if !path.exists() {
        return false;
    }

    match fs::read(&path) {
        Ok(bytes) => match bincode::deserialize::<ChunkSave>(&bytes) {
            Ok(save) => {
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