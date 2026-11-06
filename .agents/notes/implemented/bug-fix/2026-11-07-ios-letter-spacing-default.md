# Agent Note: iOS 26 键排距回到用户设置值

Status: implemented

## Problem

上一版把 26 键竖屏的排距固定成用户设置值加 4pt。改后截图仍显示出明显的行间空白，实际反馈说明这份额外对齐间距也过大。

## Decision

26 键竖屏直接使用 `touch_row_spacing_tenths` 的用户设置值，默认排距从 11pt 收回到 7pt。44pt 键帽继续保留，52pt 基准键区省下的高度按排数放到键区上下：四排各 16pt，带顶部数字行的五排各 20pt。键盘外框高度保持不变。

这篇取代 [iOS 26 键排距保持紧凑](2026-10-18-ios-letter-spacing-compactness.md) 中“用户设置值加 4pt”的排距决定；[iOS 26 键排距](2026-11-06-ios-key-spacing.md) 记录了本次 PR 的首轮实现，当前代码以本篇为准。

## Alternatives considered

- 保留 11pt 排距：实现简单且能沿用上一轮约束，但改后模拟器截图仍然显示行间空白过大，放弃。
- 恢复 52pt 键帽：能填满键区，但 26 键字母键会重新变得又高又窄，放弃。

## Consequences

- 默认 26 键的三条行间空白减少 4pt，键帽高度和键盘外框不变。
- 上下补偿留白增加，键区面板和按键底边继续按实际补偿值定位。
- 26 键竖屏、数字行切换、面板高度测试需要同时覆盖四排和五排补偿。

## Verification

- iOS 27 模拟器定向测试覆盖 `KeyboardFormFactorTests`、`KeyLayoutStyleTests`、`MorePanelFoldTests`。
- 改后截图应来自同一模拟器测试附件，并与上传后下载的文件逐字节一致。
