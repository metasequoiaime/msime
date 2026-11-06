# Agent Note: iOS 26 键排距

Status: implemented

## Problem

iOS 26 键竖屏把 44pt 字母键帽收紧后，额外在键区上下和每个排间插入留白，紧凑键盘上的排间距显得过大。

## Decision

- 26 键竖屏保留 44pt 键帽的上下对齐留白（四排 10pt、五排 12pt），但移除排间距中重复叠加的 `phoneLetterSpareRowPadding`。各排只保留用户设置的排距和一份对齐间距，键盘外框高度不变。
- 原有布局决定见 [触屏按键布局样式](../feature/2026-10-27-touch-key-layout-styles.md)；本次只修正 iOS 26 键排距，不改变共享偏好或其他平台。

## Alternatives considered

- 把省下的空间留在键区底部：会让候选栏和按键之间的视觉比例失衡，因此把这部分空间回收进按键高度，保持键盘外框不变。

## Consequences

- 26 键竖屏的排间距从 15pt 收到 11pt，键盘外框、候选栏位置和上下对齐不变；九键、平板和横屏布局不受影响。
- `KeyboardFormFactor` 相关几何测试与面板高度测试同步固定新的排距。

后续排距决定见 [iOS 26 键排距回到用户设置值](2026-11-07-ios-letter-spacing-default.md)。

## Verification

- `git diff --check`
- `pnpm run verify-notes`
- iOS 定向测试：远端 iOS 27 模拟器完成资源准备、原生编译；`KeyboardFormFactorTests`、`KeyLayoutStyleTests`、`MorePanelFoldTests` 及工具栏布局回归共 119 项全部通过（0 failures）。

