# Agent Note: 运行时复用 attention 投影前向缓冲

Status: implemented

## Problem

每个 `run_prefix` 和非空 `run_tail_states` 的模型层都把 attention 输出投影写入新的 `Vec<f32>`，随后立即交给同一层的前馈路径。已有前馈 scratch 的 `normalized` 缓冲在投影完成后可以承接这段数据，但旧路径仍申请并释放独立投影向量。九键 16 ms 目标仍未达到，本片只验证这段局部申请是否可以安全删除。

## Decision

固定上游 `chinese-ime-lm` 合并提交 `eb3c7a6ff3cab1867583e89371847da11edc3355`（上游 PR #69）。`run_prefix` 和非空 `run_tail_states` 通过私有写入 helper 把 attention 投影直接写入已有 `FeedForwardScratch::normalized`，残差相加后继续沿用原前馈顺序。每次调用仍只持有局部 scratch，不跨按键保存，不改变权重、KV、隐藏状态、评分、公开 API 或数值运算顺序。

## Existing decisions

已检索 `attention`、`projected`、`FeedForwardScratch`、`run_prefix`、`run_tail_states` 及活跃前馈/KV 笔记。[同一次前向复用前馈缓冲](2026-10-16-runtime-feed-forward-scratch.md)继续规定局部缓冲的寿命和两份存储；[最终 KV 直接构造](2026-10-16-runtime-rerank-packed-kv.md)继续规定 KV 的所有权；[attention 双缓冲否决](../../rejected/testing/2026-10-16-runtime-attention-scratch.md)否决的是 attention 输出与 softmax 分数跨层复用，未覆盖本片的投影向量复用。本片没有翻转这些决定。

## Alternatives considered

- 保留独立投影 `Vec`：代码最直接，但每层都重复申请一个随后立即被前馈覆盖的同形状向量。
- 跨按键持有完整模型工作区：可以继续减少申请，但会扩大 reranker 状态和持有内存，并改变直接评分入口的同步边界；本片只复用一次前向内的 scratch。
- 复用 attention 输出或 softmax 分数：可能节省更多空间，但会改变 attention 的生命周期和别名边界，且已有配对实验在带上文形状未通过；本片只处理投影结果。

## Verification

上游新增回归测试逐位比较投影 helper 与旧 `linear` 结果，并检查暖 `normalized` 缓冲地址保持不变。上游参考实现的 Rust 单测、最低 Rust 版本、Clippy、fmt、评测集和 CI 全部通过；六层前馈配对预算也通过。主项目固定版本后，`env -u MSIME_EVAL_RESOURCES cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 通过引擎 2252、golden 31、runtime 317 项，忽略 40 项；目标 Clippy、全仓 fmt、差异检查和 `CI=true pnpm run verify-notes` 通过。`env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，覆盖 Rust workspace、Android/WASM、Android Java、macOS 原生宿主和共享 Apple bridge；Docker、MinGW、HarmonyOS 等缺少前置的阶段按脚本跳过。九键质量与延迟结果保持原口径记录。

## Consequences

同一次模型前向每层少一个 attention 投影临时向量的申请和释放，`normalized` 的容量在投影和前馈之间复用。投影结果仍在残差相加前完整写入，前馈随后覆盖同一缓冲；调用结束后不保留新增状态。收益只属于有模型前向的局部路径，不能换算成九键 16 ms 达标，也不改变此前端到端 p95 超预算的事实。
