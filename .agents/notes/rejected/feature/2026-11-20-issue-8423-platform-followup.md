# Agent Note: 跟踪平台 issue #8423

Status: rejected — issue #8423 已由 #8427 修复并关闭，无需单独跟踪

## Problem

issue #8423 反馈 macOS 端必须先按 `Shift+K` 进入快捷输入模式，快捷短语无法直接参与候选，希望去掉这个前置要求。该需求已由 #8427（`Fixes #8423`，2026-10-17 合并）实现：macOS 中文模式下普通输入编码会把匹配的快捷短语追加到普通候选中，`Shift+K` 独立入口继续保留，决定见 [macOS 快捷短语参与普通候选](../../implemented/feature/2026-10-17-macos-quick-phrase-candidates.md)；issue 随之以 completed 关闭。

## Proposal

为 #8423 建立独立 Draft PR 作为交付跟踪入口，补齐复现、版本与环境证据，再按确认的责任路径加入代码、回归和 macOS 平台验收。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- 跟踪 PR 关联 `#8423`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，加入代码、回归和 macOS 平台验收。

## Risks

跟踪 PR 本身不包含修复；在 #8427 已经交付的情况下继续保留，会让已完成的需求看起来仍未处理。
