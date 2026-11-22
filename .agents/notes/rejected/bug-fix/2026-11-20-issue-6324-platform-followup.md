# Agent Note: 跟踪平台 issue #6324

Status: rejected — issue #6324 已由 #6366 处理并关闭，无需单独跟踪

## Problem

issue #6324 报告 Linux 版 0.11.0 打开「悬浮工具栏」后桌面上看不到浮动条。Linux 的 IBus 与 Fcitx5 前端本来就不画悬浮窗，工具栏分别是 IBus 面板属性菜单与 Fcitx5 托盘状态区菜单里的「工具栏」子菜单（GNOME 下的 IBus 没有这个子菜单），是设置页沿用悬浮条预览和「在桌面显示悬浮工具栏」文案误导了用户。#6366 把 Linux 工具栏设置页改成按输入法菜单说明，issue 随之于 2026-10-09 以 completed 关闭，决策见 [Linux 工具栏设置页](../../implemented/bug-fix/2026-10-09-linux-toolbar-settings-menu-copy.md)。

## Proposal

为 #6324 单独开一个 Draft PR 作为跟踪入口，补齐复现、责任路径、回归测试和 Linux 平台验收后再交付。

## Alternatives considered

- **只在 issue 下追加调查评论**：不需要分支，但不形成可审查的交付边界，当时因此未采用。

## Acceptance criteria

- PR 关联 #6324，记录该 issue 的平台、复现和验证边界。
- 责任路径确认后加入代码、回归和 Linux 平台验收。

## Risks

单独跟踪会与已合并的 #6366 重复，留下一个长期悬空的提案。
