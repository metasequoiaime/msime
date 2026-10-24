# Agent Note: 共享勾选图标组件

Status: implemented

## Problem

`keyboard-switch-guide`、触屏键盘皮肤编辑器、设置引导状态卡片、选项面板和皮肤网格各自内嵌相同路径的 SVG 勾选图标。它们只在尺寸和线宽上有宿主语境差异，修正路径或无障碍属性时容易漂移。

## Decision

公开 `CheckGlyph` 与最小的 `CheckGlyphProps`，保留同一勾选路径、圆角线帽、`aria-hidden` 和 `focusable` 语义；通过可选 `size`、`strokeWidth` 和 `className` 覆盖五个调用点的尺寸样式。皮肤网格用 `display: contents` 的标记包保留原有 `data-skin-check` 选择器，SVG 本身仍由公共组件负责绘制。由共享 UI 入口导出，所有调用点删除内嵌实现并复用它。

## Existing notes audit

已按 `CheckGlyph`、勾选 SVG 路径和共享图标导出检索活跃笔记；没有现成公共实现或冲突决定。React 官方文档确认 JSX 使用驼峰式 SVG 属性并支持内置 SVG 元素。

## Alternatives considered

- 继续保留两份内嵌 SVG：改动最小，但公共 UI 仍复制同一图标。
- 用 Fluent 图标替换两处：可减少 SVG 代码，但会改变现有路径、线宽和局部布局，超出抽取范围。
- 用带大量通用 SVG 属性的图标框架：扩展性更强，但会引入不必要的 API 和依赖。

## Verification

公共入口测试先因缺少导出失败，随后 2 项测试验证默认路径、可选尺寸和线宽；键盘引导、触屏编辑器、设置引导、选项面板及皮肤网格测试通过，并确认所有内嵌勾选路径已消失。桌面构建、Harmony 类型检查、lint、格式、笔记和 diff 校验通过；quick 门禁和 CI 结果记录在 PR 中。

## Consequences

`size` 未设置时保留皮肤编辑器由父级 CSS 控制尺寸的行为；键盘引导显式传入 18px 固定尺寸和 2.4px 线宽，新增调用点分别保留原有的固定尺寸和线宽。组件只负责装饰性图标，不新增可访问名称；皮肤网格的包裹标记只保留现有测试和样式选择器，不改变可访问树。
