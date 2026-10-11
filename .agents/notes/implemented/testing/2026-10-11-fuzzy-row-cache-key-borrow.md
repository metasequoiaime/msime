# Agent Note: 全拼模糊行缓存借用查询键

Status: implemented

## Problem

全拼模糊候选会为每个前缀在 `fuzzy_row_cache` 中查行。即使行缓存已命中，旧实现也会先用 `format!` 拼出包含规则、上限和前缀读法的 `String` 键；不同整句输入共享前缀时，这个热路径会反复分配查询键。

## Decision

将模糊行缓存改为 `u64` 哈希索引，缓存值保留规则、路径上限和完整前缀读法，命中后核对全部字段。未命中或哈希碰撞时才查询数据库并保存一份拥有的前缀键；缓存行、查询上限、FIFO 淘汰和结果顺序保持不变。

## Alternatives considered

- **继续拼接 `String` 键**：实现简单，但每次跨查询的行缓存命中都会申请临时字符串。
- **只使用哈希不核对完整键**：命中路径更短，但哈希碰撞可能返回错误候选；保留完整键校验。
- **为每个前缀增加独立缓存**：可以避开复合键，但会改变现有行缓存的统一 FIFO 淘汰边界，增加缓存状态。

## Verification

回归测试先在生产代码没有 `CachedFuzzyRows`、哈希函数和键校验时按预期编译失败。实现后热命中测试确认不分配查询键，碰撞测试确认会重新查询而不返回哨兵行；全拼词典测试 45 项、引擎全量测试 1783 项（13 项忽略）、Clippy、Rustfmt、差异检查和笔记检查均通过。quick 门禁中的 Rust workspace、Android、Linux（79/79）、macOS 和 Apple bridge 通过，整体仅因 develop 已有的 Android 标签工厂、键盘试用 watcher、live status、ripple、sheet header 和 UI label factories 契约检查失败而返回 1。

## Consequences

跨查询共享前缀的模糊行缓存命中不再为查找键申请 Rust 堆存储；冷未命中仍需保存一份前缀键，哈希碰撞会重新查询。分配计数只覆盖 Rust 分配，不代表 SQLite C 堆或宿主延迟。
