# Agent Note: 九键消费切分原地更新

Status: implemented

## Problem

九键候选选择消费已经用掉的数字时，`consume` 会把剩余切分位置重新收集到新的 `Vec<usize>`。切分数量受 `DIGIT_LIMIT` 限制，旧向量的容量在会话中已经存在，却仍在每次消费路径上分配新的临时缓冲。

## Decision

使用 `Vec::retain_mut` 原地删除已消费的切分，并把保留位置减去消费长度。它保持切分的原有顺序和相对偏移，不改变锁定、光标或数字消费规则。

## Alternatives considered

- **继续使用迭代器收集**：实现简短，但每次消费都会产生新的切分向量。
- **手写索引压缩循环**：可以原地完成，但 `retain_mut` 已提供相同的修改与保留语义，减少重复边界代码。
- **为消费另设固定数组**：会增加会话状态的栈内存；现有切分向量已经拥有足够容量，直接复用更小。

## Verification

新增 `consuming_digits_reuses_split_storage` 分配回归测试：旧实现消费含两个切分的位置产生 1 次分配并失败，新实现保留 `[2]` 且为 0 次分配。Rust 标准库的 `Vec::retain_mut` 自 1.61 起支持原地访问、修改和筛选，顺序保持不变。九键测试 80 项通过，完整引擎 1561 项通过、13 项忽略，golden 31 项通过；Clippy、格式、差异、笔记和 quick 门禁通过。quick 中 Windows 本机 102 项通过、Linux 原生宿主 73 项通过；wasm、Windows 交叉构建和 HarmonyOS 真打包因工具链/资源缺失跳过。

## Consequences

九键选择候选并消费数字时不再为切分偏移建立临时堆缓冲；切分容量由会话已有的 `Vec` 保持，候选结果与后续撤销行为不变。
