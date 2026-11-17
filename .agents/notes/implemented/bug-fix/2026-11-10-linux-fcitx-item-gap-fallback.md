# Agent Note: Fcitx5 行间距使用可识别的主题字段

Status: implemented

## Problem

Linux Fcitx5 主题同步共享皮肤版式时，把 `item_gap` 写成 `[InputPanel]` 下的 `Spacing`。Fcitx5 classicui 的 `InputPanelThemeConfig` 不读取这个字段；`Spacing` 属于菜单配置，因此生成的主题文本虽然包含数值，候选项之间仍没有对应间距。

## Decision

生成主题时不再写 `[InputPanel]` 下的 `Spacing`。Fcitx5 只提供 `InputPanel/TextMargin` 作为候选文本盒子的间距原语，因此把 `item_gap` 在四个方向拆分到文本边距；奇数像素在相邻两侧分别取进位和舍去，保证相邻候选的总间距不丢失。`InputPanel/Highlight/Margin` 使用同一组边距，保持选中背景和候选命中区域与文本盒子一致。

这是 Fcitx5 classicui 能表达的最接近映射；该主题格式没有候选项专属的 gap 字段，所以预编辑和辅助文本也会沿用同一组 `TextMargin`，不会伪造一个 classicui 不认识的配置键。

## Alternatives considered

- **继续写 `InputPanel/Spacing`**：配置文件看起来与共享字段对应，但 Fcitx5 不会读取，运行时行为与测试字符串不一致，因此不采用。
- **完全删除 `item_gap`**：能避免无效字段，却让共享皮肤在 Linux 上继续缺少可见的行间距，不符合本次跨平台版式同步目标，因此不采用。

## Consequences

支持 `item_gap` 的皮肤在 Fcitx5 classicui 中会获得实际可见的候选间距，选中背景的外沿和命中区域也同步扩大。没有 `item_gap` 的旧主题生成结果保持不变；由于 classicui 只有共享文本边距，预编辑和辅助文本会同时继承这组边距。单测同时确认生成结果不再包含无效的 `InputPanel/Spacing`，并核对奇数间距的拆分值。
