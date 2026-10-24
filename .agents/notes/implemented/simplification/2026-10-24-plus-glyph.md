# Agent Note: 共享加号图标组件

Status: implemented

## Problem

词库详情和设置语言卡片分别绘制语义相同的加号 SVG。两处设计使用不同的坐标系和线宽，直接复制路径会让图标属性继续分散维护。

## Decision

公开 `PlusGlyph` 与最小的 `PlusGlyphProps`，用明确的 `variant` 保留 `compact`（16×16 词库行）和 `row`（24×24 语言行）两种既有几何；通过 `size`、`strokeWidth` 和 `className` 保留调用点的尺寸、线宽和布局。由共享 UI 入口导出，词库详情通过兼容转出复用它，语言卡片删除内嵌 SVG 并复用它。

## Existing notes audit

已按加号路径、词库详情、语言卡片和共享图标导出检索活跃笔记；没有现成公共 `PlusGlyph`（词库详情当前只有局部组件）或冲突决定。已有 `CheckGlyph`、`ChevronGlyph`、`SearchGlyph` 和 `CloseGlyph` 笔记记录了相同的最小 SVG 公共化模式。

## Alternatives considered

- 继续保留两份 SVG：改动最小，但同一语义图标仍可能漂移。
- 强行统一为一个坐标系：API 更小，但会改变至少一处图标的实际线段长度和视觉重量。
- 用 `FluentIcon` 替换：会改变现有路径、线宽或局部尺寸，超出抽取范围。

## Verification

- 公共入口测试先因缺少导出失败，随后 2 项测试覆盖两个变体的路径、viewBox、尺寸、线宽、类名和装饰性属性；词库详情、语言卡片和 HarmonyOS 词库相关测试共 64 项通过，并确认语言卡片不再内嵌加号 SVG。桌面类型检查、构建、lint、格式、笔记和 diff 校验通过；quick 门禁和 CI 结果记录在 PR 中。

## Consequences

两个变体明确锁定既有几何，避免通过缩放改变线段长度；组件只负责装饰性图标，不新增可访问名称，并将 `focusable` 固定为 `false`。
