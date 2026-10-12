# Agent Note: 社区安装与替换确认共用组件

Status: implemented

## Problem

社区候选皮肤与插件详情页重复组合安装按钮、安装完成内容和同名项目替换确认。两处都用同一个 `useCommunityInstallState`，但展示分支和 `install(false)`、`install(true)` 的事件连接仍各写一遍。

## Decision

新增 `CommunityInstallActions`，统一安装前按钮、安装后内容、替换确认和对应的布尔安装事件。页面传入各自的安装完成提示、替换文案和确认标题；原有的 `CommunityInstallButton`、`CommunityReplaceConfirmation` 继续负责按钮及对话框的具体展示。

## Alternatives considered

- 把不同页面的提示和替换文案写进新组件：会让共用组件知道插件种类及皮肤包 id。
- 只共用现有两个基础组件：两页仍重复控制安装、替换确认的状态分支和回调。

## Consequences

两个详情页复用同一安装状态展示入口，页面特有文案和安装操作保持原样。社区页面交互测试 63 项通过，覆盖普通安装、已存在项目的替换、取消和安装后状态；类型检查、前端构建、格式、lint、notes 和本地 quick 门禁均通过。quick 中 HarmonyOS 原生编译因缺少已暂存的库和资源按脚本规则跳过。
