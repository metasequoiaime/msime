# Agent Note: 跟踪平台 issue #9081

Status: rejected — issue #9081 已由 #9396 修复并关闭，无需单独跟踪

## Problem

issue #9081（iOS）希望在键盘上横滑直接移动文档光标，免去停下输入、长按文本拖动光标的步骤。PR #9396 新增默认关闭的「横滑键盘移动光标」开关，无组字时从普通字符键横滑即可移动光标，合入后以 `Fixes #9081` 关闭该 issue；决策见 [iOS 无组字时横滑键盘移动光标](../../implemented/feature/2026-10-23-ios-keyboard-cursor-swipe.md)。

## Proposal

为该 issue 建立独立 Draft PR 作为交付跟踪入口，补齐复现、责任路径、回归和 iOS 平台验收后再判断是否完成。

## Alternatives considered

- **直接复制相邻平台修复**：可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入 iOS，因此不采用。
- **只在 issue 下追加调查评论**：无需创建分支，但不能形成可审查的交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#9081`，记录平台、复现和验证边界。
- 实现、回归和 iOS 平台验收落在关联同一 issue 的 PR 中。

## Risks

跟踪 PR 本身不修复问题；若实现已在别处合入，它会作为过期提案长期悬挂并误导后续盘点。
