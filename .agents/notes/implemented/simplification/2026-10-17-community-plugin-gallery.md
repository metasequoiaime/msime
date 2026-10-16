# Agent Note: 社区插件图库状态独立复用

Status: implemented

## Problem

`CommunityPluginsPage` 仍同时持有图库列表适配、类型筛选、安装预检与发布刷新，其他共享宿主无法独立复用插件图库状态。

## Decision

公开 `useCommunityPluginGallery`，接收现有客户端、本地目录读取器和安装通知回调，原样迁移页面业务状态；页面保留 JSX 和发布对话框接线。沿用公共图库和安装状态，不合并插件与候选皮肤的安装语义。

类型筛选失败时恢复旧类型，后续分页沿用原列表的类型与偏移量；列表适配继续传递 `skipped`。安装预检同时比较类型与包标识，扫描失败继续安装，迟到的旧预检不调用旧客户端。安装完成后仍只在当前图库代次有效时更新界面并通知调用方。

## 旧笔记归属

- [插件卡片](../../implemented/simplification/2026-10-17-community-plugin-card.md)与[插件详情](../../implemented/simplification/2026-10-17-community-plugin-detail.md)部分重叠，同步其状态 owner 到本 hook。
- [图库适配器](../../implemented/simplification/2026-10-12-community-gallery-adapter.md)与[安装控件](../../implemented/simplification/2026-10-12-community-install-actions.md)继续持有通用能力，本片只组合插件规则。
- [插件发布预检](../../implemented/bug-fix/2026-10-16-community-plugin-publication.md)负责发布草稿和本地打包预检，保持独立。

检索未发现同名提案或完全吸收的决定，无需归档。

## Alternatives considered

- 保留页面内逻辑：依赖关系最少，但无法独立复用图库业务状态。
- 合并所有图库为一个大 hook：可以少写几个组合函数，但插件的类型、跳过行偏移和安装参数与皮肤试用、同步不同，容易引入无效参数组合。

## Verification

公共入口可独立调用，独立测试覆盖筛选回滚和偏移、安装替换、旧预检隔离与发布刷新；相关 7 个测试文件共 51 项通过；桌面生产构建、HarmonyOS 类型检查、lint、格式及笔记校验通过，提交前另运行完整 quick 门禁。页面 JSX 与原文案保持一致。

## Consequences

公开入口增加一个 hook 及其输入和返回类型，页面只保留展示接线。搬迁时漏传 `skipped` 会导致分页重复；安装通知提前越过代次检查会改变既有语义。用直接 hook 测试与页面回归守住边界，独立审查确认逻辑除注释与格式外逐字一致。不涉及宿主文件读写、权限或原生资源，不声明设备验收。
