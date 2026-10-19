# Agent Note: AI 功能共用导航入口

Status: implemented

## Problem

「AI 辅助」与「AI 助手」并列，名称相近却要占两个侧栏入口。用户要求合为一个导航、用顶部标签切换，同时取消四字标题限制。

## Decision

桌面侧栏统一为「AI」，顶部「辅助功能」承载服务、候选与对话入口，「连接助手」承载 MCP 客户端与权限。复用既有内容与设置存储，标签只影响显示。没有 MCP 能力时不显示空标签。触屏导航保持原有独立页面。

保留 `settings:ai` 与 `settings:mcp` 的路由身份，桌面后者打开 AI 的连接标签，同一窗口重复收到路由也切到正确标签。Windows 原生窗口同步合并，社区皮肤说明同步给出新路径。

## Historical audit

- [MCP 独立页](../../implemented/architecture/2026-10-18-mcp-independent-settings-page.md)：部分取代桌面独立侧栏决定；保留 MCP 路由、能力过滤、权限边界和触屏页面。
- [维护页助手标签](../../implemented/feature/2026-10-18-maintenance-assistant-tabs.md)：已由独立页笔记取代，继续保留历史理由，不恢复维护页入口。
- [自然标题](../../implemented/simplification/2026-10-19-settings-navigation-semantic-titles.md)：继续取消字数限制，本篇取代其中两项 AI 桌面入口并列的事实。
- MCP 刷新所有者、权限、复制反馈与服务配置等笔记不涉及导航归属，均不变。

## Alternatives considered

**继续分开导航**：两种配置边界最明显、可直接进入；但相似名称降低识别，顶部标签也能保持边界，因此采用用户要求的合并方式。

**内容直接堆在一页**：无需切标签，但连接与服务配置混杂，长页难找入口，因此保留清晰的两类标签。

## Verification

合成浏览器核对 macOS 1000×780 浅色、800×600 深色与 Linux 800×600 深色：只有一个 AI 入口，旧 MCP 路由选中连接标签，两类内容与方向键切换正常，没有横向溢出。Windows 导航模型在 macOS 本机运行通过；设备冒烟脚本改为遍历 16 个入口并切换 AI 的两类配置。

相关回归 13 个文件共 572 项通过，覆盖同页重复 MCP 路由、切回辅助路由、无 MCP 能力、触屏独立页、表单草稿与权限选择。提示词用例中两处旧文案断言先在改名前的分支复现失败，再同步为当前说明，原有保存断言保留；该文件 7 项单独重跑全过。桌面与 HarmonyOS 共享设置构建、改动文件格式和 lint、笔记门禁及本地 `scripts/verify-local.sh --quick` 通过。Windows 原生测试在本机运行 157 项通过；WinUI 编译与 HarmonyOS ArkTS 仍因本机工具链条件跳过。

## Consequences

合并减少一个相似入口，服务配置与助手权限仍由不同标签和原有组件管理；旧路由不失效，标签选择不写入配置。Windows 原生窗口仍需 Windows 工具链及桌面验收，本机导航模型不能替代 WinUI 真机。
