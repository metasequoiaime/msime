# Agent Note: 悬浮工具栏页面共用按钮与尺寸选项模型

Status: implemented

## Problem

悬浮工具栏页面把按钮顺序、宿主能力过滤、必选模式切换项、缩放和字号选项写在 JSX 中。页面组件同时维护能力投影和渲染，其他共享宿主无法直接复用这组稳定规则。

## Decision

增加无 React 依赖的 `settings/floating-toolbar-options.ts`，持有按钮选项、宿主能力过滤和两组选项数组。页面消费纯模型；Linux/GNOME 文案、页面分组、持久化回调和预览仍由页面负责。

保留按钮顺序和能力约束：`input_scheme`、`handwriting`、`voice` 只在宿主声明对应能力时列出；其余按钮不受能力门控。清单里没有锁定的模式切换项，`english_mode` 一项就叫「中英文切换」；尺寸是「工具栏大小」的五档 `floatingToolbarSizes`，每档同时给出 `scale_percent` 和 `font_size`，见[精简悬浮工具栏、屏幕键盘和手写输入三页的设置](2026-10-16-toolbar-keyboard-handwriting-settings-prune.md)。

## Historical audit

检索 `floatingToolbarOptions`、`floating_toolbar_input_scheme`、`floating_toolbar_handwriting`、`floating_toolbar_voice` 和尺寸数组，没有已有共享 UI 模型笔记。Windows 工具栏预览与 Linux 菜单语义笔记分别持有原生绘制和平台入口约束，本片只抽取共享选项，不改变它们。

## Alternatives considered

- **继续把选项留在页面。** 改动最少，但能力过滤和顺序无法被其他宿主复用，新增按钮要在 JSX 中维护。
- **把 Linux/GNOME 文案判断也移进模型。** 可以集中全部页面规则，但文案依赖页面布局与平台说明，不是按钮选项契约；会让纯模型依赖额外页面状态。

## Consequences

按钮顺序、宿主能力门控和「工具栏大小」各档集中在纯模块；页面只负责 Linux/GNOME 文案、布局、预览和单字段回调。模型以类型导入 `HostCapabilities`，不增加运行时循环；每次调用生成新的选项数组，不把投影写回偏好。

新增两项模型测试先因未导出 API 失败，再与既有工具栏偏好、页面投影、页面可见性、主题和输入偏好共六文件 28 项通过。共享 UI 构建、Harmony 类型检查、格式、lint 和 diff 检查通过。
