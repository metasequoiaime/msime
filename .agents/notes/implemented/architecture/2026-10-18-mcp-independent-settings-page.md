# Agent Note: MCP 作为独立的 AI 助手设置页

Status: implemented

## Problem

MCP 连接以前挂在「维护与诊断」下，页面说明也把它主要描述成读取诊断日志的工具。MCP 实际还可以在用户授权后管理快捷短语和设置、制作候选窗口皮肤、读取词库与打字统计，因此把入口放在维护分组会误导用户，也让社区皮肤页无法给出清晰的使用路径。导航注册表、共享宿主路由和 Windows 原生设置还必须对同一个页面身份保持一致。

桌面独立侧栏决定由[AI 功能共用导航入口](../simplification/2026-10-19-ai-settings-top-tabs.md)部分取代：桌面现在从「AI → 连接助手」进入，`mcp` 路由、能力与权限边界保留，触屏仍独立。

## Decision

MCP 使用独立的 `mcp` 设置路由，在桌面和触屏宿主都显示为「AI 助手」（触屏页面标题为「连接 AI 助手」），归入「工具」分组。`SettingsCategory::Mcp` 加入 `client-core` 的共享路由词汇表，Windows 原生设置把它作为工具组中的独立页面；「维护与诊断」只保留输入法服务、诊断日志、数据目录和卸载等维护操作。

连接页明确说明 MCP（Model Context Protocol）通过本机服务调用水杉输入法，不联网，并列出设置、快捷短语、皮肤、打字统计、词库和诊断等能力。社区候选皮肤页引导用户从「工具 → AI 助手」连接 MCP，并开启「允许修改设置」后让助手制作皮肤。能力过滤仍以 `mcpServerStatus` 为准，没有 MCP 服务时不显示该页面。

## Historical audit

[工具与支持页精简](../simplification/2026-10-16-macos-tools-support-settings-prune.md)记录了 MCP 从工具页移入维护子页的上一轮决定；[维护页的 AI 助手顶部标签](../feature/2026-10-18-maintenance-assistant-tabs.md)随后记录了桌面承载方式。两篇笔记的历史事实保留，但其中关于当前 MCP 承载位置的决定由本篇取代。Windows 原生设置的 MCP 原先与维护页共存的说明也随本次独立页面调整。

## Alternatives considered

- **继续放在「维护与诊断」中，只扩充说明文字**：改动最小，诊断场景的入口不会移动。没有采用，因为 MCP 的设置、皮肤和词库能力与维护操作的用户目标不同，社区皮肤页也无法用一个维护入口准确表达使用路径。
- **保留桌面顶部标签，触屏单独放进「工具」**：能减少桌面导航改动。没有采用，因为不同设备会把同一个 MCP 能力放在不同分组，宿主路由和页面标题也会分叉；独立页面能让共享 UI、Windows 原生窗口和触屏导航使用同一个 `mcp` 身份。
- **把 MCP 合并进「AI 辅助」页**：视觉上可以减少一个入口。没有采用，因为 AI 对话、候选联想等服务设置与本机 MCP 连接和权限管理是不同的配置边界，合并会再次把连接入口藏在另一项 AI 功能之下。

## Consequences

- 「工具」分组会多出一个能力条件页面；用户能从社区皮肤页按明确路径连接 MCP，权限开关仍由 MCP 页面控制。
- `settings:mcp` 成为共享路由契约的一部分，所有实现路由表都必须在新增或迁移页面时同步更新；旧的桌面 `mcp` 请求仍可打开同一独立页面，而不再映射到 `developer`。
- 维护页更短，Windows 原生设置侧栏从 16 页变为 17 页；Windows 的 MCP 页面仍使用既有 host-api 安装与状态接口。

## Verification

页面的连接分组在宿主状态加载前就存在，`settingsFormReady` 也只代表设置表单可用。命令测试共用的 `mcpGroup` 因此等待分组内的「Claude Code」标签出现后才返回，避免把空分组当成连接内容已就绪。CI 曾在移除命令测试中读到只有说明的分组；既有命令与交互断言保持不变。不使用固定延时或扩大超时，它们不能证明状态已经填入。[MCP 宿主状态刷新](../bug-fix/2026-10-14-mcp-load-owner-refresh.md)仍持有生产组件的请求所有者与代次规则，本项只修正测试等待条件。

`cargo test -p msime-client-core host_surface` 覆盖 `settings:mcp` 的解析和全部路由往返；共享设置的 MCP 连接、页面投影和社区皮肤测试通过，TypeScript 类型检查、格式检查和 `git diff --check` 通过。Windows `settings_navigation` 独立编译运行通过；macOS 设置路由覆盖脚本确认 `mcp` 已由共享路由类别覆盖，不再列为未路由页面。笔记门禁使用 `pnpm run verify-notes`。
