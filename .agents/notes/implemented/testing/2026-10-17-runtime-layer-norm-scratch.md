# Agent Note: 运行时复用 attention 前置归一化缓冲

Status: implemented

## Problem

上一片已让 attention 输出投影和前馈阶段复用同一个 `FeedForwardScratch::normalized`。每层 attention 开始前的 `ln1` 和调用结束时的最终归一化仍分别申请临时 `Vec<f32>`，这些结果的生命周期也不与 KV 或隐藏状态重叠。九键 16 ms 目标仍未达到，本片只处理同一次前向内的归一化缓冲复用。

## Decision

固定上游 `chinese-ime-lm` 合并提交 `c0206beca56519e306e2706fbab0a0f4f4be9607`（上游 PR #70）。`run_prefix` 和非空 `run_tail_states` 用同一份局部 `normalized` 依次承接 attention 前置 `ln1`、attention 投影、前馈归一化和最终归一化；每次写入覆盖完整当前形状。前向输出、KV、隐藏状态、评分、公开 API、缓存寿命和数值运算顺序保持不变，不跨按键持有工作区。

## Existing decisions

已检索 `layer_norm`、`layer_norm_into`、`FeedForwardScratch`、`run_prefix`、`run_tail_states` 及前两片运行时笔记。[attention 投影 scratch](2026-10-17-runtime-attention-projection-scratch.md)继续持有投影和前馈阶段的复用顺序；[同一次前向复用前馈缓冲](2026-10-16-runtime-feed-forward-scratch.md)继续规定两份局部缓冲和不跨调用的寿命；[attention 双缓冲否决](../../rejected/testing/2026-10-16-runtime-attention-scratch.md)否决的是 attention 输出与 softmax 分数复用，不覆盖本片的归一化向量。本片没有翻转这些决定。

## Alternatives considered

- 保留独立 `layer_norm` 输出：实现最直接，但每层归一化结果在 `linear` 消费后立即失效，重复申请可以删除。
- 跨按键持有模型工作区：可继续减少申请，但会改变 reranker 状态和内存边界；本片只复用同步调用内的 scratch。
- 复用 attention 输出或 softmax 分数：生命周期和别名范围更复杂，已有带上文形状的配对实验未通过；本片只使用可证明不重叠的归一化结果。

## Verification

上游新增回归测试逐位比较归一化 helper 与旧 `layer_norm`，检查暖缓冲地址保持不变，并加入六层 attention 前置归一化配对预算。12 种 Float/Quantized、192/768 宽、1/4/12 行形状全部通过；上游 Rust 单测、最低 Rust、Clippy、fmt、评测集和 CI 全部通过。主项目接入后，`env -u MSIME_EVAL_RESOURCES cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 通过引擎 2252、golden 31、runtime 318 项，忽略 40 项；目标 Clippy、全仓 fmt、差异检查和 `CI=true pnpm run verify-notes` 通过。`env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，覆盖 Rust workspace、Android/WASM、Android Java、macOS 原生宿主和共享 Apple bridge；Docker、MinGW、HarmonyOS 等缺少前置的阶段按脚本跳过。九键质量与端到端延迟仍按原口径记录，不能从局部配对结果宣称 16 ms 达标。

## Consequences

每层前向少一次 attention 前置归一化临时向量，最终归一化也复用已有容量；写入型 helper 保证当前形状全部覆盖，避免读取旧元素。局部前向减少申请和释放，不改变模型权重、KV 所有权、缓存寿命或公开行为。收益只适用于实际执行模型前向的路径，九键 16 ms 目标仍保持 active。
