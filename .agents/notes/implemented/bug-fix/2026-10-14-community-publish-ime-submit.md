# Agent Note: 社区发布的回车先让输入法确认组词

Status: implemented

## Problem

候选皮肤与插件发布对话框共用 `handleCommunityPublishKeyDown`，它把文本框里的任何回车当成发布。名称输入尚在组词、发布材料和授权已就绪时，确认候选也会上传未完成的名称，同时取消输入法默认处理。

## Decision

在共享回车处理入口检查 `nativeEvent.isComposing` 和 `keyCode === 229`。组合事件保留默认行为，不调用发布；独立普通回车继续提交，复选框仍只阻止外层表单提交。

插件的事件入口位于独立 `community-plugin-publish-dialog.tsx`，仍调用同一个共享处理函数；状态边界见[插件发布模块](2026-10-16-community-plugin-publication.md)。

[候选窗口皮肤发布状态](../simplification/2026-10-16-candidate-skin-publication.md)独立后，IME 事件仍由对话框转交同一处理函数，hook 不接收键盘事件。

## Alternatives considered

- 只允许点击发布按钮，可以完全避开输入法回车边界，但会移除现有文本框快捷提交；沿用其他共享输入框的组合事件检查即可保留两者。
- 只检查 `isComposing` 最简单，但最后一个组词键盘事件可能晚于 `compositionend`，需要 `keyCode=229` 兼容边界，依据 [MDN 键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。

## Testing

4 个组件回归用例在旧实现上全部失败，直接确认两类组合回车都会调用发布接口。修复后运行皮肤发布、插件发布、共享回车入口和字段校验四个测试文件，共 79 项通过；组合事件不发布且保留默认行为，独立普通回车继续发布。桌面前端类型检查与构建、改动文件 lint 和格式检查通过。

`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Rust 工作区、Android 目标及 Apple 桥接编译通过；Linux Docker、Windows MinGW 和 HarmonyOS 原生工具链未配置，相关阶段跳过。

## Consequences

输入法确认候选后，用户需要独立回车或点击按钮发布，避免把编辑中的名称提前上传。使用弃用的 `keyCode` 仅为识别输入法兼容事件，与仓库既有输入框一致；本次不增加组合状态机。组件测试使用合成资源和发布替身，没有对线上服务做发布，也没有做原生宿主设备验收。

## 旧笔记归属

检索 `handleCommunityPublishKeyDown`、发布回车和社区发布表单：发布建议字段笔记持有 Rust 默认值生成，特效包分享笔记持有插件类型与库位置，手机预览笔记持有预览布局；都不持有共享发布按键契约，属于相关模块内的独立决定。
