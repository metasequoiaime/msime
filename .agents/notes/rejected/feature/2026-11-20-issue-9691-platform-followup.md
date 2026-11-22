# Agent Note: 跟踪平台 issue #9691

Status: rejected — issue #9691 已由 #9721 实现并以 completed 关闭，无需另建跟踪 PR；实现决定见[五笔 14 键笔记](../../implemented/feature/2026-10-25-wubi-fourteen-key.md)

## Problem

平台 issue `#9691`（ios）的标题是「[Feature]: iOS下五笔支持14键布局」，反映 iOS 上 26 键五笔误触较多，希望提供 14 键布局。本提议写成时 issue 已经关闭，提议里「仓库中没有 `Fixes` 或 `Refs` PR 关联它」的判断有误：PR #9721 正文写有 `Fixes #9691`，在 iOS、Android、HarmonyOS 三端为五笔加入 14 键布局，于 2026-10-25 合并，issue 随之以 completed 关闭。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#9691`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
