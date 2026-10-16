# Agent Note: 运行时普通重排续跑借用 KV

Status: implemented

## Problem

上游 `TailRun::resume` 为普通共享前缀逐层复制 KV，下一步前向入口再次复制相同可见数据。前一次复制只在同步评分中存在，是九键热续跑的重复分配与搬运。

## Decision

接入已合并的上游 [PR #59](https://github.com/metasequoiaime/chinese-ime-lm/pull/59)，固定提交为 `093f4a260cb1144efee6bb5a6f1b6c71dc8dea2b`。私有 `Prefix` 用 `Cow` 借用普通续跑的 KV，前向入口按 `length * n_embd` 限定可见部分后只复制一次；独立上文继续持有 KV，待处理末位置的中间拼接由[末位置两段借用](2026-10-17-runtime-terminal-prefix-view.md)补齐；普通借用与可见长度契约继续成立。两个 crate 清单、主锁与 fuzz 锁使用同一提交。

公开接口、权重、矩阵计算、归一化、候选顺序和缓存寿命保持。借用只存在于同步评分，不进入跨轮持久缓存。空前向和非空前向都必须截断可见位置，不能把旧缓冲中的未来位置带入新候选。

## Existing decisions

已检索活跃笔记中的 `KV`、`Prefix`、`TailRun`、共享前缀和借用。[末位置按需前向](../bug-fix/2026-10-15-runtime-defer-terminal-forward.md)、[完全命中缓存](../bug-fix/2026-10-15-runtime-rerank-complete-cache.md)与[归一化共享](../bug-fix/2026-10-15-runtime-rerank-normalization-reuse.md)部分相关，继续约束状态与评分；[候选外层状态缓冲](2026-10-16-runtime-rerank-resume-buffer.md)和[分数缓冲](2026-10-16-runtime-rerank-score-buffer.md)只处理容器容量。没有完全吸收的笔记或过时提案。上游取舍、红绿证据及完整基准表见[对应笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/093f4a260cb1144efee6bb5a6f1b6c71dc8dea2b/.agents/notes/implemented/testing/2026-10-16-resume-kv-borrow.md)。

## Alternatives considered

- 全部 KV 使用 `Arc`：可以统一共享持有，但需要同时改变追加和末位置拼接的所有权；当前同步借用已能消除重复复制。
- 新建平行前缀视图：保留原结构，但增加一组字段与评分入口；私有 `Cow` 复用已有结构。
- 保持复制：生命周期更简单，但同一次评分仍重复搬运只读前缀。

## Verification

上游地址测试先在旧实现因数组地址改变失败，借用后通过；移除可见长度截断时，拥有型前缀对照实际失败，恢复后通过。Rust 1.97.1 与 1.75.0 均通过 30 项单元测试、14 项模型文件测试与 doc tests，全目标 Clippy、Rustfmt 和笔记检查通过；独立审查未发现阻塞问题，并独立重跑完整测试通过。

固定模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`，上游完整 `bench` 每形状 40 次。九候选十二字只改末字的热续跑 p50 从 0.077 ms 到 0.034 ms；该局部两轮计时不等于主项目逐键收益。十五字列缺样本及新版冷计算离群值已在上游笔记明确保留。

主项目以固定资源运行 `MSIME_NEURAL_MODEL_DIR=… cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet`，2152 项引擎单测、31 项 golden、294 项 runtime 单测通过，另有 28 项既有忽略。两 crate 全目标 Clippy（`-D warnings`）、全仓 Rustfmt、笔记检查与差异检查通过。

固定资源的全拼 60 例与九键 81 例分别使用 `convert_eval` 生成前后报告，以 Python 载入 JSON 后整体 `==`，全部字段一致（含分组、失败样本与来源桶）。全拼 top1 为 0.833、MRR 为 0.842；九键 top1 为 0.617、MRR 为 0.640；top1 来源桶分别为 database/generated 3/57、7/74，命中金标来源桶分别为 2/49、7/55。未改质量基线。

最终依赖上的 `MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，包含 Rust 工作区、Android 与 WASM 目标、Android Java 宿主、macOS 原生宿主、共享 Apple bridge 编译和本机可执行的 149 项 Windows 原生测试。Linux 的 Docker、Windows 的 MinGW、HarmonyOS 完整构建前置缺失，相关阶段明确跳过；未做平台 GUI 验收。

编译与质量评测结束后，直接运行 release 二进制 `rerank_latency --resources … --set resources/eval/sentences-nine-key-v1.tsv --nine-key`。前后各 81 例、每臂 1791 键、丢弃 5 个预热 case：

| 路径 | 前 mean / p95（ms） | 后 mean / p95（ms） |
| --- | --- | --- |
| engine only | 4.20 / 8.19 | 4.12 / 8.32 |
| with model | 7.39 / 19.26 | 7.12 / 18.80 |
| reranker adds | 3.19 / 13.10 | 3.00 / 12.44 |

模型臂超过 16 ms 的按键从 180/1791 到 154/1791；新版仍以预算失败退出 1。前后各一轮且存在调度尖峰（模型臂 max 82.30 / 44.89 ms），不能把小差值解释为稳定收益。局部借用的收益由上游借用地址与热续跑基准支撑，端到端目标仍未通过；不调整预算。

## Consequences

每次普通前缀续跑少两份外层向量与每层两份 KV 行复制；内部新增拥有与借用分支及可见长度约束。末位置拼接仍有复制，模型前向仍是主要成本。本轮固定资源九键模型臂 p95 为 18.80 ms、增量 p95 为 12.44 ms，16 ms 目标继续保持 active；不能用局部分配减法宣称目标完成。
