# Agent Note: 运行时尾部前向直接构造最终 KV

Status: implemented

## Problem

上游尾部前向每层先拆分出短 K、V，再复制可见前缀并追加短数组。中间数组只供复制，追加的几何扩容还使短尾部持有多余 KV 容量。

## Decision

接入上游已合并的 [PR #60](https://github.com/metasequoiaime/chinese-ime-lm/pull/60)，固定提交 `362bf56dff68b47976ce929cd45512380db0b8cd`。内部 QKV 拆分直接构造最终大小的 KV，普通尾部先按可见长度截断旧前缀；独立上文传空前缀。两个 crate 清单、主锁与 fuzz 锁统一固定此来源。

公开 API、权重、浮点计算次序、候选顺序、缓存寿命和末位置缓存规则保持。每层非空尾部删除两份中间 K、V 数组并避免追加扩容；不改变矩阵和 attention 的计算。

## Existing decisions

已检索活跃笔记的 `split_qkv`、KV 容量和拆分。[普通续跑借用](2026-10-16-runtime-rerank-kv-borrow.md)继续持有可见长度约束，[末位置按需前向](../bug-fix/2026-10-15-runtime-defer-terminal-forward.md)继续规定末位置仅持有新增一行 KV；容器缓冲与归一化仍属独立机制。本片不翻转这些决定，没有完全吸收或过时提案。上游[完整笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/362bf56dff68b47976ce929cd45512380db0b8cd/.agents/notes/implemented/testing/2026-10-16-packed-tail-kv.md)保留前后完整 bench 表与配对微基准。

## Alternatives considered

- 仅预留最终 KV 容量：改动更小，能消除追加扩容，但保留短 K、V 的临时数组和复制。
- 跨轮持久前向工作区：能复用更多临时存储，但需要改变共享模型的可变状态与同步调用边界；本片先删除确定冗余的中间复制。
- 交错 QKV 直接送入 attention：能免拆分，但改变 attention 索引契约，超出当前存储优化。

## Verification

上游容量回归旧实现实际失败（需 24 元素、保留 40），新版通过。直接拆分对照按位覆盖空与非空前缀、零行、多行及负零；既有续跑对照覆盖可见截断和末位置。Rust 1.97.1 / 1.75.0 各 32 单测、14 模型文件测试与 doc tests、全目标 Clippy、fmt、笔记检查通过；独立只读审查无阻塞问题。

固定模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。同进程 helper 配对测试覆盖 36 种形状、每形状 40 对、每臂 500 次、交换先后并先验证输出，非空前缀新版/旧版 p50 为 0.286–0.494。真实模型每形状 40 次的完整表中，九候选十二字冷计算 p50 为 45.32/46.55 ms，热续跑均为 0.032 ms；各一轮的完整模型计时未证明端到端提速，十五字决策列无样本。初次与编译重叠的计时整轮弃用。

主项目以固定真实模型运行 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet`，2152 个引擎单测、31 个 golden 与 294 个 runtime 单测通过，另有 28 个既有忽略项；全目标 Clippy（`-D warnings`）、全仓 Rustfmt、笔记和差异检查通过。

全拼 60 例与九键 81 例分别生成 `convert_eval` JSON，与上一片 `093f4a2` 的报告载入后整体 `==`，全部字段一致。两次报告之间主项目的 engine、input-runtime、评测集与 workspace 清单没有其它改动。全拼 top1/MRR 为 0.833/0.842，九键为 0.617/0.640；top1 来源 database/generated 分别 3/57、7/74，金标命中来源分别 2/49、7/55，失败样本和分组均保持。

最终依赖的本地 quick 通过，包含 Rust 工作区、Android 与 WASM 目标、Android Java 宿主、macOS 原生宿主与共享 Apple bridge 编译，以及本机可执行的 150 项 Windows 原生测试。Linux Docker、Windows MinGW 和 HarmonyOS 完整构建前置缺失，相应阶段明确跳过；未做平台 GUI 验收。

## Runtime latency

为排查上游单次完整表的冷计算差异，在所有本轮编译及质量评测终态后，用固定资源按旧、新、新、旧串行运行主项目 `rerank_latency` release 二进制。旧版为 `b13b337d53`、上游 `093f4a2`；新版的主项目基线虽多三条 Windows 宿主提交，但 engine、input-runtime、评测集与 workspace 清单没有其它改动。每轮 81 例、每臂 1791 键，丢弃 5 个预热 case：

| 轮次 | engine mean/p95 ms | model mean/p95 ms | adds mean/p95 ms | model max ms | 超 16 ms |
| --- | --- | --- | --- | --- | --- |
| 旧 1 | 3.98 / 7.64 | 7.04 / 18.18 | 3.06 / 11.99 | 35.35 | 156 |
| 新 1 | 3.89 / 7.55 | 7.02 / 18.80 | 3.13 / 12.73 | 33.46 | 160 |
| 新 2 | 4.05 / 8.03 | 7.19 / 18.95 | 3.13 / 12.34 | 85.97 | 159 |
| 旧 2 | 4.10 / 7.97 | 7.20 / 19.06 | 3.10 / 12.51 | 32.63 | 163 |

四轮均因预算失败退出 1。新版 p95 落在旧版两轮变化范围内，没有稳定端到端提速的证据，也未复现上游单次表所提示的持续回退；这组小样本不能排除更小差异。保留新 2 的调度尖峰，不删除离群值、不调整预算。收益由 KV 构造配对微基准及容量回归支撑，16 ms 目标仍未达到。

## Consequences

尾部每层少两个短 K、V 分配，最终 KV 不因几何扩容持有多余空间；helper 增加两个前缀切片参数。本片九键模型臂两轮 p95 为 18.80/18.95 ms、增量为 12.73/12.34 ms；16 ms 目标继续 active，不能用局部 KV 构造收益宣称目标达成。
