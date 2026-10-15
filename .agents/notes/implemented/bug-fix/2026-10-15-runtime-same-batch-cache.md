# Agent Note: 接入同批候选前缀复用

Status: implemented

## Problem

九键逐键 16 ms 预算仍未通过。上游评分只搜索上一轮缓存，当前批次已经得到的前缀和完整候选仍重复前向；冷批次合成回归实际前向 11 个位置，只需 5 个。

## Decision

接入已合并的上游 [PR #52](https://github.com/metasequoiaime/chinese-ime-lm/pull/52)，两个消费 crate 与主、fuzz 锁统一固定 `0f19789abbad5aa887e3b27088bf168acf6e9098`。保留上一轮完全命中和等长部分匹配的优先级；新批次可复用更长的共同前缀。候选预算、质量基线、模型资源、门控和宿主接口不变。

## Existing decisions

已检索同批、候选缓存和末位置笔记。[末位置按需前向](../../implemented/bug-fix/2026-10-15-runtime-defer-terminal-forward.md)、[归一化复用](../../implemented/bug-fix/2026-10-15-runtime-rerank-normalization-reuse.md)、[完全命中缓存](../../implemented/bug-fix/2026-10-15-runtime-rerank-complete-cache.md)部分重叠，继续约束失效和生命周期；本篇补充本轮已算状态的复用，没有废弃的同批提案。

## Alternatives considered

- 仅依赖跨按键缓存：不增加本批线性搜索，但候选前缀首次出现、上下文失效时仍重复计算。
- 宿主先合并候选或减少模型评分条数：能省计算，但改变引擎候选契约与排序覆盖；共享因果状态由模型实现维护。

## Upstream verification

旧合成计数回归前向 11 个位置（预期 5）而失败；新实现前向 5、投影 4，重复候选共享完成状态，同长匹配保留旧轮最后项。Rust 1.97.1 与 MSRV 1.75.0 各 32 项测试、Clippy、fmt、笔记检查通过，保留三精度与 Send/Sync。

固定真实模型，141 条全拼和 81 条九键，五轮共 1110 轮、10646 个评分，1014 个位模式改变，最大绝对误差 1.4305115e-6（预设小于 2e-6），冷、热首选保持。完整列表先过滤再截九条的官方 replay：全拼词典 9/9、解码器 104/131；九键 7/7、42/74，旧新一致。前九条再过滤的离线池九键 43/74，两种池不可混用。

同进程交替顺序 40 对/形状：16 种首 token 互异的独立候选 new/old 中位比 0.9963–1.0038；12 种共享多候选 0.1201–0.5016，全部通过预设 5% 收益。完整 bench、配对全表和测量限制见上游同提交笔记 `.agents/notes/implemented/bug-fix/2026-10-15-same-batch-cache.md`。本地复核缓存查找、生命周期、末位置补算及锁文件，未启动子代理。

## Host verification

固定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`，模型 SHA256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。接入前后各自 release 构建 `convert_eval`；全拼组合（sentences-v1 与 sentences-nine-key-v1）141 条、九键 81 条完整 JSON 逐字段一致：全拼 top1/top9 为 0.801/0.830，九键为 0.617/0.728。没有更新质量基线。

九键 `rerank_latency --set resources/eval/sentences-nine-key-v1.tsv --nine-key`，81 条、每臂 1791 键、丢弃 5 条预热。固定旧 pin 的二进制先保存，再构建新 pin；以下顺序运行，本任务没有同时运行构建、测试或其它 benchmark。

| 顺序 | 引擎 mean / p95 ms | 模型臂 mean / p95 ms | 模型增量 mean / p95 ms | 超 16 ms 键 |
| --- | --- | --- | --- | --- |
| 旧版 | 5.53 / 10.93 | 9.70 / 25.07 | 4.16 / 16.48 | 402 / 1791 |
| 新版 | 6.30 / 12.25 | 9.63 / 23.77 | 3.34 / 13.81 | 323 / 1791 |
| 新版复测 | 6.44 / 12.65 | 9.46 / 22.90 | 3.02 / 13.04 | 352 / 1791 |
| 旧版复测 | 9.47 / 27.27 | 16.37 / 51.50 | 6.90 / 37.52 | 645 / 1791 |

四轮均退出 1，16 ms 未通过。末轮旧版引擎本身 p95 已达 27.27 ms；新版也有调度尖峰，不能把独立整轮差值全归因于优化。因主仓库并行开发，旧二进制宿主基线是 `a70d90be1f`，新二进制在 `026b76c168` 上接 pin；期间仅合入 iOS 换季主题与个人词库锁释放，没有引擎或评分变化，仍保留此来源限制。因果收益以同一上游基线的配对和确定性计数证明，不承诺这里的端到端比值。

`MSIME_NEURAL_MODEL_DIR=<固定资源> cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast`：引擎 2109 项、golden 31 项、runtime 286 项通过，20 项既有忽略。模型资源变量明确设置，旧真实词库夹具仍依其既有环境要求执行或跳过。全目标 Clippy、工作区 fmt 和差异检查通过；fuzz 全量 `cargo metadata --locked` 只解析到一个固定新提交的模型来源，NOTICE 检查通过（80 crates）。

本地 `CARGO_TARGET_DIR=<主工作区>/target CARGO_BUILD_JOBS=6 MSIME_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 退出 0。Windows 本机 149 项测试、HarmonyOS 设置页 bundle、仓库契约、Rust 工作区、Android Rust/Java、WASM、Linux 设置外壳、Linux 原生宿主及 87 项测试、macOS 原生宿主和 Apple bridge 构建通过。MinGW/x86/pipe-only、HarmonyOS ArkTS 缺工具或暂存输入的阶段按既有规则跳过；macOS Tauri 外壳因缺独立 app 资源未参与工作区检查。未绕过门禁。

## Consequences

本批增加少量线性查找，省去共同因果位置前向和归一化。模型只改变复用来源，更长切点改变 f32 分组，因此不声称逐位不变。候选顺序、预算、门控、模型资源和缓存寿命保持。主项目质量报告保持，九键 16 ms 仍失败，后续继续定位剩余重排和引擎开销。本项验证属于进程内评分及构建测试，未进行原生设备按键验收。
