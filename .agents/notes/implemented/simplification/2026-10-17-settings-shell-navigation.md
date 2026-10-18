# Agent Note: 抽取设置外壳导航投影

Status: implemented

## Problem

`SettingsShell` 仍直接持有推入页返回规则、父页面查找、侧栏搜索过滤和搜索框组装。它们都属于外壳导航，却和平台布局、社区页面以及表单渲染交织在同一组件中；返回路径和搜索投影的约束也只能通过外壳的大型集成测试间接验证。

## Decision

`packages/ui/src/settings/use-settings-shell-navigation.tsx` 集中管理外壳导航的四个结果：`parentPage`、`goBack`、`shownSidebarGroups` 和 `navSearchField`。返回逻辑继续使用现有的 `launchPage`、`mobileTabForPage` 和 `selectPage(..., true)` 契约：推入页沿浏览器历史返回，启动页替换为父页或所属标签根页。搜索仍只按页面标题过滤，组名不参与匹配，空查询继续返回原分组引用。设置外壳只传入路由、侧栏分组和查询状态，并消费同名结果。

## Alternatives considered

- **继续把导航派生逻辑留在 `SettingsShell`**：改动最小，但返回和搜索规则继续隐藏在页面渲染函数中，后续拆外壳时会重复移动这些约束。
- **只抽出搜索过滤纯函数**：能覆盖一个条件，但返回出口和搜索框接线仍留在外壳，导航行为没有完整边界。
- **把整个 `SettingsShell` 一次搬到新文件**：能同时降低入口文件长度，但会把页面拼装、平台展示和社区图库契约混入迁移，扩大冲突面；本切片只抽取导航投影和动作。

## Consequences

外壳导航有独立的 hook 与测试，页面组件和平台展示继续由 `SettingsShell` 负责。返回历史、启动页替换、父页标题、侧栏过滤和搜索框回调保持原行为；hook 依赖 React 元素和浏览器历史，因此仍属于共享 UI 外壳层，不下沉到纯模型层。新增测试覆盖推入页历史返回、启动页父页替换、标题过滤和搜索框动作接线。

导航 hook 由[独立设置页组件](2026-10-19-settings-page-component-module.md)中的 `SettingsShell` 调用；组件搬移保留其返回、搜索和侧栏折叠规则。
