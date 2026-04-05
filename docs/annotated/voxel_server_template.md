<!-- Template: voxel_server 注释草稿 -->
# voxel_server_explained — `src/voxel/voxel_server.rs` 注释草稿

目的：解释服务端如何加载/生成/保存 chunk、异步任务管线（task pool、channel）、以及网络广播（ChunkNew/ChunkDel）。

主要内容结构：
- 系统概述（`chunks_load` 的生命周期）
- 数据流：player -> chunk request -> async generate/load -> spawn chunk entity -> save/unload
- 关键函数/系统的逐段注释
- 并发模型与性能提示（避免重复生成、任务去重）
- 练习题（改为延迟加载、模拟多个玩家请求）

TODO: 从源码提取代码并填充详细注释。
