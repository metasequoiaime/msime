# Agent Note: 账号面板卸载前恢复开启控件的焦点

Status: implemented

## Problem

`BottomSheet` 的注释约定关闭时回到开启控件，实际用关闭按钮或 Esc 关闭后，焦点却落在页面 `body`。恢复代码只接受焦点为空或在 `body` 的情况，但布局效果清理先于面板 DOM 移除，清理时焦点仍在面板内，条件被跳过。严格模式额外的效果周期还会把面板内的控件记录为开启控件。

## Decision

保留打开时的同步聚焦；清理时只要原开启控件仍连接，且焦点为空、在 `body` 或仍在即将卸载的面板内，就恢复开启控件。焦点已主动移到页面其他控件时保留它。

## Alternatives considered

- 改用 `useEffect` 可在 DOM 移除后看到 `body`，但会把打开时的聚焦推迟到绘制之后；保留布局效果，在清理时识别面板内的焦点即可。
- 每次关闭都无条件聚焦开启控件，规则最简单，但会覆盖异步操作或导航主动转移到面板外的焦点；保留原有外部焦点保护。

## Testing

普通模式与严格模式分别覆盖关闭按钮、Esc 的焦点恢复，以及已移到页面其他输入框的焦点保留。旧实现上四个恢复用例全部失败、两个外部焦点用例通过；修复后六项全部通过。焦点回归、账号页和动作面板三个测试文件共 81 项通过；前端类型检查与构建、改动文件 lint 和格式检查通过。生命周期顺序依据 [React 的布局效果说明](https://react.dev/reference/react/useLayoutEffect)，组件回归直接检查关闭后的 `document.activeElement`。

`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 的 Rust 工作区、Android 目标和 Apple 桥接编译通过；Linux Docker、Windows MinGW 及 HarmonyOS 原生工具链未配置，相关阶段跳过。

## Consequences

关闭面板后键盘用户可以从开启控件继续操作，避免重新寻找入口；主动移到其他页面控件的焦点保持原位。只改共享账号面板的清理条件，初始聚焦、关闭禁用和登录流程保持原有契约；没有做原生宿主设备验收。

## 旧笔记归属

按 `BottomSheet`、账号、底部面板和焦点恢复检索活跃笔记：平台设计和 Google 登录笔记分别持有视觉布局、等待及取消流程，没有共享面板的焦点生命周期决定。账号组合 Esc 修复持有输入法按键过滤，本次保持其决定，属于独立的关闭后焦点问题。
