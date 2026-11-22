# Agent Note: 跟踪平台 issue #7405

Status: rejected — issue #7405 已由 #7556 修复并于 2026-10-13 关闭，无需单独跟踪

## Problem

issue #7405（iOS，QQ 群反馈）反映 iOS 键盘的候选区和工具栏占地方，打字时可视区域被压缩。#7556（`feat(ios): 读音并入候选行，顶栏只占一行`，正文 `Fixes #7405`）把读音并入候选行、顶栏最低高度从 50pt 降到 42pt，合并后 issue 以 completed 关闭；取舍记在 [iOS 读音并入候选行](../../implemented/feature/2026-10-13-ios-inline-reading-top-row.md)。

## Proposal

为 #7405 建一个独立的 Draft PR 作为交付跟踪入口，补齐复现、责任路径、代码回归和 iOS 验收后再合并。

## Alternatives considered

- **直接复制相邻平台修复**：可能更快覆盖相似症状，但没有本 issue 的独立复现和验证，容易把无关改动带进 iOS，当时因此没有采用。
- **只在 issue 下追加调查评论**：不用开分支，但形成不了可审查的交付边界，当时因此没有采用。

## Acceptance criteria

- 跟踪 PR 关联 #7405，并记录平台、复现和验证边界。

## Risks

跟踪 PR 本身不含修复；issue 已经关闭时它只会作为过时的 Draft 留在列表里。
