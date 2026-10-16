# Agent Note: 运行时全词表投影按精度遍历权重行

Status: implemented

## Problem

当前固定上游 `362bf56` 的九键二十秒采样中，`Matrix::dot_row` 有 2934 个栈顶样本，是最大单一热点。完整词表投影为同一隐藏状态逐行重复分派精度。九键 16 ms 预算仍未达到，采样扰动后的延迟不能用于验收。

## Decision

接入已合并的上游 [PR #61](https://github.com/metasequoiaime/chinese-ime-lm/pull/61)，四处统一固定 `cc1d3dd3e8a8cfe40ffeeca1fddd65f047b56d2e`。完整投影匹配权重精度一次，按行计算 logits，保留点积、逐行缩放、最高值和归一化次序。公开 API、通用 linear、权重表示和缓存寿命保持。

独立审查发现量化权重的文件 shape 首维可能与配置要求的运行时行数不同，但元素总数相同，旧加载器会误收。上游现在检查 tok/qkv/proj/fc/out 的 scale 数等于运行时输出行数，投影 zip 前也限定 scale 切片，避免静默缩短词表。

## Existing decisions

已检索活跃笔记中的 project、词表投影、行遍历和 scale 行数。[共享归一化接入](../../implemented/bug-fix/2026-10-15-runtime-rerank-normalization-reuse.md)继续规定状态投影寿命；[最终 KV 直接构造](../../implemented/testing/2026-10-16-runtime-rerank-packed-kv.md)只决定 KV 存储，与本片无重叠收益。上游保留通用矩阵行分派与行交错的负结果，本片只验证独立 project，不翻转那些决定；没有完全吸收或过时提案。

## Upstream verification

上游[完整决定与前后 bench 表](https://github.com/metasequoiaime/chinese-ime-lm/blob/cc1d3dd3e8a8cfe40ffeeca1fddd65f047b56d2e/.agents/notes/implemented/testing/2026-10-16-project-row-stream.md)保留测量约束与负结果。固定旧完整 project 对照 Float/Quantized、宽 1/7/8/9/192/768、词表 3/9/257，全部 logits 与 normalizer 位模式一致。每形状 40 对、每臂 50 次、交替先后，最终新版/旧版逐对中位比值如下：

| 精度 | 宽×词表 | 比值 | 预定上限 |
| --- | --- | --- | --- |
| int8 | 192×8192 | 0.9613 | 0.99 |
| float | 192×8192 | 0.9550 | 1.03 |
| int8 | 768×8192 | 0.9848 | 1.03 |
| float | 768×8192 | 0.9907 | 1.03 |

旧生产路径实际未通过主形状预算；量化行数回归也先在旧加载器失败，再在修复后通过。Rust 1.97.1 与 1.75.0 各 33 单测、15 模型文件测试与 doc tests 通过，1 项性能测试手动通过；全目标 Clippy、fmt、笔记和差异检查通过。独立审查发现的静默截断问题修复后复查无阻塞。

## Alternatives considered

- 在主项目复制投影实现：可独立修改，但会使权重读取、共享模型和数值契约分叉，继续由上游持有。
- 复用通用行遍历闭包：能统一 linear 和 project，但通用 linear 已有预算失败的负结果，本片只接入独立投影的已验证路径。
- 仅在投影处限定 scale 切片：改动更小，但畸形模型会延后到推理时报越界，加载阶段验证可以直接返回 Geometry 错误。

## Main verification

固定真实模型 SHA-256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`，以 `MSIME_NEURAL_MODEL_DIR` 指向模型目录，运行 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet`：2152 个引擎单测、31 个 golden、294 个 runtime 单测通过，28 个既有忽略项。初次误将只含评测所需资源的目录作为完整 `MSIME_EVAL_RESOURCES`，导致 5 个资源相关单测和 4 个真实词库 golden 失败；修正环境变量后整套重跑通过，没有更改或跳过测试来掩盖失败。

全目标 Clippy `-D warnings`、全仓 Rustfmt、笔记和差异检查通过。本地 quick 通过，覆盖 Rust 工作区、Android/WASM 目标、Android Java 宿主、macOS 原生宿主和共享 Apple bridge 编译，以及本机可执行的 150 项 Windows 原生测试。Docker、MinGW、HarmonyOS 完整构建前置缺失的阶段明确跳过，没有平台 GUI 验收。独立接入审查确认四处 pin 一致且没有额外依赖变更。

全拼 60 例与九键 81 例分别生成 `convert_eval` JSON，与上一片 `362bf56` 报告以 Python 载入后整体 `==`，所有字段一致（含分组、失败样本与来源桶）。全拼 top1/MRR 为 0.833/0.842，九键为 0.617/0.640；top1 来源 database/generated 分别 3/57、7/74，命中金标来源分别 2/49、7/55。未改质量基线。

## Runtime latency

所有本轮编译、测试和质量评测终态后，固定资源、release 二进制，按旧、新、新、旧串行运行九键 `rerank_latency`。旧臂二进制 SHA-256 `8eddf0f49942aeffcfb93a16f687d5db92d60404c6344f27c61ac90d15d88902` 测前测后保持，固定上游 `362bf56`；新臂主项目基线 `c49cb8b2be`，测量时两臂 engine、input-runtime、评测集和 workspace 清单无其他差异。每轮 81 例、每臂 1791 键，丢弃 5 个预热 case；四轮均因 16 ms 预算失败退出 1。

| 轮次 | engine mean/p95 ms | model mean/p95 ms | adds mean/p95 ms | model max ms | 超 16 ms |
| --- | --- | --- | --- | --- | --- |
| 旧 1 | 4.35 / 8.42 | 7.72 / 20.05 | 3.37 / 13.37 | 135.62 | 199 |
| 新 1 | 4.83 / 10.79 | 8.08 / 21.46 | 3.25 / 13.83 | 117.45 | 197 |
| 新 2 | 5.26 / 11.51 | 8.98 / 23.78 | 3.71 / 17.21 | 122.15 | 256 |
| 旧 2 | 4.17 / 8.16 | 7.31 / 18.82 | 3.14 / 12.49 | 40.61 | 176 |

新版两轮的模型臂更慢，同时纯引擎臂 p95 由旧版 8.16–8.42 ms 变为 10.79–11.51 ms。这组运行受明显波动影响，不能证明端到端提速，也不能单凭它排除回退；保留全部尖峰，不删除样本、不调整预算。

## Recheck on the current base

为复核首轮新版两臂变慢的具体风险，同步至 `204d3aa577` 后重新跑测试：2156 个引擎单测、31 个 golden、294 个 runtime 单测通过，29 个既有忽略项。全目标 Clippy、Rustfmt 和本地 quick 再次通过；重建后的全拼 60 例、九键 81 例质量 JSON 与首轮报告整体相等。

两臂均从这份最新主项目代码构建，仅切换四处固定 pin。旧版 release 二进制保存在规定 `target/release/examples/` 目录，随后恢复最终 pin 并构建新版；测前确认来源恢复，测前测后核对二进制哈希。旧/新 SHA-256 分别为 `15dd1ec308cdcb7e501159c57dd1c88a0603b2e359faca14fae2faefdec510a5` / `c3b7bbf3a098408f59855af557b79437001ae67f46273df36fe321f886a43f9d`。全部构建与质量评测终态后，保持相同资源、81 例、1791 键、5 个预热 case、16 ms 预算，再按旧、新、新、旧串行测量；没有删除任何首轮数据。

| 轮次 | engine mean/p95 ms | model mean/p95 ms | adds mean/p95 ms | model max ms | 超 16 ms |
| --- | --- | --- | --- | --- | --- |
| 旧 1 | 3.82 / 7.42 | 6.82 / 17.84 | 3.01 / 11.85 | 31.24 | 134 |
| 新 1 | 3.87 / 7.44 | 6.77 / 17.54 | 2.90 / 11.56 | 31.22 | 125 |
| 新 2 | 3.82 / 7.33 | 6.74 / 17.65 | 2.92 / 11.60 | 32.05 | 124 |
| 旧 2 | 3.90 / 7.68 | 7.02 / 18.65 | 3.12 / 12.45 | 56.46 | 149 |

复核中纯引擎臂更稳定，新版两轮模型 p95 为 17.54/17.65 ms，旧版为 17.84/18.65 ms；增量 p95 为 11.56/11.60 ms 对 11.85/12.45 ms。这一轮观察到小幅改善，未复现首轮的持续变慢，但四轮样本不能推断稳定端到端收益。全部仍因 16 ms 预算失败退出 1，目标未达到。收益的主要证据仍是完整投影的配对预算及逐位一致回归。

## Consequences

完整词表投影减少逐行精度分派，主形状局部配对约少 3.9% 耗时；代价是浮点与量化各有行循环。加载阶段也拒绝原本可能误收的量化行数不匹配文件。九键 16 ms 目标继续 active。

微基准布局可能不同于发布构建，局部收益不能推断端到端达标；十五字上游决策列无样本。依赖改动不涉及文件读写、权限或 Android 资源，因此本片验证集中在 Rust 与本地 quick，不宣称平台 GUI 验收。
