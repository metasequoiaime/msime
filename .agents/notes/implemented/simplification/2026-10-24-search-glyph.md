# Agent Note: 共享搜索图标组件

Status: implemented

## Problem

触屏首页、设置导航和插件库三个搜索框各自内嵌相同放大镜 SVG；它们只在尺寸、线宽和外层类名上有上下文差异，路径和装饰性属性重复维护。

## Decision

公开 `SearchGlyph` 与最小的 `SearchGlyphProps`，保留现有放大镜路径、圆头线帽和 `aria-hidden` 语义，并补上 `focusable="false"`；通过可选 `size`、`strokeWidth` 和 `className` 保留三个调用点的尺寸、线宽和布局类名。由共享 UI 入口导出，触屏首页、设置导航和插件库搜索框删除内嵌 SVG 并复用它。

## Existing notes audit

已按放大镜路径、搜索框和共享图标导出检索活跃笔记；没有现成 `SearchGlyph` 实现或冲突决定。已有 `CheckGlyph` 与 `ChevronGlyph` 笔记记录了相同的最小 SVG 公共化模式。

## Alternatives considered

- 继续保留四份 SVG：改动最小，但共享 UI 继续复制同一图标。
- 用 `FluentIcon` 替换：可减少内嵌 SVG，但会改变现有路径、线宽或局部布局，超出抽取范围。
- 引入通用图标库：扩展性更强，但会增加依赖和属性契约。

## Verification

- 公共入口测试先因缺少导出失败，随后 2 项测试验证默认路径、尺寸、线宽、类名和装饰性属性；触屏首页、设置导航和插件库测试继续覆盖搜索框行为，并确认同一路径的内嵌 SVG 已消失。桌面类型检查、构建、lint、格式、笔记和 diff 校验通过；quick 门禁和 CI 结果记录在 PR 中。

## Consequences

三个搜索框显式传入原有尺寸或类名，插件库显式传入原有的 1.5px 线宽，避免抽取后依赖不同默认值。组件只负责装饰性图标，不新增可访问名称；`focusable="false"` 使键盘焦点语义一致。
