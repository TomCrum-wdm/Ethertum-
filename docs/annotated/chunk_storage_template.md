<!-- Template: chunk_storage 注释草稿 -->
# chunk_storage_explained — `src/voxel/chunk_storage.rs` 注释草稿

目的：解释仓库中 `chunk` 的磁盘/序列化格式、`StoredChunk` 与 `StoredCell` 的设计，以及 `FsWorldStorage` / `WasmWorldStorage` 的实现要点。

主要内容结构：

- 文件目的与高层设计说明
- 关键类型与常量（结构体、枚举、序列化格式）
- 重要函数详解（`chunk_to_stored`、`stored_to_chunk`、`encode_chunk`、`decode_chunk`）
- 存储后端说明（Fs vs Wasm）
- 示例调用（保存/加载流程）
- 练习题（3 个难度分级，含答案提示）

TODO: 从 `src/voxel/chunk_storage.rs` 提取代码段并填充逐段解释。
