# Agent Note: 共享关闭图标组件

Status: implemented

## Problem

账号登录面板和设置提示分别内嵌相同的关闭按钮 SVG；它们只在图标尺寸和外层按钮样式上有差异，路径、线宽和装饰性属性重复维护。

## Decision

公开 `CloseGlyph` 与最小的 `CloseGlyphProps`，保留现有关闭路径、圆头线帽、`aria-hidden` 语义并补上 `focusable="false"`；通过可选 `size`、`strokeWidth` 和 `className` 保留两个调用点的尺寸与类名。由共享 UI 入口导出，登录面板和设置提示删除内嵌 SVG 并复用它；两个原来仅大小写不同的等价路径统一为一份。

## Existing notes audit

已按关闭路径、账号面板、设置提示和共享图标导出检索活跃笔记；没有现成 `CloseGlyph` 实现或冲突决定。已有 `CheckGlyph`、`ChevronGlyph` 和 `SearchGlyph` 笔记记录了相同的最小 SVG 公共化模式。

## Alternatives considered

- 继续保留两份 SVG：改动最小，但同一关闭路径仍可能出现属性漂移。
- 用 `FluentIcon` 替换：会改变现有路径、线宽或局部尺寸，超出抽取范围。
- 引入通用图标库：扩展性更强，但会增加依赖和属性契约。

## Verification

- 公共入口测试先因缺少导出失败，随后 2 项测试验证默认路径、尺寸、线宽、类名和装饰性属性；登录面板、账号页底部面板焦点和设置提示共 94 项测试通过，并确认同一路径的内嵌 SVG 已消失。桌面类型检查、构建、lint、格式、笔记和 diff 校验通过；quick 门禁和 CI 结果记录在 PR 中。

## Consequences

两个调用点显式传入原有尺寸，组件只负责装饰性图标，不新增可访问名称；`focusable="false"` 使关闭图标不会进入键盘焦点顺序，外层按钮继续提供原有中文可访问名称。
