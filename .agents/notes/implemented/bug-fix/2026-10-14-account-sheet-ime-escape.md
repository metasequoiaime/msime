# Agent Note: 账号底部面板的 Esc 先让输入法取消组词

Status: implemented

## Problem

共享账号界面的 `BottomSheet` 承载登录和昵称编辑。它对任何 Esc 都关闭面板并停止传播，取消输入法候选也会丢掉面板内的编辑草稿。

## Decision

Esc 关闭入口先检查 `nativeEvent.isComposing` 和 `keyCode === 229`。组合事件保留面板、草稿和默认处理；独立普通 Esc 继续关闭，操作进行期间仍由 `closeDisabled` 约束关闭。

## Alternatives considered

- 取消 Esc 关闭可以避开组词边界，但会移除既有键盘操作；过滤组合事件即可保留普通 Esc。
- 只看 `isComposing` 字段更简单，但组词结束可能早于最后一次键盘事件，还需 `keyCode=229` 识别结束边界，依据 [MDN 键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。

## Testing

实际账号登录与昵称编辑面板各自覆盖两类组合 Esc。旧实现上 4 项全部失败；修复后面板和字段保留，默认行为不取消，随后普通 Esc 可以关闭。账号页、昵称编辑器和验证码表单三个测试文件共 72 项通过，前端类型检查与构建、改动文件 lint 和格式检查通过。

`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Rust 工作区、Android 目标、Apple 桥接编译通过；Linux Docker、Windows MinGW 与 HarmonyOS 原生工具链未配置，相关阶段跳过。

## Consequences

用户先取消组词，再用一次独立 Esc 关闭面板，避免丢掉编辑草稿。本次不改登录流程、昵称校验和焦点逻辑；验证属于共享组件，没有做原生宿主设备验收。

## 旧笔记归属

检索 `BottomSheet`、账号面板与组词：全平台设计稿笔记持有平台视觉和设置分层，鸿蒙 Google 登录笔记持有等待与取消流程，iOS 登录呈现笔记持有原生 SwiftUI 的挂载约束；均不持有共享面板的 Esc 组合事件契约，保留各自决定。
