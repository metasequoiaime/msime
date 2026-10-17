# Agent Note: 运行时复用 QKV 前向缓冲

Status: implemented

## Problem

前两片已让 attention 归一化、输出投影和前馈阶段共用 scratch，但每层 `linear` 仍先申请独立 QKV 向量，再由 `split_qkv` 立即拆成 query、key 和 value。QKV 结果在拆分结束后没有消费者，仍可由前馈 scratch 的 `hidden` 缓冲承接。九键 16 ms 目标仍未达到，本片只处理这一段临时向量。

## Decision

固定上游 `chinese-ime-lm` 合并提交 `44318ef29da18b7eea33ffe297b79f0d65a7a07e`（上游 PR #71）。`run_prefix` 和非空 `run_tail_states` 先把 QKV 写入 `FeedForwardScratch::hidden`，`split_qkv` 或 `split_qkv_with_suffix` 完成拆分后，前馈阶段按原 4 倍宽形状覆盖同一缓冲。QKV 线性计算、拆分顺序、KV 所有权、前向输出、评分和缓存寿命保持不变，不跨按键持有工作区。

## Existing decisions

已检索 `qkv`、`split_qkv`、`FeedForwardScratch`、`run_prefix`、`run_tail_states` 及前馈、attention 投影和归一化 scratch 笔记。[attention 前置归一化 scratch](2026-10-17-runtime-layer-norm-scratch.md)继续持有 `normalized` 的生命周期；[attention 投影 scratch](2026-10-17-runtime-attention-projection-scratch.md)继续持有投影与前馈的覆盖顺序；[attention 双缓冲否决](../../rejected/testing/2026-10-16-runtime-attention-scratch.md)否决的是 attention 输出与 softmax 分数复用，不覆盖 QKV 拆分前的独立向量。本片没有翻转这些决定。

## Alternatives considered

- 保留独立 QKV 向量：改动最少，但 QKV 在拆分后立即失效，每层重复申请。
- 改写 `split_qkv` 直接把线性结果写入三份最终存储：可少一次中间读取，但会扩大拆分接口和借用边界；本片先保持拆分逻辑不变，只替换输入缓冲。
- 跨按键持有完整模型 scratch：可以减少更多申请，但会扩大 reranker 状态和持有内存；本片限定一次同步前向。

## Verification

上游新增回归测试逐位比较 QKV helper 与旧 `linear`，检查暖 `hidden` 缓冲地址保持不变，并加入六层 QKV 配对预算。12 种 Float/Quantized、192/768 宽、1/4/12 行形状全部通过；上游 Rust 单测、最低 Rust、Clippy、fmt、评测集和 CI 全部通过。主项目接入后，`env -u MSIME_EVAL_RESOURCES cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 通过引擎 2252、golden 31、runtime 318 项，忽略 40 项；目标 Clippy、全仓 fmt、差异检查和 `CI=true pnpm run verify-notes` 通过。推送前的 `verify-local.sh --quick` 作为本 PR 门禁，九键质量与端到端延迟仍按原口径记录，不能从局部配对结果宣称 16 ms 达标。

## Consequences

同一次前向每层少一个 QKV 临时向量的申请和释放；`hidden` 在 QKV 拆分后由前馈完整覆盖，当前形状不会读取旧元素。局部收益只属于实际模型前向路径，不改变量化权重、KV 存储、隐藏状态或公开行为，九键 16 ms 目标仍保持 active。
