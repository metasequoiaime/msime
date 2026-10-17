# Agent Note: 二元分备忘未命中时复用一次哈希查找

Status: implemented

## Problem

词网格解码的 `BigramMemo::score` 在备忘未命中时先调用 `HashMap::get`，再调用 `HashMap::insert`。同一个 FNV 键因此在缓存查找和写入时各经过一次 `KeyHasher`；九键一次刷新会对多个切分重复走这条评分路径。缓存值只由键和当前 n 元表决定，未命中分数在插入前不需要第二次独立查询。

动手前检索了 `BigramMemo`、二元分缓存、`HashMap` 键构造和 n 元表查找笔记。[二元分行缓冲延后预留](2026-10-11-lattice-bigram-lazy-reserve.md)负责每次解码的行容器生命周期，[桶内键切片](../../implemented/testing/2026-10-15-ngram-bucket-key-slice.md)负责 `NgramTable` 的表内查找，[二元分前缀试验](../../rejected/testing/2026-10-15-lattice-bigram-prefix.md)记录了改变键构造的否决；它们都不改变备忘表项的单次哈希查找决定。

## Decision

`BigramMemo::score` 改用标准库 `HashMap::entry` 取得一次 `Occupied` 或 `Vacant` 视图。命中直接借用已有分数；未命中才查 `NgramTable` 并通过同一 `VacantEntry` 写入。达到固定容量时，仍只在未命中后清空并插入，满容量命中继续返回原缓存值。表地址变化仍先清空，键计算、分数、清空时机、缓存上限和公开解码结果保持不变。实现遵循 Rust [`HashMap::Entry`](https://doc.rust-lang.org/std/collections/hash_map/enum.Entry.html) 的占用/空缺条目契约。

## Alternatives considered

- **继续 `get` 后 `insert`**：代码最直观，但每个未命中键要重复计算哈希，缓存只减少表访问却保留了哈希开销。
- **先按容量清空，再用 `entry`**：能让所有路径只调用一次 `entry`，但满容量命中会被无谓清空并重新查表，改变现有缓存的生命周期行为；本实现只在空缺分支清空。
- **手写原始表或保存预计算哈希**：可以进一步控制探测，但会引入新的存储和安全边界；标准 `Entry` 已覆盖一次查找并保持现有 `HashMap` 类型。

## Consequences

非满缓存的每个二元分未命中从两次哈希降为一次，命中仍只需一次哈希；满容量未命中保留原先的清空后插入语义。n 元表查询、FNV 键、表切换失效、缓存容量和候选排序不变。新增的计数器仅在测试构建中统计 `KeyHasher` 调用，不进入产品二进制。

## Verification

TDD 先在旧实现运行新测试：先预热一个缓存键，再查询新键，`KeyHasher` 计数为 `2`，按一次哈希目标失败；改用 `Entry` 后计数为 `1` 并通过。`lattice::decode` 定向测试 57 项通过（9 项既有忽略），Rustfmt 与 `git diff --check` 通过。完整引擎、golden、runtime、Clippy、quick 和笔记门禁仍是合并前门禁；本片只把局部哈希次数减少作为证据，不把它外推为整机延迟预算达标。
