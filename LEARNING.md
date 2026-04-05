# Ethertum 项目入门与贡献者指南

**目标读者**
- 完全没有编程经验但想学 Rust 并参与本项目的初学者。
- 想了解本仓库架构并能做小改动、修复或实现功能的开发者。

**使用方式**
- 建议按模块顺序学习，每个模块包含阅读清单、实践练习与小任务。
- 每完成一个练习，尽量在本地编译运行并提交小的 Git 分支（`git checkout -b feat/xxx`）。

**准备环境（必做）**
- 安装 Rust（推荐 rustup + stable）。
- 安装 Git 与 VS Code。
- 在仓库根目录执行：

```bash
cargo build
```

若构建失败，先把错误截图或日志保存以便寻求帮助。

**快速导航（项目关键文件）**
- 主入口：[src/main.rs](src/main.rs#L1)
- 体素（voxel）模块：
  - 区块数据结构：[src/voxel/chunk.rs](src/voxel/chunk.rs#L1)
  - 单元格定义：[src/voxel/vox.rs](src/voxel/vox.rs#L1)
  - 区块存储与序列化：[src/voxel/chunk_storage.rs](src/voxel/chunk_storage.rs#L1)
  - 世界生成：[src/voxel/worldgen.rs](src/voxel/worldgen.rs#L1)
  - 服务器加载/保存：[src/voxel/voxel_server.rs](src/voxel/voxel_server.rs#L1)
# Ethertum 面向零基础的完整入门教材（教学版）

欢迎！本教材把你当成“完全不懂编程、也不了解计算机基本概念”的初学者来编写。
先别担心术语，我会手把手说明如何阅读本教材、如何看代码、如何运行以及如何一步步学习并参与项目。

重要说明：本文件使用 Markdown（简称 MD）书写——这是一个文本格式，用于写说明文档。你不需要懂任何技术就能看懂：

- 标题用 `#` 表示，例如 `#` 是一级标题，`##` 是二级标题。它们只是章节标题。
- 代码或命令会放在单独的代码块里，像这样：

```bash
cargo build
```

  上面那段就是命令：打开“终端/命令行”后把 `cargo build` 粘贴并回车即可运行。
- 加粗会用 `**加粗**` 表示，链接会显示为可点击的文本（在 GitHub 或 VS Code 中可以点击）。

如何在电脑上正确看本教材（最简单方法）：
1. 推荐使用 VS Code（免费）。在 VS Code 中打开仓库文件夹，点击左侧文件列表里的 `LEARNING.md`，VS Code 会自动把它渲染成漂亮的页面，命令和代码块一目了然。你也可以在浏览器中打开 GitHub 仓库查看。
2. 如果只看到原始文本（有 `#`、```），那也没关系——这就是 Markdown 原文，阅读顺序与普通文档相同，代码块可以直接复制。

现在我们从最基础开始：

重要安全规则（请务必遵守）
- 请不要直接在仓库的源文件上进行破坏性修改。许多练习原先会建议修改代码以观察效果，但这可能会让项目状态难以恢复。
- 推荐做法：在 `docs/` 或本地的独立文件夹中复制源码片段（或在 `docs/annotated/` 中创建注释副本），在副本上做实验与注释；不要直接提交或覆盖 `src/` 下的文件，除非你非常确定并在分支上操作。

下面的“练习”主要以“阅读 + 注释”为主，示例代码直接摘自仓库以供学习，但请把修改限制在副本中。


====================
第一部分：极基础（给从未接触过计算机的人）
====================

什么是计算机程序？
- 计算机是“做事的机器”。程序就是告诉机器“按步骤做事情”的说明书。
- 程序由许多指令组成，指令按顺序执行，指令可以做算术、记住信息（存储）和判断（条件分支）等。

什么是文件与文件夹？
- 文件就像纸张，文件夹就像文件柜。代码通常保存在以 `.rs`（Rust 源代码）、`.md`（文档）等为后缀的文件里。

什么是命令行（终端）？
- 终端是你和计算机直接对话的地方。你可以在里面输入命令（像 `cargo build`），计算机会返回结果（成功或错误）。

简单的数学与逻辑（为理解代码做准备）
- 加法减法乘除，这些在编程里叫“算术运算”。
- 条件：例如 “如果 温度 > 30 度，则 打开风扇”，这就是条件判断（在代码里叫 `if`）。
- 循环：例如反复做某件事直到满足条件（在代码里叫 `for` 或 `while`）。
- 坐标：游戏世界里常用三维坐标（x, y, z），就像地图上的东西有经纬度。

====================
第二部分：入门必备工具（实践）
====================

安装与基本命令（逐条执行）
1. 安装 Rust：运行（Windows 在 PowerShell 中运行）：

```powershell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

  安装完成后请重启终端，运行：

```bash
rustc --version
cargo --version
```

2. 安装 Git：在终端运行：

```bash
git --version
```

3. 安装 VS Code：打开仓库文件夹并点击 `LEARNING.md` 阅读文档。

第一次在仓库里尝试构建（一步步来）
1. 打开终端，切换到项目根目录（你电脑上项目所在的文件夹）。
2. 运行：

```bash
cargo build
```

3. 如果看到很多输出并最后出现 `Finished ...`，说明构建成功。若出现错误，把前 20 行错误信息复制给我，我会帮你分析。

====================
第三部分：理解本项目（逐步带读源码）
====================

我把项目分成若干简单章节，每章都配有练习：

- 章节 1：程序入口
  - 要读：`src/main.rs`
  - 目标：找到 `main` 并理解程序最开始做的事情。
  - 练习：在 `main` 中加 `println!("Hello from Ethertum");`，运行验证。

  ----
  示例：源码逐行注释（安全的阅读/练习方式）

  下面是一个完整的示例：我把 `src/voxel/chunk.rs` 中的关键片段摘录出来，并逐行注释，示范如何把“Rust 知识点”直接与仓库源码对应起来。请把下面的代码视为“只读参考”——若要实验，请先在 `docs/annotated/` 或本地副本里复制并修改。

  ```rust
  // Chunk is "Heavy" type (big size, stored a lot voxels). thus copy/clone are not allowed.
  pub struct Chunk {
    voxels: [Vox; Self::LEN3],
    pub chunkpos: IVec3,
    pub is_populated: bool,
    pub entity: Entity,
    pub mesh_handle_terrain: Handle<Mesh>,
    pub mesh_handle_foliage: Handle<Mesh>,
    pub mesh_handle_liquid: Handle<Mesh>,
    pub neighbor_chunks: [Option<Weak<Chunk>>; Self::NEIGHBOR_DIR.len()],
    pub chunkptr_weak: Weak<Chunk>,
  }

  impl Chunk {
    pub const LEN: i32 = 16; // i32 for xyz iter
    pub const LEN3: usize = (Self::LEN * Self::LEN * Self::LEN) as usize;

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

    pub fn at_voxel(&self, localpos: IVec3) -> &Vox {
      &self.voxels[Chunk::local_idx(localpos)]
    }

    pub fn at_voxel_mut(&self, localpos: IVec3) -> &mut Vox {
      self.at_voxel(localpos).as_mut()
    }

    pub fn local_idx(localpos: IVec3) -> usize {
      (localpos.x << 8 | localpos.y << 4 | localpos.z) as usize
    }
  }
  ```

  逐行注释（要点说明）
  1. `pub struct Chunk { ... }` — 结构体声明：这是 Rust 的自定义复合类型，用来封装区块的状态与行为（数据 + 方法）。
  2. `voxels: [Vox; Self::LEN3],` — 固定长度数组：在内存中为每个 `Chunk` 预留 `LEN3` 个 `Vox`，体现内存布局与连续数组的好处（顺序遍历效率高）。
  3. `pub chunkpos: IVec3,` — 公有字段：`pub` 允许模块外访问；`IVec3` 表示三维整型坐标（类型示例）。
  4. `neighbor_chunks: [Option<Weak<Chunk>>; ...]` — 使用 `Weak` 避免循环强引用导致内存泄漏；`Option` 表示某些邻居可能不存在。这里可讨论所有权（Ownership）与引用计数（Rc/Arc/Weak）。
  5. `pub const LEN: i32 = 16;` 与 `LEN3` — 常量定义：`LEN3` 用于数组大小，展示类型常量与编译期计算。注意 `LEN` 为 `i32`，`LEN3` 为 `usize`，这体现类型在不同 API/上下文中的选择。
  6. `pub fn new(chunkpos: IVec3) -> Self { ... }` — 关联函数（构造器模式）：`Self { ... }` 初始化每个字段。这里 `voxels: [Vox::default(); Self::LEN3]` 展示如何用默认值初始化大数组（`Vox::default()` 需实现 `Default` trait）。
  7. `pub fn at_voxel(&self, localpos: IVec3) -> &Vox { ... }` — 不可变借用（immutable borrow）：返回对内部数组元素的不可变引用，演示借用规则：`&self` 表示对 `self` 的只读借用，函数返回的 `&Vox` 与 `self` 的生命周期相关。
  8. `pub fn at_voxel_mut(&self, localpos: IVec3) -> &mut Vox { ... }` — 可变借用（mutable borrow）：尽管签名里是 `&self`（只读借用），实现通过内部可变手段（例如 `as_mut()`）获得可变引用；在学习时关注编译器如何保证同时只有一个可变引用存在（借用规则）。
  9. `local_idx` 的位运算：`(localpos.x << 8 | localpos.y << 4 | localpos.z)` 展示如何把三维坐标压缩为一维索引，体现位运算与性能优化的实践。

  如何用这些示例学习 Rust 概念（不改源码）
  - 复制上面的代码块到 `docs/annotated/chunk_explained.md` 或你的本地笔记文件中；逐行添加你自己的中文注释。
  - 不要直接替换 `src/voxel/chunk.rs`，若要实验借用与所有权，可以在本地新建一个 `examples/` 小程序或在 `docs/` 中编写小的演示代码并运行那里（与主项目分离）。

  以上即为“把仓库代码直接用于教学”的示范：我会以这种方式把更多概念（变量/类型/模式匹配/泛型/错误处理/模块与包等）逐条映射到项目源码的具体片段并逐行注释，生成可阅读的教学材料。


- 章节 2：体素简介
  - 要读：`src/voxel/vox.rs`，`src/voxel/chunk.rs`。
  - 练习：确认 `Chunk::LEN == 16` 并计算 `Chunk::LEN3`（应为 4096）。

- 章节 3：存储策略
  - 要读：`src/voxel/chunk_storage.rs` 中的 `StoredCell` / `StoredChunk` 与保存函数。
  - 要点：内存中保存完整区块；存盘时只写入非空格子（稀疏保存），这就是“只存储改变的内容”的含义。

- 章节 4：世界生成（种子）
  - 要读：`src/voxel/worldgen.rs` 中的 `generate_chunk_with_seed`。
  - 练习：修改默认 seed，运行并观察差异。

- 章节 5：区块加载/卸载
  - 要读：`src/voxel/voxel_server.rs` 的 `chunks_load` 函数。
  - 要点：优先尝试从磁盘加载区块，找不到时使用 `seed` 生成新区块，区块在无人需要时被保存并卸载。

- 章节 6：客户端/服务器与网络
  - 要读：`src/net/netproc_client.rs`（接收 `ChunkNew`）与服务端发送代码。

每学完一章，写两行笔记：1) 这段代码做什么；2) 如果改它要改哪。

====================
第四部分：逐步练习清单（手把手）
====================

初级练习（第 1 周）
1. 找到 `ActiveWorld::default()`（文件 `src/voxel/chunk_storage.rs`），把默认 seed 改成 `42`，运行并观察。

中级练习（第 2 周）
1. 在 `src/voxel/voxel_server.rs` 的加载分支插入日志：

```rust
info!("Load Chunk from disk {}", chunkpos);
info!("Generate Chunk {} with seed {}", chunkpos, world.seed);
```

2. 运行服务器并记录日志，确认能够看到“Generate Chunk”或“Load Chunk”。

贡献者小任务（提交 PR）
1. 新建分支：`git checkout -b docs/learning-improve`。
2. 修改 `LEARNING.md`，提交并推送：

```bash
git add LEARNING.md
git commit -m "docs: expand learning guide for absolute beginners"
git push origin HEAD
```

我可以代为执行这些 `git` 操作。

====================
第五部分：初学者常见问题与排错
====================

Q：构建失败怎么办？
- 把前 20 行错误信息复制给我，我会逐条帮你分析。

Q：不懂某个术语怎么办？
- 复制术语给我，我会用生活化语言解释并举例。

====================
第六部分：我可以继续帮你做的事
====================

选择下一步：
1. 我把 `LEARNING.md` 提交到新分支并创建 PR 草稿（我会为你完成 git 操作）。
2. 我继续把每个源码文件生成逐行注释版（例如 `chunk.rs` 的注释版）。
3. 暂不提交，我先等待你的反馈并按需修改教材。

请回复你要的选项编号或写出你的具体需求。

====================
第七部分：按源码文件逐步学习（源码驱动教学）
====================

目标：把每个关键源码文件作为一个小课堂，边读边练，最终能理解该文件在项目中的职责并做出小改动。

说明：每个小节都包含三部分：1) 用非专业语言讲这个文件做什么；2) 推荐顺序读的函数或代码段；3) 可动手的练习（从最简单的改动开始）。

- `src/main.rs` — 程序入口与启动流程
  - 做什么：程序从这里开始，创建窗口、加载插件或资源，启动主循环。
  - 重点阅读：`main` 函数；初始化插件/资源的代码段。
  - 练习：在 `main` 开头添加 `println!("启动 Ethertum：欢迎学习");`，保存并 `cargo run`，确认能看到输出。

- `src/voxel/vox.rs` — 单个体素（Vox）的定义
  - 做什么：定义一个小方块如何表示（包括材质 ID、形状、isovalue 等）。这是体素系统的最小单位。
  - 重点阅读：`struct Vox` 的字段和构造函数（`Vox::new`、`Vox::default`）。
  - 练习：找到 `Vox::default()`，在返回值处添加注释并在代码中打印一个默认 `Vox` 的字段值（临时 `println!`），编译并运行。

- `src/voxel/chunk.rs` — 区块（Chunk）数据结构与索引
  - 做什么：把 16×16×16 的格子放在一个 `Chunk` 里，提供访问本地坐标与邻居区块的函数。
  - 重点阅读顺序：`const LEN`/`LEN3` → `new` → `local_idx` / `local_idx_pos` → `at_voxel` / `at_voxel_mut` → 邻居相关 `NEIGHBOR_DIR` 和 `get_chunk_rel`。
  - 练习：编写一个小测试函数或临时 `main` 代码片段，创建 `Chunk::new(IVec3::ZERO)`，并用 `for_voxels` 统计非空格子数量，打印结果。

- `src/voxel/chunk_storage.rs` — 存盘/加载（稀疏保存）
  - 做什么：把内存里的 `Chunk` 转换为更小的 `StoredChunk`（只保存非空格子）并写入磁盘或 wasm 内存。
  - 重点阅读：`StoredCell` / `StoredChunk` 定义、`chunk_to_stored`、`stored_to_chunk`、`encode_chunk`、`decode_chunk`、以及 `FsWorldStorage::save_chunk`/`load_chunk`。
  - 练习：手动创建一个 `Chunk`，设置少量格子为非空，调用 `encode_chunk` 并把生成的字节长度打印出来，观察“稀疏”效果（比全部序列化小得多）。

- `src/voxel/worldgen.rs` — 程序化地形与种子（seed）
  - 做什么：用噪声函数（Perlin/Fbm）生成地形数据，`seed` 控制随机性，改变 `seed` 会生成不同地图。
  - 重点阅读：`generate_chunk_with_seed`（理解如何从坐标和噪声里计算 `val` 并决定 `VoxTex`）。
  - 练习：在函数开头把 `seed` 打印出来；或把 `seed` 固定为 `1`，构建并截图地图；再改为 `2` 比较差异。

- `src/voxel/voxel_server.rs` — 服务端区块加载、保存与分发
  - 做什么：决定何时从磁盘加载区块、何时生成、并把区块发送给在线玩家；还负责区块的卸载与保存。
  - 重点阅读：`chunks_load`（较长，分段阅读：世界切换 → 异步加载 → 完成加载 → 卸载 → 发送给玩家）。
  - 练习：在找不到 chunk 时生成新区块的分支插入日志（“从生成器生成”），在成功从磁盘加载时插入另一条日志，运行服务器看区别。

- `src/net/netproc_client.rs` — 客户端如何接收区块并建立本地数据
  - 做什么：接收网络包 `SPacket::ChunkNew`，把 `CellData` 转换回 `Chunk` 并插入客户端 `ChunkSystem`。
  - 重点阅读：匹配 `SPacket::ChunkNew` 的分支、`CellData::to_chunk` 的调用位置。
  - 练习：在接收分支中添加日志，记录接收到区块的 `chunkpos` 与来源（本地/远端），在客户端运行并查看日志。

- `src/client/game_client.rs` — 客户端如何设置 `ActiveWorld`（用户输入的种子）并启动连接
  - 做什么：把用户界面里的世界名与种子写入 `ActiveWorld` 资源，随后连接服务器或本地世界。
  - 重点阅读：`select_active_world`、`connect_local_world`、`enter_world`、`exit_world`。
  - 练习：修改 `select_active_world` 使其在设置 `ActiveWorld` 时额外写一条日志：`info!("Selected world {} seed {}", world_name, seed);`。

- `src/voxel/meshgen.rs` 和 `src/voxel/render.rs` — 从体素到渲染网格
  - 做什么：把体素转换为可渲染的三角形（mesh），并上传给渲染管线。
  - 重点阅读：面生成、法线计算与材质分配的函数（挑最小的几个函数来读）。
  - 练习：调整某个面生成的阈值或某种材质的纹理 ID，重构并观察渲染差异。

每个小节建议的学习流程（适用于零基础读者）：
1. 先看本节“做什么”的一句话总结。不要急着看全部代码。 
2. 在 VS Code 中打开文件，用“搜索”定位 `fn`（函数）或 `struct` 定义，从小处入手读函数签名与注释。 
3. 找到练习里的简单改动，先做改动、构建、运行，观察变化。 
4. 把你做的改动提交到一个新分支（哪怕只是注释或日志），这是学习如何协作的重要步骤。

进阶：如果你愿意，我可以为每个文件自动生成一个“注释版”——原始文件的副本，但在关键行上插入逐行说明（只作为阅读材料，不会改原始源码）。要我先生成哪一个文件的注释版？

====================
第八部分：进阶专题 — 把计算机学科知识与源码结合
====================

目标：把计算机组成原理、数据结构与算法、操作系统、网络通信、计算机图形学等专业知识，逐项映射到本项目的源码与实践练习，便于有志进入开发的读者系统学习并上手贡献。

每个专题包含：1) 概念要点（尽量以生活化比喻说明）；2) 本项目中的对应代码位置与阅读建议；3) 实验/练习（可在本仓库里动手验证）；4) 推荐深入资料。


- 计算机组成原理（CPU、内存、缓存、I/O）
  - 概念要点：CPU 是“计算引擎”，内存是临时存储，磁盘是持久化存储；缓存（cache）是快速的短期存储，用于提高访问速度；I/O 负责外设交互。
  - 在项目中的体现：
    - `Chunk` 在内存中占用大量空间（4096 个 `Vox`），频繁创建/销毁会带来内存分配与缓存影响，影响性能（见 `src/voxel/chunk.rs`）。
    - 存盘逻辑（`src/voxel/chunk_storage.rs`）体现了内存 ↔ 磁盘的数据移动成本：选择稀疏序列化减少 I/O 量。
  - 练习：
    1. 在本地运行程序并监测内存使用（使用系统任务管理器或 `top`），在不同加载距离下比较内存峰值。记录并解释变化。
    2. 修改 `Chunk` 创建频率（例如临时创建/摧毁模拟），观察分配时间与帧率影响（若能运行渲染）。
  - 深入阅读：数字逻辑与计算机体系结构教材（《Computer Organization and Design》）。

- 数据结构与算法（数组、哈希、空间索引、并行任务调度）
  - 概念要点：数据如何在内存中组织影响查找/更新的成本；哈希表适合快速查找，数组紧凑适合批量遍历；空间索引（四叉树、八叉树、网格）用于地图与碰撞。
  - 在项目中的体现：
    - 区块集合使用 `HashMap<IVec3, ChunkPtr>`（见 `ChunkSystem` 的实现），用于按坐标快速查找加载的区块。
    - `Chunk::local_idx` 将 3D 索引压缩为一维，用位运算实现高效索引。
    - 并发加载使用 Bevy 的任务池（AsyncComputeTaskPool）调度异步任务（见 `src/voxel/voxel_server.rs` 中的异步加载逻辑）。
  - 练习：
    1. 在小测试中比较遍历 4096 个元素的不同写法（索引循环 vs 迭代器），测量耗时差异。 
    2. 在 `ChunkSystem` 中用日志记录哈希表查找时间（插入计时点），理解散列冲突的可能影响。
  - 深入阅读：算法导论、并发任务调度与锁相关资料。

- 操作系统（进程/线程、并发、文件系统）
  - 概念要点：操作系统提供进程/线程模型、调度、文件系统接口与权限管理；并发程序须考虑共享资源与锁。
  - 在项目中的体现：
    - 服务器端使用异步任务池并通过频道（channel）在任务之间通信，体现了并发与同步（见 `src/voxel/voxel_server.rs` 中的 `ChannelTx/ChannelRx` 的使用）。
    - 文件读写（`fs::read` / `fs::write`）体现对文件系统的直接调用（见 `src/voxel/chunk_storage.rs`）。
  - 练习：
    1. 在 `chunks_load` 的并发加载处故意引入延时，观察并发加载队列长度、延迟及日志顺序，理解异步执行效果。 
    2. 模拟并发保存同一文件（在短时间内多线程调用 `save_chunk`），观察错误或锁竞争并记录结果（注意做好备份）。
  - 深入阅读：现代操作系统教材（如《Operating Systems: Three Easy Pieces》）。

- 网络通信（客户端/服务器模型、可靠/不可靠消息、序列化）
  - 概念要点：网络通信将数据拆成包发送；可靠通道保证送达并按序，非可靠通道不保证；序列化把内存数据转为字节流，反序列化恢复。
  - 在项目中的体现：
    - 使用 `Renet`/`renet`（或自定义协议）实现客户端/服务器数据包（`SPacket::ChunkNew`、`ChunkDel`）（见 `src/net/netproc_client.rs` 与 `src/voxel/voxel_server.rs`）。
    - `CellData::from_chunk` 提供把 `Chunk` 序列化为网络可传输格式的过程，接收端用 `CellData::to_chunk` 恢复。
  - 练习：
    1. 在本地启动服务端与两个客户端，测量在玩家移动触发区块发送时的网络带宽占用（可用抓包工具或日志统计字节数）。
    2. 修改序列化策略（例如压缩或移除某些字段），对比包大小与延迟。
  - 深入阅读：计算机网络基础与序列化框架文档（serde、bincode、protobuf）。

- 计算机图形学（网格生成、光照、材质）
  - 概念要点：体素到网格（mesh）需要从体素边界生成三角形；光照模型决定表面亮度；材质/纹理决定外观。
  - 在项目中的体现：
    - 网格生成逻辑在 `src/voxel/meshgen.rs`，包括把体素面转为顶点/索引、法线与 UV。 
    - 渲染相关资源与材质在 `src/voxel/render.rs` 与 `assets/shaders/` 下的 shader 文件。
  - 练习：
    1. 找到 `meshgen::put_face`（或类似函数），在渲染前修改材质 ID，观察运行时效果。 
    2. 在 shader 中临时改变漫反射颜色，重新编译并观察渲染变化。
  - 深入阅读：计算机图形学入门书籍与着色器语言（WGSL/GLSL）教程。

- 系统设计与架构（模块化、插件、资源管理）
  - 概念要点：大型项目通过模块化（plugin、resource）组织代码，明确定界接口与生命周期可降低耦合。
  - 在项目中的体现：
    - 使用 Bevy 引擎的插件/资源系统（`app.insert_resource`、`Plugin` 实现）来注册 voxel、网络、渲染等子系统。
    - `ActiveWorld`、`WorldSaveRequest` 等资源作为全局状态管理示例。
  - 练习：
    1. 尝试在 `main` 中动态禁用某个插件（例如某个可选模块），观察运行时差异。 
    2. 增加一个简单的资源（例如 `DebugMode(bool)`），让系统在开启时输出更多日志。


====================
进阶练习与学习路径建议
====================

1. 学习路径（3 个月示例）：
  - 第 1 个月：巩固 Rust 基础 + 阅读 `chunk.rs`、`vox.rs`、`chunk_storage.rs`，完成基础练习。 
  - 第 2 个月：学习并实践并发与操作系统要点，阅读 `voxel_server.rs` 并完成并发加载/保存的实验。 
  - 第 3 个月：学习网络与图形学，分别修改序列化与 meshgen，完成一个小 PR（例如可选的区块压缩）。

2. 项目级练习（目标：提交 PR）：
  - 任务 A（中级）：为 `ChunkStore` 增加可选压缩（gzip 或 zstd）并在配置中开启/关闭，比较磁盘占用与加载时间。 
  - 任务 B（进阶）：实现客户端对区块接收的增量更新（仅发送改变的 `StoredCell`），减少带宽。 
  - 任务 C（研究）：在 `meshgen` 中加入简单的 LOD（细节层次）支持，减少远处区块顶点数。

3. 我可以帮你：
  - 为其中任一练习生成详细步骤（包括需要修改的函数、测试方法与验证命令）。
  - 为 `ChunkStore` 压缩任务提供初始实现补丁（分支提交并创建 PR 草稿）。

请选择下一步：要我先为哪个进阶专题生成详细练习步骤或开始实现哪个任务（A / B / C）？

