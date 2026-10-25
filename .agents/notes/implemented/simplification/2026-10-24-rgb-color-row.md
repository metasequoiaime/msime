# Agent Note: 整数 RGB 颜色编辑行独立复用

Status: implemented

## Problem

触屏键盘皮肤编辑器仍内嵌一套整数 RGB 颜色行：显示规范化的六位颜色、隐藏时保持整行布局语义，并把原生取色器的字符串转换为 24 位颜色数。其他共享设置若需要同一颜色契约只能复制这段 UI。

## Decision

公开 `RgbColorRow` 与 `RgbColorRowProps`，迁移现有颜色行实现。显示值统一为大写 `#RRGGBB`，原生取色器仍使用受控整数值。组件接收数值颜色、标题、可选 `hidden` 和数值回调，继续复用 `skinColor` 与 `skinColorNumber`；不引入文本输入、透明度或跟随主题重置，因为这些属于 `HexColorRow` 和候选主题颜色行的不同契约。

## Existing notes audit

已按 `ColorRow`、`skinColorNumber`、`skin-editor-platform-controls` 检索活跃笔记。

- [皮肤编辑器平台控件](../../implemented/feature/2026-10-12-skin-editor-platform-controls.md)持有触屏编辑器的整数颜色行决定，本片只把实现位置和公共归属独立出来，行为仍由该笔记约束。
- [十六进制颜色编辑行](2026-10-24-hex-color-row.md)持有透明度和文本草稿契约；本片明确不合并两种颜色类型。
- 候选窗皮肤编辑器笔记规定其颜色行使用十六进制透明度组件，与本片无关。

## Alternatives considered

- 把整数 RGB 与十六进制透明度合并成带模式参数的颜色行：可减少文件数量，但会把数值、文本草稿、透明度和可选隐藏行为揉成一个组合 API，调用方契约不再清晰。保留两个领域组件。
- 只移动函数而不公开导出：改动更小，但其他共享 UI 仍不能复用，未达到公共组件抽取目标。公开最小 props。

## Verification

公共入口测试先因缺少导出失败，随后覆盖规范化显示、原生取色器回调数值、隐藏行、多实例隔离和外部受控更新并全部通过。触屏编辑器相关测试、构建、类型检查、lint、格式、笔记、diff 与 quick 门禁待本片验证完成。

## Consequences

数值颜色继续由调用方保存并回传；组件不维护草稿。`hidden` 继续传给标准 `Row`，不通过条件渲染改变设置页布局。现有 `skinColorNumber` 的输入边界保持不变。
