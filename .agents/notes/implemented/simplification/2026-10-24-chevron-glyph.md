# Agent Note: 共享右向 chevron 图标组件

Status: implemented

## Problem

设置平台控件、触屏皮肤编辑器和输入方案列表各自内嵌同一右向 chevron 的 SVG 路径。尺寸、线宽和颜色类名因宿主语境不同，但路径、viewBox 和装饰性语义相同，重复代码会让图标属性逐渐漂移。个人资料行、选择面板和鸿蒙子页头还各自保留了一套局部几何。

## Decision

公开 `ChevronGlyph` 与最小的 `ChevronGlyphProps`，保留右向路径、圆角线帽、圆角连接、`aria-hidden` 和 `focusable` 语义；通过可选 `size`、`strokeWidth` 和 `className` 保留调用点的尺寸与颜色样式。增加 `profile`、`sheet-back` 和 `subpage-back` 变体，分别承载个人资料行、选择面板和鸿蒙子页头的原有 viewBox、尺寸、方向、线宽和路径。由共享 UI 入口导出，平台控件、触屏皮肤编辑器、输入方案卡片、个人资料行、选择面板和鸿蒙子页头复用它。

## Existing notes audit

已按右向和左向 chevron 路径、`PickerRow`、触屏皮肤编辑器、输入方案卡片、个人资料行、选择面板和鸿蒙子页头检索活跃笔记；没有现成公共实现或冲突决定。React 官方文档确认 JSX 支持内置 SVG 元素并使用驼峰式 SVG 属性。

## Alternatives considered

- 保留各处 SVG：改动最小，但共享设置 UI 继续复制相同图标。
- 统一替换为 Fluent 图标：会改变现有路径和局部尺寸，超出抽取范围。
- 使用通用图标库：会引入不必要的依赖和更大的属性契约。
- 为每个返回箭头另建组件：可以保留几何，但继续复制相同的装饰性语义和渲染属性，并增加公共图标数量。

## Verification

公共入口测试先因缺少导出失败，随后 5 项测试验证默认、`profile`、`sheet-back` 和 `subpage-back` 几何、尺寸、线宽、类名和装饰性属性；选择面板、账号资料页和公共 chevron 测试共 21 项通过。桌面构建、Harmony 类型检查、lint、格式、笔记和 diff 校验通过；quick 门禁和 CI 结果记录在 PR 中。

## Consequences

所有调用点显式传入原尺寸和需要覆盖的线宽，避免抽取后依赖不同默认值；组件只负责装饰性图标，不新增可访问名称。
