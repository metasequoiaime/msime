# Agent Note: iOS 26 键排间距保持紧凑

Status: implemented

## Problem

上一版把 44pt 字母键省下的高度全部分摊到三条排间距，默认排距从 11pt 增加到约 18pt。键盘外框和九键一致，但字母键之间的空白过大，26 键看起来松散。

## Decision

手机竖屏 26 键继续使用 44pt 键帽，排间距固定为默认存储值加 4pt，默认值下为 11pt。为了保持九键与英文切换时外框高度不变，`KeyboardFormFactor.keyboardHeight` 按 52pt 九键键区计算补偿高度，`KeyboardViewController` 在字母键区上下各留 10pt。横屏不增加额外排距，也不使用竖屏补偿留白。

## Alternatives considered

- 把 32pt 全部分摊到三条排间距：内部键区边界能与九键重合，但默认排距约 18pt，空白过大，放弃。
- 恢复 52pt 键帽：可以直接填满高度，但字母键重新变得又高又窄，放弃。

## Consequences

26 键的排距回到紧凑的 11pt，键帽仍保持 44pt，九键与英文切换时外框高度不跳。字母键区上下各有 10pt 的补偿留白，因此字母键区覆盖范围比九键短 20pt；这是为了保留紧凑排距和较矮键帽的视觉比例。回归测试覆盖高度公式、排距、补偿留白和切换后的外框高度。

本决定取代[26 键内部键区与九键等高](2026-10-18-ios-letter-inner-height-alignment.md)。

九键一族（拼音九键、九键双拼、笔画）切到英文时的上下留白已由[九键一族切英文时字母区与九宫格等高](2026-10-30-ios-nine-key-english-inner-height.md)取代，其他方案的 26 键仍按本篇。
