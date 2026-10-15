# Agent Note: 输入源启动状态与展示条件独立复用

Status: implemented

## Problem

`InputSourceStartupStatus` 和 `inputSourceNeedsAdding` 位于提示组件中，安装页、宿主接口和设置 hook 依赖该组件文件。提示是否显示、是否继续重查又分别内联在状态区域和 hook 中，安装失败、等待重新登录与系统目录重复副本之间的条件缺少集中验证。

## Decision

启动状态类型及添加、重查、展示三项纯判定位于 `input-source-startup-model.ts`，让组件、设置 hook、安装页和宿主类型直接依赖模型。保留提示组件的旧类型与函数导出，包入口维持现有类型导出。文案、定时器和请求代次留在原调用处，不改变任何状态组合的结果。

## Alternatives considered

- 只挪类型：迁移较少，但继续将纯判定分散在 React 文件，无法统一验证重查与提示的边界。
- 连安装流程和提示文案一起移入模型：模型能提供更完整的展示结果，但这些文案和状态流转各由单个 UI 使用，本片不需要扩大契约。

## Verification

纯模型用六种 action、三种 enabled 状态和有无系统副本的 36 组矩阵验证三个判定，另覆盖无启动结果与缺少可选字段。旧路径函数身份由组件测试覆盖；安装页、启动提示及设置页重查测试覆盖调用关系。类型检查、共享组件构建和 quick 是提交门禁。

## Consequences

类型和纯判定的消费者直接依赖无运行时导入的模型，测试无需加载 React。代价是保留旧路径的转出声明。`login_required` 和 `not_installed` 仍显示提示，但不因添加状态触发重查；系统副本独立触发重查，包括安装失败时。模型不负责安装流程、请求代次、定时器或提示文案。

## Related notes

已检索活动笔记：本片与[首次安装登录提示](../../implemented/bug-fix/2026-10-09-macos-first-install-login-required.md)及[共享间隔 hook](../../implemented/simplification/2026-10-15-shared-interval-hook.md)部分重叠，分别保留其宿主探针决策和定时器所有权。本片只收拢前端状态契约与判定。
