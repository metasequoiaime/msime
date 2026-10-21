# Agent Note: 键盘语音取消时释放润色请求

Status: implemented

## Problem

`ImeVoiceEntry` 的本地、流式和系统识别路径原先在唯一的语音工作线程里临时创建 `VoicePolisher`。取消只结束识别器并推进代次，不能中断已经进入响应等待的润色。新一次录音排在旧请求之后，可能等到 60 秒读取超时才能开始。识别窗口已持有润色器并在取消时调用 `cancel()`，键盘内路径缺少同样的生命周期归属。

## Decision

键盘持有当前的润色器，后台任务在提交前捕获它。取消时调用旧实例的 `cancel()`，再替换成新实例，既中断正在等待的连接，也让尚未执行的旧任务在上传前退出。新识别捕获新实例，既有结果代次守卫继续负责拒绝过期上屏。聆听视图被外部移除时，先清空视图引用，再复用同一 `cancel()`，避免键区重建留下旧润色请求。

## Existing notes audit

- [语音工作线程拒绝](../../implemented/testing/2026-10-09-android-voice-worker-rejection.md)：部分重叠，继续规定提交失败复用 `cancel()`；本篇补足它管理的润色资源。
- [语音贡献隐私代次](../../implemented/bug-fix/2026-10-11-android-voice-contribution-privacy-generation.md)与[贡献撤回](../../implemented/bug-fix/2026-10-11-android-voice-contribution-consent.md)：无关，管理独立的可选贡献任务。
- [不完整润色回复](../../implemented/bug-fix/2026-10-19-voice-polish-incomplete-reply.md)：无关，继续管理回复接受条件。
- [IBus 取消传播](../../implemented/bug-fix/2026-10-11-linux-ibus-voice-cancellation.md)、[Fcitx5 取消传播](../../implemented/bug-fix/2026-10-11-fcitx-voice-cancellation.md)、[AI 动作独立完成](../../implemented/bug-fix/2026-10-15-ai-assistant-overlapping-actions.md)：无关，宿主与请求归属不同。Windows 原生语音页、设置桥与 macOS 对齐笔记，以及 iOS 可见请求代次笔记属于其它宿主，与这次键盘内润色资源管理无关；没有同一决定的活跃提案。

## Alternatives considered

- 仅在结果投递前检查代次：能防止过期文字上屏，且已有这项守卫；无法释放被 HTTP 等待占用的工作线程。
- 取消整个执行器或只中断 Future：能处理排队任务，但此队列还负责统计和贡献；线程中断也不能代替润色器已有的连接取消入口。
- 后台任务每次读取可变的当前润色器：省去捕获局部变量，但取消后旧任务可能拿到新实例并继续发请求。

## Verification

JVM 回归调用真实的 `ImeVoiceEntry.cancel()`、`VoicePolisher.polish()` 和单线程执行器验证连接取消、队列继续执行、取消前已排队的旧实例不上传，以及后续实例仍可请求。传输使用合成阻塞连接，不访问网络或私人数据。旧实现因缺少所有者失败；修复后通过。移除取消调用、保留其余修复的变异版本仍因活动连接未中断失败，恢复后通过，证明回归同时检查实际取消行为。独立审查覆盖两条产品任务的实例捕获及视图卸载入口。`bash platforms/android/check-host.sh` 通过 251 项 JVM smoke 和 197 份设备套件源码编译；`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与 `git diff --check` 通过。

## Consequences

取消和键区卸载会释放旧润色占用的队列，新识别使用未取消的实例。回归不启动真实录音或 Android 网络栈；连接中断沿用 `VoicePolisher` 已有实现，不改变其平台行为。每次取消额外分配一个轻量润色器；它在真正发请求前不持有连接。
