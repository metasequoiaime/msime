# Agent Note: Android 丢弃过期的引擎启动任务

Status: implemented

## Problem

#7728：Android 输入法在微信、QQ 和浏览器等应用中偶尔显示“共享运行时准备中 · 英文输入”，随后中文输入暂时不可用。`onStartInput` 和 `restartInput` 会推进 `engineStartGeneration`，但旧输入框已经失效的启动任务仍留在单线程 `preferencesWorker` 队列里，继续读取实时偏好和同步个人词库。频繁重启输入时，新会话必须等待这些旧任务完成，状态行就会长时间停在准备中。

## Decision

`engineStartGeneration` 作为跨线程读取的 `volatile` 代数。启动任务进入工作线程时先检查代数；实时偏好读取完成后、个人词库同步完成后再次检查。任务过期就直接结束，不再执行后续维护，也不向主线程发布启动结果。只有当前输入框的任务可以调用 `startEngineSession`，因此旧任务不能阻塞新的会话或恢复过期的英文状态。

## Alternatives considered

- **只在主线程发布结果前检查代数**：能阻止旧会话覆盖新状态，但旧任务仍会占用单线程执行实时偏好和词库同步，新会话依旧可能排队等待，无法解决“准备中”持续时间过长的问题。
- **每次重启都新建一个工作线程**：可以绕过旧队列，但会让多个偏好读取和词库同步并行争用共享目录锁，也会增加输入法进程的线程和生命周期管理负担。
- **取消 `Future`**：`ExecutorService` 的取消只能可靠阻止尚未开始的任务，已经进入 JNI 或文件锁的旧任务仍需在阶段边界检查代数；保留单线程队列并在阶段边界丢弃能覆盖两种情况。

## Consequences

新输入框的启动任务不再被已失效任务的可选维护工作拖延；过期任务在实时偏好读取后也不会继续同步词库或发布结果。已经开始的单次 native 读取无法被强制中断，最迟会在该读取返回后退出。输入法仍保持单线程访问偏好目录和词库的约束。

## Verification

`bash platforms/android/check-host.sh`、Android JVM host smoke tests、`git diff --check` 和 `npm run verify-notes` 通过。未在用户的 Redmi Android 16 设备上复现；设备验证需要该 issue 报告者的应用场景或本机 Android 16 模拟器。
