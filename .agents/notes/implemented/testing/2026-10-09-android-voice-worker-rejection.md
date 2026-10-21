# Agent Note: 键盘语音工作线程拒绝时释放识别器

Status: implemented

## Problem

键盘内语音识别会先创建本地或流式识别器，再把识别任务提交到单线程执行器。执行器在输入法服务退出的竞态中可能拒绝任务；原路径只恢复键区并返回 `false`，没有取消已经创建的识别器，随后回退到识别窗口时旧识别器可能继续持有麦克风。

## Decision

工作线程提交被拒绝时统一走现有 `cancel()` 生命周期路径。它会使当前代失效、取消本地或流式识别器、清空引用、释放系统识别器、取消当前润色器并恢复键区，然后让调用方继续原有的窗口回退。

## Alternatives considered

- 只调用 `local.cancel()` 或 `streaming.cancel()`：遗漏系统识别器和共享的代数、界面清理逻辑。
- 继续只调用 `dismiss()`：只改变视图，不能释放已经创建的识别器。
- 改变回退策略：扩大行为变化范围，且不能修复资源释放根因。

## Verification

新增 Android voice project contract，先在旧实现上确认失败，再在修复后通过；随后运行 Android host checks、`scripts/verify-local.sh --quick`、笔记检查、格式与差异检查。

## Related decisions

[键盘润色取消](../bug-fix/2026-10-21-android-keyboard-voice-polish-cancellation.md)补足 `cancel()` 对润色连接的管理；当前任务捕获旧实例，取消后新任务使用新实例。

## Consequences

线程提交失败时不会遗留麦克风或识别器；正常提交、完成和用户取消路径继续复用原有生命周期逻辑。
