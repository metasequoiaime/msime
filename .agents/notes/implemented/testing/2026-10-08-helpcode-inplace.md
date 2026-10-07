# Agent Note: 单字辅助码重排复用候选缓冲

Status: implemented

## Problem

`reorder_candidates_with_single_helpcode` 为 `First|Both`、`Last` 和 `None` 三组分别创建 `Vec`，每次单字辅助码重排都要为临时分组缓冲分配堆内存。

## Decision

直接在输入候选 `Vec` 上做两轮稳定原地分组：第一轮把 `First|Both` 行移到前缀，第二轮在剩余区间把 `Last` 行移到其后；每次命中使用区间 `rotate_right(1)` 保持组内顺序，剩余行自然保留在末尾。

## Alternatives considered

继续为三组预留并合并 `Vec` 会保留每次重排的临时堆分配；使用不稳定交换会改变候选在组内的字典顺序。稳定原地旋转不需要额外缓冲，并保持既有输出顺序。

## Verification

新增候选缓冲零分配回归测试；辅助码相关测试 39 项、引擎完整单元测试 1186 项、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖实现。

## Consequences

单字辅助码重排复用调用方传入的候选容量，不再创建三组临时 `Vec`；每个候选最多参与两轮匹配，原有优先级和稳定顺序保持不变。
