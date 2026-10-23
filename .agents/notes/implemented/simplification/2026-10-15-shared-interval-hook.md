# Agent Note: 共享界面定时器复用公共间隔 Hook

Status: implemented

## Problem

验证码重发倒计时、鸿蒙输入法切换引导和 macOS 输入法列表重读分别手写 `setInterval`、回调和卸载清理。它们都需要在停用时取消定时器，但没有共享间隔生命周期；回调变化时也容易误把旧闭包留在定时器里。

## Decision

在 `packages/ui/src/core/use-interval.ts` 提供 `useInterval(callback, delayMs)`，接口与现有 `useTimeout` 对齐：`delayMs` 为 `null` 时不注册，间隔变化时清理旧定时器，回调通过 `useLatestRef` 读取最新值。调用方继续各自决定何时启用、何时停止以及回调内部的代次和状态更新。

统计总览的 `useTypingStatisticsState`（`packages/ui/src/settings/use-typing-statistics-state.ts`，见[读取状态边界](2026-10-23-typing-statistics-state.md)）和云词库应用页的 `useCloudDictionaryApplyPanel`（`packages/ui/src/keyboard/use-cloud-dictionary-apply-panel.ts`）的轮询与客户端代次在同一个 effect 中重建，因此保留各自的 effect，并复用同模块的 `subscribeToInterval(callback, delayMs)`。它只注册 interval 并返回清理函数，`useInterval` 自身也复用此入口。统计页仍先读取，再订阅窗口恢复和轮询；云词库仍先使旧代次失效并读取，卸载时先使代次失效再停止轮询。`overviewPollMs` 与 2000 毫秒频率、effect 依赖和请求闭包不变。

## Alternatives considered

- 两处有业务 effect 的轮询直接换成 `useInterval`：可以只保留 hook 调用，但该 hook 只在间隔变化时重建定时器，客户端变化后会沿用旧 tick 相位，改变原有时序，因此改为复用同步订阅入口。
- 每个页面继续保留局部 `setInterval`：改动最小，但会继续复制注册和清理规则。
- 把倒计时、输入法引导和列表重读的状态也收进一个业务 hook：可以减少页面代码，但会把互不相关的启用条件、宿主刷新和验证码代次绑定在一起，破坏各页面的状态边界。

## Verification

同步订阅测试先失败后通过，覆盖 400 毫秒边界、订阅时不立即回调与重复清理；间隔 Hook、统计页及云词库生命周期共四文件四十八项通过。hook 测试覆盖正常 tick、卸载清理、最新回调和 `null` 停用；验证码、输入法切换引导及 macOS 输入法列表相关回归测试通过，桌面 TypeScript 检查通过。完整前端门禁和 CI 继续负责其余共享 UI 文件。

## Consequences

共享 UI 定时器统一使用同一套生命周期，回调更新不会重启不必要的 interval。业务 hook 仍负责时间戳计算、请求代次和宿主状态；公共 hook 不替代平台原生定时器，也不改变轮询频率。验证码倒计时的行为边界由[验证码冷却笔记](../bug-fix/2026-10-14-account-verification-cooldown.md)继续持有。

## Existing notes audit

按 `setInterval`、`useInterval`、`useTimeout`、倒计时和轮询检索活跃笔记。验证码冷却笔记记录共享时钟的业务决定，本篇只记录定时器注册清理的 UI 抽取；原生候选、IBus 和 macOS 输入法的定时器笔记属于平台生命周期，未合并。

## macOS 调用方覆盖

macOS 输入源启动提示重查与首次安装页的模拟进度同样使用 `useInterval`。输入源仍只在等待用户添加输入法或处理系统安装副本时每 3 秒重查；首次安装仍只在 `installing` 阶段每 90 毫秒推进，等待真实结果和最短展示时间后才完成。首次读取、请求代次、提示关闭、进度上限以及完成页的延迟切换仍由调用方控制。客户端变化时定时器读取最新回调，继续按已启用的间隔运行。原有安装页交互、输入源重查与晚到回答测试覆盖业务条件；复用契约检查阻止这两处重新手写间隔注册与清理。
