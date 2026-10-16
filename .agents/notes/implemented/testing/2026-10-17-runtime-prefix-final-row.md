# Agent Note: 运行时接入上文末层单位置输出

Status: implemented

## Problem

九键按键仍超过 16 ms，实际采样仍以线性层和量化点积为主要模型热点。上游已删除构建上文缓存末层没有消费者的位置输出，需要固定合并提交接入，并分别验证数值、真实质量和主项目延迟。

## Decision

接入上游 PR #66 的正式合并提交 `03b3b5db8c6758f5489edd29bf7f5c40ae3bc0b7`，两个 crate 清单、主锁文件与 fuzz 锁文件保持同一个来源。上游只在独立上文最后一层计算末位置 attention 输出投影和前馈，保留完整各层 KV、全部 QKV、其它层和候选尾部路径。不增加新格式、缓存寿命、依赖或公开 API。

## Existing decisions

已检索上文、最后一层、末层、前向与 prefix。[前馈工作区接入](../../implemented/testing/2026-10-16-runtime-feed-forward-scratch.md)继续持有同次调用内缓冲复用；[候选末位置按需计算](../../implemented/bug-fix/2026-10-15-runtime-defer-terminal-forward.md)继续持有候选末 token 延迟规则。它们部分相关，本片仅接入上文末层无消费者输出的删除，没有完全吸收或过时提案。

## Upstream verification

上游 [PR #66](https://github.com/metasequoiaime/chinese-ime-lm/pull/66) 已合并至实际默认分支 `main`；[固定提交笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/03b3b5db8c6758f5489edd29bf7f5c40ae3bc0b7/.agents/notes/implemented/bug-fix/2026-10-17-prefix-final-row.md)保留完整方法、全部形状及整表。旧路径查询计数与完整性能门禁均真实失败；90 组合成几何的 KV、末隐藏状态、完整分布与续跑累计分逐位一致。宽 192、1/6 层、1/4/12/32 位置、Float/Quantized 共 16 形状，各 21 对，全部通过预设多位置 <0.98、单位置 <1.05 门槛，无构建重叠观察。固定真实六层模型 4–64 位置上文完整构建 new/old 中位 0.876–0.908，单位置 0.999942；全部 KV、隐藏状态、分布及可续跑位置累计分逐位相同。

Rust 1.97.1 与最低 1.75.0 各通过 37 单测、15 模型文件测试与 doc tests；Clippy/fmt 及两次独立只读审查通过。完整模型 benchmark 的旧版一轮无构建重叠观察，两轮新版均有构建干扰，全部表原样保留，不能作为稳定整体收益证据。局部收益只归于已无构建重叠的完整上文配对门禁，不能推导九键达标。

## Alternatives considered

- 主项目复制模型实现：可以就地调试，但会分叉数值和缓存契约，继续由上游持有计算。
- 依赖上游分支：接入早，但来源随时移动，继续固定已审查且已合并提交。
- 省略最后一层较早位置的 KV：能再少算，但候选后续计算仍依赖它们，必须完整保留。

## Main verification

主项目基线 `3cd324091788336d5fca76a0ce60c8105d078cdc`，新 pin `03b3b5db`。`env -u MSIME_EVAL_RESOURCES MSIME_NEURAL_MODEL_DIR=<固定模型目录> CARGO_BUILD_JOBS=4 cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 默认并行重跑通过：引擎 2235、golden 31、运行时 317 及 doc tests，失败 0，引擎忽略 37。

首轮引擎有一项既有读法筛选分配预算受干扰，单跑通过；受控两臂间缓存失效足以重现预算失败。顺带复用既有进程隔离器补齐两个读法预算入口，原矩阵、冻结体、状态/容量和预算逐字保留；隔离后同一注入仍由子进程原断言失败并传到父进程。诊断已撤回，完整重跑通过。证据和归因边界保留在[原隔离笔记](2026-10-17-nine-key-allocation-isolation.md)，没有声称捕获历史失败精确时序。

release `convert_eval` 按默认口径（不加 `--keyboard-neural`，运行时仍加载实际重排模型），全拼 `sentences-v1.tsv` 60 例和九键 `sentences-nine-key-v1.tsv --grid nine` 81 例各自与同基线旧 pin JSON 整体相等；分组、来源桶与失败样本均保持。全拼 top1/MRR 0.833/0.842，九键 0.617/0.640。两者使用同一固定模型，没有 settled 模型；质量基线不改。最初附加的可选键盘神经模式不属于此默认口径，发现开关不一致后已重新生成默认旧臂，未把不同口径的结果混作新旧对照。

全目标 `cargo clippy -p msime-engine -p msime-input-runtime --all-targets --locked -- -D warnings`、全仓 Rustfmt、笔记 tree/format/归档封印与 `git diff --check` 通过。独立只读审查核对四处 pin 和上游 PR 来源；追加审查确认测试包装器还原后原体逐字一致、隔离器未改且只注册一次、注入确已撤回。

本地 `env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 退出 0。覆盖仓库契约、actionlint、1179 篇笔记与 4 个归档封印、Rust workspace、Android 目标、WASM、Android Java、macOS 原生宿主和共享 Apple bridge。Windows 可移植测试 155 通过，2 项宿主差异排除、66 项需 Windows；不构成 Windows 产品验收。macOS bundle 未预先构建，因此 Rust workspace 阶段排除 `msime-desktop`；后续 macOS 原生宿主实际编译通过。Linux Docker、完整 Windows 交叉、HarmonyOS 真工程与 pipe-only 缺前置跳过。本片不修改生产文件访问或 Android 资源，未进行设备验收。

## Nine-key latency

所有本任务构建结束后，用接入后的 release `rerank_latency --resources <固定模型目录> --set resources/eval/sentences-nine-key-v1.tsv --grid nine --warmup 5` 跑完整集合，预算仍为 16 ms。81 例、每臂 1791 键、丢弃 5 例预热；结果如下，单位 ms。

| 测量臂 | mean | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: | ---: |
| engine only | 3.73 | 3.57 | 7.23 | 9.30 | 14.47 |
| with model | 6.76 | 5.28 | 17.83 | 22.62 | 31.42 |
| reranker adds | 3.04 | 0.32 | 12.28 | 17.39 | 24.83 |

137/1791 键（7.6%）超过 16 ms，程序因 p95 预算失败退出 1，没有运行错误。每秒观察构建进程共 20 次，构建观察数 0；这不保证系统完全空闲，也不排除采样间短进程。原始日志和观察记录为工作区外 `runtime-prefix-final-row-nine-latency.log`、`.monitor.json`。本片没有同轮交替测量旧 pin，不能把它与历史日志的差值当成稳定回归或收益；该集合通常只有 BOS 一行，结果符合上文局部优化不保证九键改善的范围。

## Consequences

收益主要在构建非空上文缓存；九键延迟集合没有提交上文，通常仅 BOS 一行，不能用上文配对收益推断按键达标。完整模型整表两轮新版均有外部构建干扰，不能用于稳定整体收益结论。四处 pin 与上游来源已独立核对，无无关锁文件变更。新增的两个测试子进程只影响测试启动成本，不修改生产缓存或性能预算。
