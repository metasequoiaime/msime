# Agent Note: 设置导航搜索组件

Status: implemented

## Problem

共享设置外壳在 `packages/ui/src/index.tsx` 内联绘制同一个导航搜索框两次：标题栏和侧栏都复用 JSX，但搜索输入、Escape 清空、筛选结果回车跳转和输入法组词保护都藏在页面入口里。之后的设置外壳拆分需要再次搬运这段 UI 与键盘契约。

## Decision

`packages/ui/src/settings/settings-navigation-search.tsx` 提供 `SettingsNavigationSearch`，接收已筛选的页面分组、查询值、查询变更回调和页面选择回调，统一渲染图标与输入框。它继续使用共享 `isComposingKeyEvent`：组词期间不清空、不跳页；普通 Escape 清空查询；普通 Enter 选择第一组的第一个页面。设置外壳把同一组件作为标题栏和侧栏的内容传入，保留外层各自的布局容器。

## Alternatives considered

- **继续保留内联 JSX**：行为最直观，但标题栏和侧栏的共享组件边界仍不明确，后续修改容易只改一处。
- **把过滤、导航状态也一并搬进组件**：组件会接管页面列表投影和路由选择，扩大职责并让它依赖设置外壳模型；本切片只抽取 UI 与键盘行为。
- **只抽取图标和输入框样式**：能减少少量 JSX，但 Escape、Enter 和组词保护仍会在调用方重复，无法固定交互契约。

## Consequences

标题栏与侧栏现在使用同一公共搜索组件，组合键回归有独立测试，设置入口只保留筛选和路由状态。组件不负责筛选页面、不创建路由历史、不包裹侧栏标签，因此平台布局和导航状态所有权保持不变。
