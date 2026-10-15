# Agent Note: 宿主句子重排复用共享状态的概率归一化

Status: implemented

## Problem

完全命中的运行结果已共享，但多条候选从同一隐藏状态分叉时，仍各自投影整张词表、求相同的 log-sum-exp。九键逐键重排的模型臂 p95 仍超过 16 ms；只消除完全命中不能覆盖实际变化的候选。

## Decision

接入已合并的上游 [PR #48](https://github.com/metasequoiaime/chinese-ime-lm/pull/48)，固定提交 `f4d41421f8633d8899cdb8ed6d6a3207c5e980f7`，复用隐藏状态的按需归一化。每个 `Arc<HiddenState>` 保存隐藏向量和一个 `OnceLock<f32>`；首次仍按原顺序计算完整投影和归一化常数，之后只算目标 token 的 logit。旧状态分叉、缩短和恢复时共享同一常数；上下文改变仍建立新状态。两个 crate 和主/fuzz 锁固定同一上游合并提交。模型、候选预算、比较集合、静态证据与门控保持既有契约。

## Existing decisions

检索活跃笔记中的 `Reranker`、共享前缀与归一化：[完全命中缓存](../../implemented/bug-fix/2026-10-15-runtime-rerank-complete-cache.md)部分重叠，继续持有上一轮完整状态，本篇补充分叉状态复用；[静态评分缓冲](../../implemented/testing/2026-10-15-rerank-static-score-buffer.md)、[神经排序缓冲](../../implemented/testing/2026-10-14-neural-rerank-score-buffers.md)属于模型外的缓冲管理；[九键句子设置](../../implemented/feature/2026-10-09-nine-key-sentence-options.md)归引擎内模型配置；其它命中属于词库查询、模糊路径或非重排归一化，均无关。没有完全吸收或过时提案。

末位置的完整前向由[候选末位置按需前向](2026-10-15-runtime-defer-terminal-forward.md)接入延迟计算；本篇的累计分、共享和缓存失效规则继续保留。

[同批候选前缀复用](2026-10-15-runtime-same-batch-cache.md)补充当前批次已算状态的复用，原有完整命中、归一化共享和失效规则继续成立。

## Alternatives considered

- 缓存每状态完整词表分布：可直接读取任意 token，但内存随词表大小乘位置数增长，键盘扩展不承担这项成本。
- 仅缓存已提交上下文：实现更小，但无法消除候选部分续跑的重复投影。
- 缩小候选集或跳过模型：减少推理最直接，但改变排序覆盖面和既有门控，不能作为这项等价优化。

## Upstream verification

上游 Rust 1.97.1 与 MSRV 1.75 各 28 项测试通过，全目标 Clippy、fmt、diff 和笔记结构检查通过。投影计数回归在旧实现为 4 次（预期 3）而失败，修复后共享上文 3 次、两条同位置续跑 1 次、再用已知位置 0 次、换上下文重算。非均匀合成权重覆盖完整分布位模式、越界 token clamp 与末位置不提前投影；三精度回归保留。

固定旧提交 `2bd674a9972ee44c87c7b06e87a90efe35095316` 和同一真实模型：141 条全拼组合、81 条九键，五轮缩短、恢复、重复、倒序、恢复，共 1110 轮、9980 个评分逐位一致，冷评分与公开完整分布逐位一致。全拼词典/解码器桶 9/9、104/131，九键 7/7、43/74，旧新一致。

完整冷、完全命中热、末字变化热及逐字输入微基准表保存在上游 PR 和同提交笔记。机器其它任务造成明显 p95 抖动，同进程交替旧新顺序的末字变化请求测 32 种形状、每形状 80 对，逐对 new/old 耗时比中位数 0.627–0.648；这只是共享归一化路径的收益，不代表宿主达标。

## Host verification

固定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`、模型 SHA256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。release 构建的 `convert_eval` 全拼组合（`sentences-v1` 与 `sentences-nine-key-v1`，141 条）和九键（81 条）完整 JSON 与旧版逐字段一致：全拼 top1 0.801、top9 0.830；九键 top1 0.617、top9 0.728。没有改质量基线。

`rerank_latency --set resources/eval/sentences-nine-key-v1.tsv --nine-key` 每臂 1791 键，丢弃 5 条预热。旧版二进制从更新依赖前的基线复制，模型 pin 为 `2bd674a…`；新版 pin 为 `f4d4142…`。两版均无采样且不与本任务编译、测试或其它 benchmark 并行。

| 运行 | 引擎 mean / p95 ms | 模型臂 mean / p95 ms | 模型增量 mean / p95 ms | 超 16 ms 键 |
| --- | --- | --- | --- | --- |
| 旧版初轮（负载抖动） | 10.13 / 36.43 | 20.28 / 78.67 | 10.15 / 57.25 | 762 / 1791 |
| 新版 | 6.06 / 12.08 | 11.41 / 28.53 | 5.35 / 19.17 | 592 / 1791 |
| 随后重测旧版 | 5.93 / 12.02 | 12.01 / 29.87 | 6.08 / 20.44 | 651 / 1791 |

旧版初轮连引擎臂也显著升高，因此不拿它与新版计算收益。相邻的新、旧重测引擎臂接近，模型臂 p95 约降低 4.5%，mean 约降低 5%；单机两轮仍受负载影响，不能据此保证所有平台的改善幅度。三轮都退出 1，16 ms 预算仍未通过，没有放宽门禁。

网页引擎 NOTICE `--check` 通过（80 crates），依赖版本仍为 0.2.0，许可证与 NOTICE 无需变化；主/fuzz 锁只替换模型来源提交，fuzz 的 locked metadata 检查通过。

`MSIME_NEURAL_MODEL_DIR=<锁定资源> cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast`：引擎 2106 项通过、19 项既有忽略，golden 31 项、runtime 284 项与文档测试通过。使用模型专用资源变量让键盘模型对照实际执行；历史真实词库夹具未提供匹配 `dict-v2.0.1`，按既有逻辑跳过，没有修改夹具。全目标 Clippy、fmt、diff 与笔记检查通过。

本地 `CARGO_TARGET_DIR=<主工作区>/target MSIME_BUILD_JOBS=6 bash scripts/verify-local.sh --quick` 退出 0：Windows 当前宿主可运行的 149 项测试通过（2 项宿主差异排除、64 项需要 Windows 构建），Rust 工作区、Android 目标和 Java 宿主检查、WASM、Linux 设置外壳、Linux 原生宿主及 87 项测试、macOS 原生宿主与 Apple bridge 编译通过。Windows MinGW/x86/pipe-only 缺环境，HarmonyOS 设置 bundle 缺前端依赖、ArkTS 缺工具链及暂存资源，按既有门禁跳过；没有做设备验收。未引入不完整的 node_modules 链接。

## Consequences

每隐藏状态增加 Arc 和 OnceLock，首次评分多算一次目标点积，换取分叉共享位置不再重复投影词表。只缓存一个常数，内存不按词表大小乘位置数增长。真实模型评分逐位一致，完整质量报告不变。九键运行时有小幅改善但仍超预算，后续继续定位其它推理开销；本项验证是进程内性能、编译和测试，不代表原生设备按键验收。
