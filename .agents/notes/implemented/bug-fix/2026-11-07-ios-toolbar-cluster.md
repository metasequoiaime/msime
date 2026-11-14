# Agent Note: iOS 工具栏图标聚拢到右侧

Status: implemented

## Problem

iOS 顶栏把品牌 Logo、功能图标和收起按钮放进全宽等分的 `UIStackView`。可见按钮数量变化时，每个按钮仍占整行的一格，Logo 以外的图标被拉得很散，和截图中的紧凑工具栏不符。

## Decision

工具栏把品牌 Logo 固定在左侧，加入可伸缩空白，其余可见按钮固定为 40pt 格、按钮之间留 8pt，整体靠右排列。隐藏按钮继续留在同一个 `UIStackView` 中，功能入口和无障碍顺序不变。

## Alternatives considered

- 继续使用全宽等分：按钮数量少时间距过大，正是反馈中的问题，放弃。
- 把所有按钮靠左：能避免空白，但品牌 Logo 与功能入口挤在一起，和现有工具栏层级不符，放弃。

## Consequences

- Logo 与右侧功能入口的位置清楚，图标间距不再随按钮数量把整行拉开。
- 右侧按钮仍使用原来的 40pt 命中格，隐藏和显示入口不会改变按钮顺序。
- 工具栏布局测试改为检查 Logo、伸缩空白、右侧固定格和间距。
- Android 和鸿蒙随后按同样的格宽和间距改成这种排法，见 [Android 与鸿蒙空闲工具栏图标聚拢到右侧](2026-11-12-touch-toolbar-cluster.md)。

## Verification

- iOS 27 模拟器定向测试覆盖工具栏布局与功能面板回归。
