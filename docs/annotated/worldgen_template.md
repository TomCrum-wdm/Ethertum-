<!-- Template: worldgen 注释草稿 -->
# worldgen_explained — `src/voxel/worldgen.rs` 注释草稿

目的：讲解世界生成器如何使用噪声/种子生成地形，`generate_chunk_with_seed` 的输入输出以及随机性的可重复性。

主要内容结构：
- 文件目的与高层流程图
- 关键函数（噪声初始化、海拔/地表判定、方块类型映射）
- 种子如何进入生成过程（哪些函数接收 seed）
- 示例：给定 seed 生成 chunk 的步骤
- 练习题与扩展：更改噪声参数、添加新的地物类型等

TODO: 从源码提取代码片段并逐段注释。
