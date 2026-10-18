# Agent Note: 维护与诊断页的 AI 助手顶部标签

Status: implemented

## Problem

共享桌面设置的「连接 AI 助手」藏在维护页的一行二级入口里，查看连接命令和返回维护操作需要反复进入、退出页面。用户要求将它独立成页顶标签。

## Decision

维护页复用 `useSettingsTabs`，顶部分为「维护与诊断」「AI 助手」。第一个标签保留服务、日志、数据目录与卸载，第二个直接呈现已有 MCP 连接组件。没有 `mcpServerStatus` 能力时不显示标签。桌面旧 `mcp` 路由仍可用，直接选中 AI 助手并保持维护页标题和侧栏高亮；手机保留原二级入口。切换只隐藏面板，保留权限和客户端选择状态，不写偏好。

## Historical audit

[工具与支持页精简](../../implemented/simplification/2026-10-16-macos-tools-support-settings-prune.md)与本篇部分重叠：其中桌面 MCP 二级入口由顶部标签取代，手机和 Windows 原生设置布局不变。[桌面顶部功能标签](../../implemented/feature/2026-10-13-desktop-input-expression-voice-tabs.md)的键盘导航和保持挂载规则继续适用。[路由状态 hook](../../implemented/simplification/2026-10-17-settings-page-routing-hook.md)继续拥有请求路由和桌面承载页的区分。

## Alternatives considered

- **保留二级页面入口**：维护页最短，也已有返回机制；但 AI 助手入口仍藏在页面下方，无法满足直接切换的要求。
- **保留 `mcp` 独立页，只替换入口为标签外观**：路由改动少，但页标题和返回按钮仍随选择变化，标签在两页重复挂载，无法保持连接组件状态。使用同一个承载页和既有请求路由更一致。

## Verification

`apps/desktop/tests/settings/mcp-connect.test.tsx` 覆盖顶部标签、权限与客户端选择保留、方向键与 Home/End、旧路由与后续外部路由、无能力回退和手机二级页。共享设置导航、能力投影及维护操作回归用于检查请求路由与承载页分离后的影响。类型检查、生产构建、笔记校验和 quick 的实际结果随 PR 记录。

## Consequences

桌面 AI 助手无需进入、退出二级页即可访问，连接组件在标签间保持挂载；维护页不再额外画一个 AI 入口行。`mcp` 仍是受能力过滤的路由，桌面承载页固定为 `developer`；导航必须使用原始请求路由验证可用性和判断重复选择，否则从 AI 助手切回维护会被丢弃。手机与 Windows 原生设置界面不随桌面共享页调整。真实原生宿主验收需有承载当前构建资源的设置应用，单元测试和前端构建不代表完成该层验收。
