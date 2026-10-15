# Agent Note: 宿主句子重排复用完全命中的候选推理

Status: implemented

## Problem

九键锁定合成集 81 条、1791 键的 runtime 重排延迟初测 p95 29.96 ms，超过 16 ms 预算。上游 `Reranker::score` 已保存完整候选状态，却将共享前缀强制截到末字之前；候选不变时仍重算末字和完整词表概率。

## Decision

升级到已合并的上游 [PR #47](https://github.com/metasequoiaime/chinese-ime-lm/pull/47)，固定提交为 `2bd674a9972ee44c87c7b06e87a90efe35095316`，两个 crate 和主/fuzz 锁文件保持同一来源。旧 pin 与上游修复前 `main` 的 `reference/src` 完全相同，清单只多既有的 0.2.0 版本号变更。上游通过 `Arc<TailRun>` 复用完全相同的截断 token 状态，部分前缀保留原续跑；上下文 token 改变仍失效，只留上一轮缓存。比较集合、静态证据、门控、权重和资源不变。

## Existing decisions

检索活跃树中的 `Reranker`、共享前缀及缓存笔记：[九键句子设置](../../implemented/feature/2026-10-09-nine-key-sentence-options.md)部分相关，约束引擎内键盘模型，与本次宿主外部重排器分层不同；[独立句子静态证据](../../implemented/bug-fix/2026-10-14-rerank-static-evidence.md)部分重叠，评分与门控继续沿用；[重排静态评分栈缓冲](../../implemented/testing/2026-10-15-rerank-static-score-buffer.md)为输入缓冲决定，不覆盖模型状态复用。没有完全吸收或过时提案。

部分共享位置的归一化复用由[共享状态归一化](2026-10-15-runtime-rerank-normalization-reuse.md)接续；完全命中与上一轮缓存寿命继续遵循本篇。

末位置的完整前向由[候选末位置按需前向](2026-10-15-runtime-defer-terminal-forward.md)接入延迟计算；本篇的累计分、共享和缓存失效规则继续保留。

[同批候选前缀复用](2026-10-15-runtime-same-batch-cache.md)补充当前批次已算状态的复用，原有完整命中、归一化共享和失效规则继续成立。

## Alternatives considered

- 缩小候选集或跳过模型：可以直接减少推理，但会改变排序质量与既有门控，本次不采用。
- 在宿主另存文本到分数的跨轮缓存：可以绕开上游重复推理，但引入另一套上下文失效和容量策略，不能直接复用模型完整状态。
- 上游移动状态或深复制：前者会消耗同批其它候选所需的旧状态，后者保留 KV/隐藏状态复制成本；使用共享不可变状态。

## Verification

上游 Rust 1.97.1 与 MSRV 1.75 均通过 26 项测试。完整状态保留回归在旧实现因累计分指针变化真实失败，修复后通过；覆盖重复候选共享、清空后释放及 `Send + Sync`。三种精度对照包含部分续跑、上下文失效、空与超窗口截断。固定真实模型对照共 1110 轮、9980 分数，首选全部一致，最大绝对差 4.7683716e-7。词典/解码器两个桶：全拼 9/9 与 104/131，九键 7/7 与 43/74，旧新相同。完整冷热 bench 表保存在上游 PR 与同提交的决策笔记；完全不变的九候选热查询由约 4.3–4.6 ms 降到约 0.001–0.002 ms。

主项目用同一锁定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`、release 构建和未采样九键对照：81 条、1791 键/臂，丢弃五条预热用例。引擎臂 mean 5.94 → 6.01 ms、p95 11.74 → 12.09 ms；含模型 mean 11.97 → 12.11 ms、p95 29.82 → 30.32 ms；模型增量 p95 20.59 → 21.06 ms。超预算键 652 → 654，两轮均退出 1，不能声称九键达标或端到端提速。采样没有参与这轮测量；机器仍有其它任务，微小差值没有因果解释。

全拼模式的 141 条组合评测与九键模式 81 条评测，完整 JSON 报告逐字段完全一致：全拼 top1 0.801、top9 0.830；九键 top1 0.617、top9 0.728。没有更新质量基线或延迟预算。

`MSIME_NEURAL_MODEL_DIR=<同一锁定资源> cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast`：引擎 2106 项通过、19 项既有忽略；31 项 golden、284 项 runtime 和文档测试通过。Clippy 全目标、fmt 与差异检查通过。这里用模型专用变量让键盘模型对照实际执行；历史真实词库夹具未提供匹配资源，按既有逻辑跳过；桌面模型资源缺失的对照仍按既有行为跳过。最初把当前词库放进 `MSIME_EVAL_RESOURCES`，触发仅供 `dict-v2.0.1` 的历史资源夹具，产生日语词条数及四条真实词库 golden 失败；本次未修改这些夹具或其期望。当前资源的产品质量由上面的旧新完整报告对照验证。

本地 `verify-local.sh --quick` 最终退出 0：Rust 工作区、Android 目标与 Java 宿主检查、WASM、Linux 设置组件、Linux 原生宿主及 87 项测试、macOS 原生宿主和 Apple bridge 均通过。Windows MinGW/pipe-only、HarmonyOS ArkTS 与设置 bundle 缺工具链或完整前端依赖而跳过，未做设备验收。初轮临时链接主工作区根 `node_modules`，但未安装应用目录依赖，导致 HarmonyOS bundle 前置失败；移除这个不完整链接后整套 quick 重跑通过。笔记检查通过，使用 `pnpm --config.verify-deps-before-run=false run verify-notes` 避免 pnpm 在未安装依赖的 worktree 中自动安装；这三项笔记脚本只需要 Node 内建模块。

## Dependency metadata

`fuzz/Cargo.lock` 原先缺少当前工作区已声明的脚本运行时、Windows 宿主和部分密码学依赖，重解析固定来源时同步补全；主锁仅更新模型来源、版本及解析器为 `cssparser-macros` 选择的 `syn` 依赖边。没有添加产品依赖或改变模型文件。`cargo metadata --locked --manifest-path fuzz/Cargo.toml --no-deps` 通过。网页引擎 NOTICE 由既有脚本重生，反映上游 crate 的 0.2.0 版本，许可仍为 Apache-2.0。

## Consequences

完全命中省去末字推理、完整词表归一化与 KV/隐藏状态复制，新计算的状态多一次 Arc 分配和引用计数开销。完全复用与旧末字重算可能有浮点末位差，质量对照限制这个风险。当前九键端到端没有测得收益，16 ms 预算仍未通过；这项改动只移除有明确回归证明的重复工作，不改变推理覆盖面。原生构建验证属于编译与单测，不代表设备按键或界面验收。
