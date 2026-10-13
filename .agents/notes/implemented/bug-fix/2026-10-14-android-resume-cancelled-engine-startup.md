# Agent Note: Android 输入视图重现时恢复被取消的引擎启动

Status: implemented

## Problem

`onStartInput` 把建会话排到偏好工作线程，期间状态行显示「共享运行时准备中」。若输入视图在会话建好前收起，`finishInputViewPresentation` 推进 `engineStartGeneration`，旧任务的主线程回调会按预期丢弃；但同一编辑器再次显示时，Android 可以只调用 `onStartInputView` 而不重新调用 `onStartInput`。原来的 `onStartInputView` 没有重排任务，`session` 因而一直为零，状态行仍写着「准备中」，键盘只能直接输入。AOSP Android 16 的 `InputMethodService.startViews` 在显示视图时独立调用 `onStartInputView`，与此时序一致。这个缺口能解释 #7728 的一类症状；该报告的每次现场触发条件仍需设备证据确认。

## Decision

`onStartInputView` 在延迟刷新之前检查：旧启动代次已失效、当前没有会话、仍是同一个输入连接、编辑器需要引擎。只有这组条件同时成立，才按当前代次重排 `scheduleEngineStartup`。`pendingEngineStartup` 在失败、换编辑器和结束输入时由 `stop` 清为 `-1`，所以这些路径不会在每次显示时盲目重试。原来的 `onFinishInputView` 仍推进代次，旧异步任务不会复活。

## Alternatives considered

- **收起输入视图时不推进启动代次。** 这样原任务能在隐藏期间继续完成，改动更少；但同一代次还保护其他异步输入工作，放宽它会让视图收起后仍接受旧回调。保留既有失效边界更稳妥。
- **每次 `onStartInputView` 都重新建会话。** 能掩盖这次取消，但正常启动仍在路上时会重复排队，真正建会话失败的编辑器也会反复重试。只恢复已被视图收起取消的任务。

## Consequences

同一编辑器快速收起再显示时，失效的启动会重新排队；当前启动、活动会话、失败后已清理的启动、不同连接与直接输入字段均保持原有行为。JVM 回归测试覆盖这些边界；宿主源码检查验证服务可编译。这个修复只覆盖已确认的会话启动时序，不把 #7728 的其他可能原因视为已排除。
