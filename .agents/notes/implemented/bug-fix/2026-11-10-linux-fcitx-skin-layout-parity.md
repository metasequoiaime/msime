# Agent Note: Linux Fcitx5 经典界面跟随共享候选皮肤版式

Status: implemented

## Problem

Fcitx5 经典界面生成的主题只读取皮肤包直接声明的卡片圆角和行圆角，没有读取共享目录里的 `resolvedLayout`。Windows 会从同一份已合并的版式读取卡片圆角、卡片内边距、行高、行间距、序号内缩和候选右边距，因此使用同一皮肤时 Linux 的卡片和选中行会出现圆角、整体高度与内边距差异。

## Decision

Linux Fcitx5 在生成 classicui 主题前读取选中皮肤的 `resolvedLayout`，并按当前横竖排合并 `geometry` 与方向版式。卡片圆角和行圆角继续由共享目录提供，卡片内边距、行间距、行高、序号内缩和右侧留白映射到 Fcitx5 能表达的 `ContentMargin`、`Spacing`、`TextMargin` 和 `Highlight/Margin`。`em` 长度按候选字号换算，DIP 长度按候选缩放换算；用户显式设置的 `candidate_padding` 仍优先于皮肤版式。没有 `resolvedLayout` 的旧目录继续使用 Linux 原有默认值。

## Alternatives considered

- **只把 Linux 的固定默认值改成 Windows 的一组数值**：不能覆盖不同皮肤的 `corner_radius_dip`、`padding` 和 `item_extra_height`，也会改变没有版式的现有主题，因此不采用。
- **让 Fcitx5 继续只画固定主题，要求报告者手工调主题**：会让同一份共享皮肤在不同宿主产生可见差异，且用户无法通过水杉设置统一它们，因此不采用。

## Consequences

Linux classicui 能够复用共享皮肤版式中它能表达的几何，默认和没有 `resolvedLayout` 的旧配置保持不变。Fcitx5 主题格式无法表达 Windows 的选中竖条、复杂角点路径等字段，这些字段仍按现有 Linux 能力退化；本次只同步卡片和候选行的尺寸、间距与圆角。测试覆盖了解析 `resolvedLayout`、方向覆盖和生成的 Fcitx5 主题字段。
