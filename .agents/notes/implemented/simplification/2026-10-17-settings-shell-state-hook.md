# Agent Note: 设置外壳临时状态 hook

Status: implemented

## Problem

`packages/ui/src/index.tsx` 的 `SettingsShell` 同时保存插件与皮肤图库视图、侧栏搜索、手机标题折叠、对话页首次挂载和页面列引用。这些状态只服务外壳，却与外壳的导航、平台布局和页面渲染混在一起；后续拆分外壳时容易重复移动同一组生命周期约束。

## Decision

设置外壳通过 `packages/ui/src/settings/use-settings-shell-state.ts` 集中管理这些临时状态。hook 保留插件与皮肤视图的 `mine/community` 初始值、侧栏查询和标题折叠状态、对话页首次进入后持续挂载的效果，以及页面列和启动页引用。`SettingsShell` 继续消费同一字段和 setter，页面路由、图库选择、动画重启和返回行为不变。

## Alternatives considered

- **继续把状态留在 `SettingsShell`**：改动最小，但外壳状态会继续与渲染和平台分支纠缠，后续公共外壳拆分需要重复理解对话页挂载约束。
- **把整个 `SettingsShell` 搬到新文件**：可以一次性降低入口文件长度，但会同时移动大量页面拼装、平台布局和社区图库契约，扩大冲突面；本切片只抽取状态生命周期。

## Consequences

外壳的局部状态有独立、可测试的边界，新增外壳页面可以复用该 hook 的生命周期约束。hook 仍由共享设置入口导出，页面状态字段保持原名和原始 setter 语义；它不负责导航选择、页面动画或渲染，因此这些逻辑的所有权没有改变。

临时状态由[独立设置页组件](2026-10-19-settings-page-component-module.md)中的 `SettingsShell` 调用，状态生命周期与返回契约保持不变。

[功能页组合](2026-10-19-settings-feature-pages.md)消费外壳传入的聊天挂载标记、焦点请求与 setter，不调用第二份状态 hook；桌面切页卸载与触屏首次打开后保留的差异仍按原条件处理。
