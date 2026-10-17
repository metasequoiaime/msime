# Agent Note: 运行时复用 query scratch 缓冲

Status: implemented

## Problem

每层 QKV 线性结果拆分时，`split_qkv_with_suffix` 都为 query 申请独立的 `Vec<f32>`。query 只在 attention 完成前有效，随后会被 attention 投影写入的 `FeedForwardScratch::normalized` 覆盖；独立向量因此在同一次前向中短暂存在并重复申请。九键 16 ms 目标仍未达到，本片只处理 query 的这段临时存储。

## Decision

固定上游 `chinese-ime-lm` 合并提交 `c690cec2c5fb1a64f4ca7d0b447d57e4249fe538`（上游 PR #72）。`run_prefix` 和非空 `run_tail_states` 让 QKV 拆分 helper 把 query 写入已有的 `FeedForwardScratch::normalized`，再以该切片调用 attention；K/V 仍按原有前缀、终末行和新行顺序生成并拥有自己的最终存储。attention 返回后，投影继续覆盖 `normalized`，残差、前馈、隐藏状态、评分和缓存寿命保持不变。

## Existing decisions

已检索 `split_qkv`、`split_qkv_with_suffix`、`FeedForwardScratch`、`run_prefix`、`run_tail_states` 及已有的 QKV、attention 投影、attention 归一化 scratch 笔记。[运行时复用 QKV 前向缓冲](2026-10-17-runtime-qkv-scratch.md)继续规定 QKV 结果在拆分后由 `hidden` 承接；[运行时复用 attention 投影前向缓冲](2026-10-17-runtime-attention-projection-scratch.md)继续规定投影在 `normalized` 中覆盖；[运行时复用 attention 前置归一化缓冲](2026-10-17-runtime-layer-norm-scratch.md)继续规定每层归一化与后续覆盖的生命周期。本片只把 QKV 拆分产生的 query 接入这条已有覆盖链，没有翻转这些决定。

## Alternatives considered

- 保留独立 query `Vec`：改动最少，但 query 每层申请后立即失效，无法利用已有 `normalized` 容量。
- 让拆分 helper 同时把 Q、K、V 都写入外部三份存储：可以统一接口，但会扩大别名与所有权边界；本片只让生命周期明确短于 attention 的 query 写入 scratch，K/V 继续由 helper 返回最终拥有者。
- 跨按键持有完整模型 scratch：还能减少更多申请，但会扩大 reranker 状态和持有内存；本片限定一次同步前向。

## Verification

上游新增回归测试逐位比较旧拆分结果与 scratch 写入结果，并检查暖 `normalized` 缓冲地址保持不变；新增六层 query 拆分配对预算覆盖 Float/Quantized、192/768 宽、1/4/12 行共 12 种形状，交替 40 对采样全部低于门槛。主项目接入后，`env -u MSIME_EVAL_RESOURCES cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 通过 Engine 2252、golden 31、runtime 318 项，忽略 40 项；目标 Clippy、全仓 fmt、差异检查、`CI=true pnpm run verify-notes` 和 `bash scripts/verify-local.sh --quick` 均通过。quick 阶段按环境跳过 Docker、MinGW 与 HarmonyOS 专用工具链检查。

## Consequences

同一次模型前向每层少一个 query 临时向量的申请和释放；`normalized` 在 QKV 拆分、attention 投影和前馈之间顺序覆盖，调用结束后不保留新增状态。K/V 的容量和所有权不变，量化与浮点路径的逐位输出不变。局部收益只属于模型前向路径，不能换算成九键 16 ms 达标。
