# Agent Note: 跟踪平台 issue #7172

Status: rejected — issue #7172 已由 #7285 实现并关闭，无需单独跟踪

## Problem

issue #7172（macOS）希望输入法在中文与英文、数字之间自动加空格，免去每次手动补空格，参考 rime-ice 的 `cn_en_spacer.lua`。#7285 已于 2026-10-12 合并并关闭该 issue：新增默认关闭的共享偏好 `chinese_english_spacing`，在 `crates/host-api` 的上屏后处理里为单次上屏文本中汉字与 ASCII 字母、数字相邻处插入半角空格，六个平台的设置页都有开关。决定与已知限制见 [中英文之间加空格](../../implemented/feature/2026-10-12-chinese-english-spacing.md)。

## Proposal

为 #7172 另开一个 Draft PR 作为交付跟踪入口，补齐复现、责任路径和目标平台验收后再加入代码。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 #7172，并记录平台、复现和验证边界。
- 实现切片带代码、回归和平台验收。

## Risks

跟踪 PR 本身不交付功能；issue 关闭后仍挂着会成为过时提案，误导后来者以为功能尚未实现。
