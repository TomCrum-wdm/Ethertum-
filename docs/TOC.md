# 文档总表（TOC）

本文件为仓库学习材料的统一入口与优先级索引。建议按下列顺序阅读：

## 初学者路径（优先）
- `LEARNING.md` — 项目导向的零基础教材（主入口）
- `docs/learning/rust_basics.md` — Rust 基础（变量、所有权、借用、结构体、枚举）

## 源码注释（并行阅读）
- `docs/annotated/chunk_explained.md` — `src/voxel/chunk.rs` 注释
- `docs/annotated/chunk_storage_explained.md` — `src/voxel/chunk_storage.rs` 注释
- `docs/annotated/worldgen_explained.md` — `src/voxel/worldgen.rs` 注释
- `docs/annotated/voxel_server_explained.md` — `src/voxel/voxel_server.rs` 注释
- `docs/annotated/meshgen_explained.md` — `src/voxel/meshgen.rs` 注释
- `docs/annotated/netproc_client_explained.md` — `src/net/netproc_client.rs` 注释

## 示例与参考代码
- `assets/_refer_code/` — 示例/参考实现（在 `LEARNING.md` 中标注对应关系）

## 工程与构建说明
- `README.md` — 项目总体说明与快速启动
- `build/android_adaptation.md` — Android 平台注意事项

## 需补齐/草稿（当前状态）
- `LEARNING.md` 的若干章节为占位，需补充：FAQ、Credits、实践任务
- 若干注释文档为草稿或模板（见 `docs/annotated/*_template.md`）

## 我建议的下一步
1. 优先补齐 `docs/learning/rust_basics.md` 前 5 章（已计划）
2. 为注释文档添加练习与代码片段
3. 定期运行字数统计，追踪 ≥100,000 字目标

----
如果你要我现在开始某一步，请回复对应数字或回复“全部开始”。
