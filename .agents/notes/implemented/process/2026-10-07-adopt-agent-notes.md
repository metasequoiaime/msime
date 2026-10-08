# Agent Note: 采用 write-notes-like-deepseek 决策笔记体系

Status: implemented

## Problem

仓库的跨会话约定只有 AGENTS.md 里的操作纪律（架构边界、发版流程、worktree 规矩），「某个决定为什么这样定、当时放弃了什么」没有固定去处。`docs/` 下的文档要么是平台对照清单，要么是像 `docs/notes/pinyin-fallback-ranking.md` 那样的叙事复盘——写得再好，新会话的 agent 也不知道去哪找、找什么：没有「路径即状态」的生命周期，没有机械校验拦住格式烂掉的笔记。多个 agent 并行干活时，同一权衡被反复重新推导，被否掉的老路会被重走。

## Decision

采用 `.agents/skills/write-notes-like-deepseek` 定义的笔记体系，树落在 `.agents/notes/{proposed,implemented,rejected,archived} × {feature,bug-fix,simplification,architecture,process,testing}`。本篇是第一篇 implemented 笔记。

- 校验脚本不复制进仓库 `scripts/`，npm script 直接引用 skill 目录内的脚本（skill 受 `skills-lock.json` 锁定，脚本路径稳定），并用 `node` 原生执行——仓库 `.nvmrc` 锁定 Node 24.21，类型剥离默认开启，无需 tsx，离线可跑。脚本从 CWD 解析 `.agents/notes`，所以 `pnpm run verify-notes` 必须在仓库根执行。
- 门禁三条线：`verify-agent-note-tree`（目录/分类/文件名/相对链接）、`verify-agent-note-format`（头块/必备节/备选方案必填/implemented 禁提案标题）、`verify-archived`（归档封印只增不改）。
- 门禁同时接入 `scripts/verify-local.sh`，成为 pre-push quick 的常驻阶段（node 缺失时跳过），`.agents/*` 归入无需平台阶段的路径类——笔记与 skill 脚本的改动由常驻的 agent-notes 阶段覆盖，不再触发全部编译阶段。

## Alternatives considered

- **沿用 `docs/notes/` 的叙事文档** — 最省事，且已有 pinyin-fallback-ranking 这样的好先例，可读性不差。但它没有位置即状态的生命周期，agent 检索全靠全文搜索碰运气，格式也没有任何机械约束，堆多了必然腐化。
- **引入编号 ADR（adr-tools，放 `docs/adr/`）** — 业界成熟，编号便于口头引用。但全局编号索引在多 worktree 并行开发下是每改必冲突的中心热点，同样没有格式门禁；这套体系用「目录即状态」换掉编号索引，与本仓库的 worktree 工作方式更契合。
- **把三条校验接进 CI workflow** — 门禁不靠自觉是最硬的形态。但本仓库是单人加 agent 开发，AGENTS.md 已规定提交前本地跑门禁，CI 只会重复同一道门；协作者变多时再接不迟。（触发条件当天即成立：PR #5251 评审要求不受本地配置约束的强制点，已由 [ci-gate 笔记](2026-10-07-agent-notes-ci-gate.md)接管。）

## Consequences

- **收益**：跨会话的决定有了确定的可检索去处，非平凡改动强制记录备选方案，归档笔记有 SHA-256 封印防篡改；新会话动手前按 SKILL 第 3 节四法检索历史，减少重复推导。
- **代价与已知上限**：非平凡改动多出一篇笔记的写作负担，判定不准时会过度留痕；校验行为随 skill 版本变化，升级 skill 后需重跑 `pnpm run verify-notes`。若笔记开始堆流水账，或长期只有寥寥几篇，说明判定门槛需要重访——过密与不记是同一个错误的两种样子。

## Verification

仓库根执行 `pnpm run verify-notes`，三条校验全绿；`scripts/verify-local.sh --quick` 的 agent notes 阶段跑同一组校验；AGENTS.md「决策笔记」一节约束所有后续非平凡改动。
