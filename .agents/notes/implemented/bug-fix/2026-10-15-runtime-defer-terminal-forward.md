# Agent Note: 接入候选末位置按需前向

Status: implemented

## Problem

九键逐键预算仍超 16 ms。量化点积占主要热点，循环分派改写没有稳定收益；上游确认候选末 token 的概率由前一状态预测，旧实现仍提前计算末 token 的完整 Transformer，末字变化时这些状态直接丢弃。

## Decision

接入已合并的上游 [PR #51](https://github.com/metasequoiaime/chinese-ime-lm/pull/51)，固定 `4c213a872c2afecf20b49aca8131e30537c39b55`。普通评分只前向 length-1 个位置，真正续跑经过末 token 时通过 OnceLock 补算并共享末位置的一行 KV、隐藏状态与归一化。完整累计分、共享前缀长度、累加分组、完全命中和上一轮失效规则保留。两个 crate、主与 fuzz 锁统一固定来源。

## Existing decisions

已检索末位置、模型缓存、归一化和量化热点笔记。[运行时归一化复用](../../implemented/bug-fix/2026-10-15-runtime-rerank-normalization-reuse.md)与[完全命中缓存](../../implemented/bug-fix/2026-10-15-runtime-rerank-complete-cache.md)部分重叠，继续约束共享及寿命；本项只推迟无需用于当前概率的末位置前向。没有需要废弃的宿主提案。

同批已算前缀由[本轮候选复用](2026-10-15-runtime-same-batch-cache.md)补充，原有末位置按需计算和失效规则继续成立。

## Alternatives considered

- 保持提前计算：续跑更简单，延长时没有补算开销，但末字修改与候选消失时的完整前向无用。
- 丢弃末状态并缩短最大复用前缀：改动较少，但多个候选延长相同旧候选会重复前向与归一化，上游已用共享计数回归排除。
- 缩候选、跳模型或放宽预算：可以降低测量耗时，但改变排序覆盖或验收目标，本次等价优化继续保留它们。

## Upstream verification

上游 Rust 1.97.1 与 1.75.0 各 30 项测试通过，Clippy、fmt、笔记检查通过。旧前向计数为 6（预期 4）而失败；新实现改末字和缩短无需前向，多个延长共享一次补算，三精度序列与 Send/Sync 保持。

固定真实模型：141 条全拼、81 条九键，经五轮缩短/恢复/重复/倒序/恢复，共 1110 轮、11976 个缓存评分逐位一致，冷评分、完整公开分布与门控首选保持；全拼词典/解码器桶 9/9、104/131，九键 7/7、43/74。上游完整 bench 表与配对 64 种形状保存在同提交笔记。32 种改末字 new/old 中位比值 0.0030–0.0480；32 种延长 0.3483–1.0157，重复候选可共享补算，不推广其较大收益。

## Host verification

固定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`，模型 SHA256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。更新依赖前后各自 release 构建的 `convert_eval`，全拼组合 141 条、九键 81 条完整 JSON 逐字段一致，全拼 top1/top9 为 0.801/0.830，九键 0.617/0.728。未修改质量基线。

`rerank_latency --set resources/eval/sentences-nine-key-v1.tsv --nine-key`，每臂 1791 键，丢弃 5 条预热。先保存固定旧 pin 的二进制，再构建新 pin；四轮顺序运行，没有同时运行本任务编译、测试或其它 benchmark。

| 运行顺序 | 引擎 mean / p95 ms | 模型臂 mean / p95 ms | 模型增量 mean / p95 ms | 超 16 ms 键 |
| --- | --- | --- | --- | --- |
| 旧版 | 6.06 / 12.07 | 11.41 / 28.84 | 5.35 / 19.24 | 584 / 1791 |
| 新版 | 6.56 / 12.73 | 11.12 / 27.77 | 4.56 / 17.61 | 472 / 1791 |
| 新版复测 | 7.56 / 16.67 | 12.58 / 32.08 | 5.03 / 21.31 | 595 / 1791 |
| 旧版复测 | 5.90 / 11.82 | 11.18 / 28.71 | 5.28 / 19.56 | 558 / 1791 |

四轮均退出 1，16 ms 预算未通过。新实现两轮的引擎臂负载更高、存在调度尖峰，不能用四轮独立运行的端到端差值承诺稳定收益；局部收益来自上游同进程配对与确定性前向位置计数。本项不放宽门禁，后续继续定位剩余矩阵与重排开销。

主/fuzz 锁只更新模型来源提交，fuzz 全量 `cargo metadata --locked` 通过。网页引擎 NOTICE 检查通过（80 crates），依赖版本、许可证和权重资源没有变化。

`MSIME_NEURAL_MODEL_DIR=<固定资源> cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast`：引擎 2109 项、golden 31 项、runtime 286 项通过，20 项既有忽略；文档测试通过。模型专用资源变量让键盘模型对照实际执行，旧真实词库夹具仍按既有环境要求跳过。全目标 Clippy、工作区 fmt、笔记和差异检查通过。

本地 `CARGO_TARGET_DIR=<主工作区>/target CARGO_BUILD_JOBS=6 MSIME_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 最终退出 0。Windows 本机 149 项测试通过（2 项宿主差异排除、64 项需 Windows 构建），Rust 工作区、Android Rust/Java、WASM、Linux 设置外壳、Linux 原生宿主及 87 项测试、macOS 原生宿主和 Apple bridge 通过。首轮 Linux 阶段曾因 Docker daemon 断开报 EOF，API 恢复后独立重试原生宿主通过，再完整重跑 quick 通过；没有改脚本或绕过门禁。HarmonyOS 设置页 bundle 通过；缺 MinGW/x86 构建环境、HarmonyOS ArkTS 工具链与暂存资源的阶段按既有逻辑跳过；macOS Tauri 外壳缺原生 app 资源未参与工作区编译。未进行设备按键验收。

## Consequences

延长时新增少量按需缓存与临时 KV 克隆，保留上游延长全表及小幅最坏中位成本。模型只改变推理调度，完整质量报告保持；宿主 16 ms 预算仍失败。验证属于进程内评分、编译与测试，不声称已完成原生设备按键验收。
