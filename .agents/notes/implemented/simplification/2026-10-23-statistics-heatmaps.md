# Agent Note: 统计热力图视图独立于整页

Status: implemented

## Problem

日历热力图、按键热力图及共享图表选择按钮仍定义在 `typing-statistics.tsx` 中。它们只需要计数、范围和选择回调，却必须连同宿主读取、摘要、上传及全部统计页面加载，无法独立组合或测试。

## Decision

把两种热力图和私有键盘行绘制原样移入 `statistics-heatmaps.tsx`，共用的图表选择按钮移入 `statistics-chart-button.tsx`，继续服务热力图和页面趋势条。页面继续持有日期选择、读取状态和图表编排；纯日历及键位算法继续归已有模型，不复制到视图。不增加包入口导出，不改变 JSX、样式、可访问名称、回调参数或时序。

## 旧笔记审计

按 `StatisticsHeatmap`、`KeyboardHeatmapRows`、`StatisticsChartButton`、统计热力图检索活跃笔记。[统计纯模型](../../implemented/architecture/2026-10-15-typing-statistics-model.md)部分重叠，更新图表归属并互链；[读取状态](../../implemented/simplification/2026-10-23-typing-statistics-state.md)仍持有生命周期，不改决定。[触屏键位一致性](../../implemented/bug-fix/2026-10-16-mobile-key-heatmap-follows-keyboard.md)持有独立的原生及 `MobileStatistics` 手机键盘实现，本片不改变其布局。Android 快照日期、云上传、Windows 原生统计和平台设计笔记均属于其他实现或数据规则。没有同范围的过时提案。

## Alternatives considered

- **把全部图表合为一个大模块**：能一次缩短页面，但热力图会继续与分布、速度和日明细耦合，无法按视图职责独立使用；按共享热力图边界渐进抽取。
- **顺便统一手机摘要热力图**：可能减少重复 JSX，但摘要手机使用 `phoneHeatmapRows`，此处按 ANSI/通用触屏模型绘制，键位规则不同，不能在搬移时合并。
- **让热力图自己持有选择日期**：调用更省参数，但日历与趋势条必须同步选择和统计范围，继续采用受控 props。

## Verification

迁移前 3 个文件、51 项测试通过；独立组件测试先因待抽取模块不存在失败。热力图与选择按钮排除注释、空白和导出修饰符后与基点实现一致；`TypingStatisticsPage` 函数逐字一致，包入口导出未改。迁移后 7 个现有回归文件、81 项测试通过；新增组件测试在修正 53 周跨年同月日查询歧义后单独运行 6 项全部通过。独立只读审查无须修复项。桌面构建与类型检查、Harmony 类型检查、变更文件 lint、格式和笔记门禁通过。本地 `CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，覆盖仓库契约、Rust 工作区、Android 目标、Linux 外壳及共享 Apple 桥接。首次运行的 Swift 测试与语音目录导出因冷编译超过 120 秒失败，复用同一产物目录分别重验通过后，完整 quick 返回 0。Windows 原生工具链与 Harmony 原生打包环境未就绪，对应阶段跳过；其他无关平台按范围跳过，未做原生宿主装机验收。

## Consequences

两种热力图可以直接接收计数与范围组合使用，页面减少 281 行内嵌视图；增加两个内部组件模块。日历与趋势条仍由页面同步受控选择，键位模型和摘要手机布局保持各自归属。搬移遗漏依赖或样式会影响既有页面，因此独立组件与页面集成回归同时覆盖。
