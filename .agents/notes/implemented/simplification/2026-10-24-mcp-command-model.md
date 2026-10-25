# Agent Note: 抽取 MCP 命令展示模型

Status: implemented

## Problem

`mcp-connect.tsx` 同时持有 MCP 设置页的异步状态、权限确认和命令展示。命令行转义、安装与移除命令、终端说明、JSON 权限追加以及授权文案都埋在组件里，独立调用方无法复用这些规则，页面测试也只能通过完整设置页间接覆盖。

## Decision

新增无 React 依赖的 `mcp-command-model.ts`，持有 MCP 客户端和服务器状态类型，以及命令 token、终端说明、配置追加和授权文案的纯模型函数。[连接状态 hook](2026-10-24-mcp-connect-state.md)持有加载、复制、安装、刷新和本地权限偏好状态，展示组件保留确认框和命令展示；两者调用模型函数。`mcp-connect.tsx` 继续重导出原有类型，`SettingsClient` 和包入口的类型路径保持兼容。

模型继续使用既有 `mcp-syntax` token 类型，因此复制文本与高亮展示来自同一结果。保留 POSIX 单引号与 Windows 双引号转义、`--options` 和两个权限参数的顺序、Claude Code/Codex 两种命令、终端文件参数说明、无权限时保留宿主原始 JSON 格式，以及无效 JSON 或 MCP 条目形状时原文回退。命令行页先说明复制、运行和三步用途，再展示完整命令；路径和权限参数仍由模型生成，但不再要求用户自行推断它们的作用。

## Existing notes audit

- [MCP 独立设置页](../architecture/2026-10-18-mcp-independent-settings-page.md)拥有页面路由、入口和宿主能力边界，本片只抽取展示命令模型。
- [MCP 复制反馈生命周期](../bug-fix/2026-10-12-mcp-copy-feedback-lifetime.md)、[加载归属](../bug-fix/2026-10-14-mcp-load-owner-refresh.md)和权限确认相关笔记的时序约束由连接状态 hook 遵循，命令模型不改变异步操作时序。
- 既有 `mcp-syntax.tsx` 拥有 token 渲染与 JSON token 化，本片复用它，不复制语法高亮逻辑。

## Alternatives considered

- **只抽出命令字符串**：会丢失现有高亮 token 与复制文本共用的契约，调用方还要重新组装 token。
- **把整个 MCP 组件搬出设置页**：会同时移动异步状态、确认和宿主错误边界，扩大切片并改变生命周期归属；本片只移动无副作用规则。
- **让模型接收完整设置客户端**：会把纯展示规则绑定宿主协议；显式接收服务器状态和权限列表即可保持可独立组合。

## Verification

独立模型测试先因模块缺失失败；实现后 5 项通过，覆盖权限顺序、两种安装/移除命令、终端说明、授权文案、JSON 追加与回退、Windows 路径转义。命令行页测试同步覆盖面向操作的三步文案；本次环境因共享工作区的 `@msime/ui` 链接指向已不存在的外部 worktree，无法启动完整设置页测试。`git diff --check` 通过。

## Consequences

MCP 命令展示规则可以在没有设置表单或 React 生命周期的环境中复用和测试；连接状态 hook 负责宿主调用和反馈，组件渲染确认框。类型通过旧模块重导出保持现有调用方不变，新增模型模块不进入包入口，避免扩大公开 API。
