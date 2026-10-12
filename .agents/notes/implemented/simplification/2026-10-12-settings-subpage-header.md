# Agent Note: 设置全屏子页复用页面标题

Status: implemented

## Problem

设置主页已经使用 `SettingsPageHeader`，但社区图库、语音服务配置和键盘皮肤的两个全屏子页仍各自重复返回按钮、图标与标题标记。子页需要 `h2`；AI 皮肤生成过程中还要禁用返回按钮。

## Decision

为现有 `SettingsPageHeader` 增加可选的二级标题与返回按钮禁用属性，保持根页面默认 `h1#page-title`。四处全屏子页改用同一组件，保留各自标题、返回动作和 AI 生成时的禁用规则。

## Alternatives considered

- 新建另一套子页标题组件：会保留两份相同的返回按钮和标题样式。
- 把四处标题都改成 `h1`：会改变嵌套子页的标题层级，并可能重复根页面的 `page-title` id。

## Consequences

设置页和全屏子页共享标题与返回按钮实现，原有 heading 层级和交互约束不变。相关 UI 测试 49 项通过，类型检查、前端构建、格式、lint、notes 和本地 quick 门禁通过；quick 中 HarmonyOS 原生编译因缺少已暂存的库和资源按脚本规则跳过。
