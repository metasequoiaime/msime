# Agent Note: 运行时重排决策分数缓冲复用

Status: implemented

## Problem

上游 `chinese-ime-lm::Reranker::best_where` 每次按键都为最多九个 `f32` 分数申请一个短 `Vec`。这些元素只用于本轮候选提升判定，外层容量却随调用反复申请和释放。

## Decision

接入上游已合并的 [PR #57](https://github.com/metasequoiaime/chinese-ime-lm/pull/57) 及容量修正 [PR #58](https://github.com/metasequoiaime/chinese-ime-lm/pull/58)，固定提交为 `42bad7174929fb2f3aece665e1fdbe7f30c667fc`。`Reranker` 通过 `scores_scratch` 跨调用复用 `best_where` 的结果存储；公开的 `log_probabilities` 继续返回独立拥有的向量。评分顺序、浮点计算、候选原始索引和提升判定保持不变。`crates/engine`、`crates/input-runtime`、主锁文件与 fuzz 锁文件统一固定同一来源。

## Existing decisions

检索活跃笔记中的 `Reranker`、`best_where`、分数缓冲和重排延迟后，[静态证据评分栈缓冲](2026-10-15-rerank-static-score-buffer.md)属于调用方输入存储，[候选索引固定缓冲](2026-10-16-runtime-rerank-selected-indices.md)属于筛选索引，[候选状态跨轮复用](2026-10-16-runtime-rerank-resume-buffer.md)属于 `resume` 状态，均为部分相关、继续保留的独立决定。未发现完全吸收的笔记或过时提案。上游实现的取舍见[对应笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/42bad7174929fb2f3aece665e1fdbe7f30c667fc/.agents/notes/implemented/testing/2026-10-16-rerank-score-buffer.md)。

## Alternatives considered

- 保留每次申请结果向量：实现简单且没有额外对象状态，但短分数存储仍在每次按键重复申请。
- 固定九项栈数组：可以完全免除该分配，但公开分数查询仍需拥有型向量；上游选择复用既有写入路径，避免维护两套结果写入逻辑。
- 等待矩阵点积优化后一起接入：可能有更大的端到端收益，但会扩大单次审查范围；本轮单独合并可验证的容器复用。

## Verification

上游最终提交通过 28 个单元测试、14 个模型文件测试、doc tests、全目标 Clippy（`-D warnings`）和 Rustfmt。新增 `best_where_reuses_the_score_buffer_capacity` 核对连续调用的分数缓冲地址；既有评分与排序测试继续覆盖结果契约。增长列表测试先在旧预留实现失败（九项候选保留十六项容量），改为相对清空后长度精确预留后通过。

主项目在最终依赖提交上运行 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet`，2152 个引擎单测、31 个 golden 与 294 个 runtime 单测通过，另有 28 个既有忽略项。两 crate 全目标 Clippy（`-D warnings`）、全仓 Rustfmt、笔记检查与差异检查通过。

最终依赖版本的 `MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，包含 Rust 工作区、Android 与 WASM 目标、Android Java 宿主、macOS 原生宿主和共享 Apple bridge 编译，以及本机可执行的 149 项 Windows 原生测试。Docker、MinGW 和 HarmonyOS 完整构建前置缺失，对应阶段明确跳过；本轮未做平台 GUI 验收。

## Consequences

连续重排少一次短分数向量分配，`Reranker` 相应保留最多九项分数的容量；公开分数结果的所有权和模型前向均不改变。本轮前的固定资源九键基线是模型臂 p95 18.83 ms、重排增量 p95 12.38 ms，采样主要成本仍是矩阵点积。本轮未重测端到端延迟，不能把局部复用换算为端到端收益，九键 16 ms 目标继续保持 active。
