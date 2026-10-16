# Agent Note: 运行时重排直接使用候选索引

Status: implemented

## Problem

上游 `chinese-ime-lm::Reranker::best_where` 每次九键重排都会先分配最多九个候选索引，再分配一个候选文本引用数组。两次小数组分配不参与模型前向，却处在每次按键的热路径上；同时评分结果必须仍然映射回引擎候选列表的原始索引。

## Decision

接入上游已合并的 PR #55（提交 `36db18fd6334f07ef261fa242dcdcdbd78cf0748`）。上游以固定 `[usize; 9]` 缓冲保存可比候选，评分入口按原始索引迭代，`best_where`、公开 `score` 和候选顺序保持不变。`crates/engine`、`crates/input-runtime`、主锁文件和 fuzz 锁文件统一固定这个提交。

## Existing decisions

已检索 `Reranker`、`best_where`、候选编码和缓存相关笔记。[候选编码所有权转移](2026-10-16-runtime-rerank-tail-ownership.md)只处理 token 缓冲复制，[完全命中缓存](../bug-fix/2026-10-15-runtime-rerank-complete-cache.md)、[同批候选缓存](../bug-fix/2026-10-15-runtime-same-batch-cache.md)和[末位置按需前向](../bug-fix/2026-10-15-runtime-defer-terminal-forward.md)继续约束模型状态生命周期；本片只接入按索引评分的上游优化，不改变这些契约。上游的实现取舍记录在[chinese-ime-lm 的对应笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/main/.agents/notes/implemented/testing/2026-10-16-rerank-selected-indices.md)。

## Alternatives considered

- 继续固定旧提交：不会引入依赖变更，但每次重排仍保留与模型前向无关的两个临时 `Vec`。
- 在 `msime` 宿主复制候选或重写评分：无法消除上游分配，且会复制评分语义和缓存边界。
- 等待更大的模型前向重构：可能带来更大收益，但不能替代这项独立、可验证的热路径减法。

## Verification

上游 PR 已通过 25 个 Rust 单元测试、14 个模型文件测试、全目标 Clippy（`-D warnings`）、Rustfmt 和差异检查；新增测试覆盖固定存储以及跳过非可比候选后仍返回原候选索引。`msime` 侧更新四处固定来源后，`msime-engine` 与 `msime-input-runtime` 测试分别通过 2152 个单元、31 个 golden、294 个 runtime 测试（另有 28 个明确忽略的测试），两 crate 全目标 Clippy、全仓 Rustfmt、`verify-notes` 和 `scripts/verify-local.sh --quick` 均通过。为清除 Rust 1.97 新报出的既有 `clippy::byte-char-slices` 基线警告，另以独立机械提交把测试中的字节数组改为字节串迭代。

## Consequences

每次运行时重排少两个最多九项的临时堆数组，候选筛选、门控、分数和原始索引映射不变。固定缓冲增加少量栈空间，模型前向仍是主要成本；九键 p95 的 16 ms 目标必须在本次依赖更新后用固定资源重新测量，不能由这项局部优化单独宣称达标。
