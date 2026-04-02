
use std::sync::{Weak, Mutex};//解释：Weak 是 Rust 标准库中提供的一种智能指针类型，用于实现非拥有（non-owning）引用。它与 Arc（Atomic Reference Counted）配合使用，Arc 用于实现共享所有权，而 Weak 则用于实现弱引用。当你使用 Arc 来创建一个共享所有权的智能指针时，Arc 会维护一个

use crate::prelude::*;


// Chunk is "Heavy" type (big size, stored a lot voxels). thus copy/clone are not allowed.
pub struct Chunk {
    // should Box?
    voxels: [Vox; Self::LEN3],//LEN3哪里来的：LEN3 是 Chunk 结构体中的一个常量，表示一个 Chunk 中包含的 voxel 数量。根据代码中的定义，LEN3 的值是 LEN 的立方，即 16^3 = 4096。这意味着每个 Chunk 中包含 4096 个 voxel。//LEN3是谁起的名字，是rust自带吗：LEN3 是在 Chunk 结构体中定义的一个常量，表示一个 Chunk 中包含的 voxel 数量。它是由开发者自己定义的，并不是 Rust 自带的。开发者选择了这个名字来表示 LEN 的立方，即 16^3 = 4096，以便更清晰地表达这个常量的含义//为什么名字这么怪，为什么len=16：LEN 是一个常量，表示一个 Chunk 在每个维度上的长度。在这个代码中，LEN 的值是 16，这意味着每个 Chunk 在 x、y、z 方向上都有 16 个 voxel。因此，一个 Chunk 中的 voxel 总数就是 LEN 的立方，即 16^3 = 4096。开发者选择了这个名字可能是为了简洁和清晰地表达这个常量的含义，虽然它可能看起来有些怪异，但在上下文中是有意义的。//self什么意思：在 Rust 中，self 是一个特殊的关键字，用于表示当前实例的引用。在结构体的方法中，self 代表调用该方法的实例。通过 self，你可以访问和修改实例的字段和调用其他方法。在这个代码中，self 被用来访问 Chunk 结构体中的字段和方法，例如 self.voxel、self.chunkpos、self.is_populated 等。；Vox 是一个类型，表示一个 voxel 的数据结构。它可能包含有关 voxel 的各种属性，例如类型、状态、光照等。在这个代码中，voxel 是一个数组，包含了一个 Chunk 中所有的 voxel 数据。整体含义是，Chunk 结构体包含了一个 voxel 数组，用于存储一个 Chunk 中的所有 voxel 数据，以及其他与 Chunk 相关的属性和方法。；为什么voxel跟Vox都是数组：在这个代码中，voxel 是一个数组，包含了一个 Chunk 中所有的 voxel 数据。Vox 是一个类型，表示一个 voxel 的数据结构。由于一个 Chunk 中包含了多个 voxel，因此使用数组来存储这些 voxel 数据是合理的。通过使用数组，可以方便地访问和管理一个 Chunk 中的所有 voxel 数据。每个元素都是一个 Vox 类型的实例，代表一个具体的 voxel 的数据。//voxel,Vox,chunk到底什么区别：在这个代码中，Chunk 是一个结构体，表示一个 Chunk 的数据结构。它包含了一个 voxel 数组，用于存储一个 Chunk 中的所有 voxel 数据，以及其他与 Chunk 相关的属性和方法。Vox 是一个类型，表示一个 voxel 的数据结构。它可能包含有关 voxel 的各种属性，例如类型、状态、光照等。在这个代码中，voxel 是一个数组，包含了一个 Chunk 中所有的 voxel 数据，每个元素都是一个 Vox 类型的实例，代表一个具体的 voxel 的数据。因此，Chunk 是一个更高层次的结构体，用于管理和组织多个 voxel 数据，而 Vox 则是一个更底层的类型，用于表示单个 voxel 的数据。//😰为什么voxel包含voxel?为什么Vox才是真正的voxel?这样起名是不是很糟糕？😰在这个代码中，Chunk 结构体包含了一个 voxel 数组，用于存储一个 Chunk 中的所有 voxel 数据。每个元素都是一个 Vox 类型的实例，代表一个具体的 voxel 的数据。虽然命名可能看起来有些混乱，但它是为了区分不同层次的概念而设计的。Chunk 是一个更高层次的结构体，用于管理和组织多个 voxel 数据，而 Vox 则是一个更底层的类型，用于表示单个 voxel 的数据。通过这种命名方式，可以更清晰地表达不同层次的概念，尽管可能会让人感到有些混乱，但在上下文中是有意义的。

    pub chunkpos: IVec3,

    pub is_populated: bool,

    pub entity: Entity,
    pub mesh_handle_terrain: Handle<Mesh>, // solid terrain
    pub mesh_handle_foliage: Handle<Mesh>,
    pub mesh_handle_liquid: Handle<Mesh>,

    // cached neighbor chunks that loaded to the ChunkSystem.
    // for Quick Access without global find neighbor chunk by chunkpos
    pub neighbor_chunks: [Option<Weak<Mutex<Chunk>>>; Self::NEIGHBOR_DIR.len()],

    // Self Arc. for export self Arc in get_chunk_neib().  assigned by ChunkSystem::spawn_chunk()
    pub chunkptr_weak: Weak<Mutex<Chunk>>,  
}

impl Chunk {
    pub const LEN: i32 = 16; // i32 for xyz iter
    pub const LEN3: usize = (Self::LEN * Self::LEN * Self::LEN) as usize;
    // pub const LOCAL_IDX_CAP: usize = 4096;  // 16^3, 2^12 bits (12 = 3 axes * 4 bits)

    pub fn new(chunkpos: IVec3) -> Self {
        Self {
            voxels: [Vox::default(); Self::LEN3],
            chunkpos,
            is_populated: false,
            neighbor_chunks: Default::default(),
            chunkptr_weak: Weak::default(),
            entity: Entity::PLACEHOLDER,
            mesh_handle_terrain: Handle::default(),
            mesh_handle_foliage: Handle::default(),
            mesh_handle_liquid: Handle::default(),
        }
    }

    // Voxel Cell

    // pub fn ax_voxel(&self, local_idx: usize) -> &Vox {
    //     &self.voxels[local_idx]
    // }

    pub fn at_voxel(&self, localpos: IVec3) -> &Vox {
        &self.voxels[Chunk::local_idx(localpos)]
    }

    pub fn at_voxel_mut(&mut self, localpos: IVec3) -> &mut Vox {
        &mut self.voxels[Chunk::local_idx(localpos)]
    }

    pub fn get_voxel_rel(&self, relpos: IVec3) -> Option<Vox> {
        if Chunk::is_localpos(relpos) {
            Some(*self.at_voxel(relpos))
        } else {
            let neib_chunkptr = self.get_chunk_rel(relpos)?;
            // Avoid blocking cross-chunk lock waits: if neighbor is busy, skip this sample.
            // Callers that use `*_or_default` will gracefully fall back.
            let guard = neib_chunkptr.try_lock().ok()?;
            Some(*guard.at_voxel(Chunk::as_localpos(relpos)))
        }
    }

    pub fn set_voxel_rel(&mut self, relpos: IVec3, mut visitor: impl FnMut(&mut Vox)) -> Option<Vox> {
        if Chunk::is_localpos(relpos) {
            let vox = self.at_voxel_mut(relpos);
            visitor(vox);
            return Some(*vox);
        }

        let neib_chunkptr = self.get_chunk_rel(relpos)?;
        // Avoid potential deadlocks from lock-order inversion across adjacent chunks.
        let mut neib_chunk = neib_chunkptr.try_lock().ok()?;
        let vox = neib_chunk.at_voxel_mut(Chunk::as_localpos(relpos));
        visitor(vox);
        Some(*vox)
    }
    pub fn get_voxel_rel_or_default(&self, relpos: IVec3) -> Vox {
        self.get_voxel_rel(relpos).unwrap_or(Vox::default())
    }

    pub fn for_voxels(&self, mut visitor: impl FnMut(&Vox, usize)) {
        for i in 0..Self::LEN3 {
            visitor(&self.voxels[i], i);
        }
    }

    // light sources
    pub fn for_voxel_lights(&self, mut visitor: impl FnMut(&Vox, usize)) {
        self.for_voxels(|v, i| {
            if v.tex_id == VoxTex::Log {  // v.id.light_emission != 0
                visitor(v, i);
            }
        });
    }

    pub fn get_chunk_rel(&self, relpos: IVec3) -> Option<ChunkPtr> {
        if Chunk::is_localpos(relpos) {
            return self.chunkptr_weak.upgrade();
        }
        self.get_chunk_neib(Chunk::neighbor_idx(relpos)?)
    }

    pub fn get_chunk_neib(&self, neib_idx: usize) -> Option<ChunkPtr> {
        // if neib_idx == usize::MAX {
        //     return Some(self.chunkptr_weak.upgrade()?);
        // }
        if let Some(neib_weak) = &self.neighbor_chunks[neib_idx] {
            // assert!(neib_chunk.chunkpos == self.chunkpos + Self::NEIGHBOR_DIR[neib_idx] * Chunk::LEN, "self.chunkpos = {}, neib {} pos {}", self.chunkpos, neib_idx, neib_chunk.chunkpos);
            return Some(neib_weak.upgrade()?);
        }
        None
    }



    pub fn as_chunkpos(p: IVec3) -> IVec3 {
        fn _floor16(x: i32) -> i32 { x & (!15) }
        IVec3::new(_floor16(p.x), _floor16(p.y), _floor16(p.z))
    }

    pub fn as_localpos(p: IVec3) -> IVec3 {
        fn _mod16(x: i32) -> i32 { x & 15 }
        IVec3::new(_mod16(p.x), _mod16(p.y), _mod16(p.z))
    }

    /// mod(p, 16) == 0
    pub fn is_chunkpos(p: IVec3) -> bool {
        p % 16 == IVec3::ZERO
    }
    // [0, 16)
    pub fn is_localpos(p: IVec3) -> bool {
        p.x >= 0 && p.x < 16 && p.y >= 0 && p.y < 16 && p.z >= 0 && p.z < 16
    }

    // the index range is [0, 16^3 or 4096)
    pub fn local_idx(localpos: IVec3) -> usize {
        if !Chunk::is_localpos(localpos) {
            debug!("Invalid localpos {}, clamped into chunk-local range", localpos);
            return Chunk::local_idx(Chunk::as_localpos(localpos));
        }
        (localpos.x << 8 | localpos.y << 4 | localpos.z) as usize
    }
    pub fn local_idx_pos(idx: i32) -> IVec3 {
        IVec3::new((idx >> 8) & 15, (idx >> 4) & 15, idx & 15)
    }

    pub fn at_boundary_naive(localpos: IVec3) -> i32 {
        if localpos.x == 0 {
            return 0;
        }
        if localpos.x == 15 {
            return 1;
        }
        if localpos.y == 0 {
            return 2;
        }
        if localpos.y == 15 {
            return 3;
        }
        if localpos.z == 0 {
            return 4;
        }
        if localpos.z == 15 {
            return 5;
        }
        -1
        // localpos.x == 0 || localpos.x == 15 ||
        // localpos.y == 0 || localpos.y == 15 ||
        // localpos.z == 0 || localpos.z == 15
    }


    #[rustfmt::skip]
    pub const NEIGHBOR_DIR: [IVec3; 6 + 12 + 8] = [
        // 6 Faces
        ivec3(-1, 0, 0),
        ivec3(1, 0, 0),
        ivec3(0, -1, 0),
        ivec3(0, 1, 0),
        ivec3(0, 0, -1),
        ivec3(0, 0, 1),
        // 12 Edges
        ivec3(0, -1, -1), // X
        ivec3(0, 1, 1),
        ivec3(0, 1, -1),
        ivec3(0, -1, 1),
        ivec3(-1, 0, -1), // Y
        ivec3(1, 0, 1),
        ivec3(1, 0, -1),
        ivec3(-1, 0, 1),
        ivec3(-1, -1, 0), // Z
        ivec3(1, 1, 0),
        ivec3(-1, 1, 0),
        ivec3(1, -1, 0),
        // 8 Vertices
        ivec3(-1, -1, -1),
        ivec3(1, 1, 1),
        ivec3(1, -1, -1),
        ivec3(-1, 1, 1),
        ivec3(-1, -1, 1),
        ivec3(1, 1, -1),
        ivec3(1, -1, 1),
        ivec3(-1, 1, -1),
    ];

    pub fn is_neighbors_all_loaded(&self) -> bool {
        self.neighbor_chunks.iter().all(Option::is_some)
    }

    fn neighbor_idx(relpos: IVec3) -> Option<usize> {
        if Chunk::is_localpos(relpos) {
            return None;
        }
        (0..Chunk::NEIGHBOR_DIR.len()).find(|&i| Chunk::is_localpos(relpos - (Chunk::NEIGHBOR_DIR[i] * Chunk::LEN as i32)))
    }

    // assert!(Self::NEIGHBOR_DIR[idx] + Self::NEIGHBOR_DIR[opposite_idx] == IVec3::ZERO, "idx = {}, opposite = {}", idx, opposite_idx);
    pub fn neighbor_idx_opposite(idx: usize) -> usize {
        // idx MOD 2 + (idx+1) MOD 2
        idx / 2 * 2 + (idx + 1) % 2
    }







    // Voxel Light

    // pub fn at_lights(&self, localpos: IVec3) -> &VoxLight {
    //     &self.light[Chunk::local_idx(localpos)]
    // }
    // pub fn at_lights_mut(&self, localpos: IVec3) -> &mut VoxLight {
    //     as_mut(self.at_lights(localpos))
    // }

    // pub fn reset_lights(&mut self) {
    //     self.light = [VoxLight::default(); Self::LEN3];
    // }

    // pub fn at_light(&self, localpos: IVec3, chan: u8) -> u16 {
    //     self.at_lights(Chunk::local_idx(localpos)).get(chan)
    // }

    // pub fn set_light(&mut self, local_idx: u16, chan: u8, val: u8) {
    //     self.at_lights_mut(local_idx).set(chan, val);
    // }

}
