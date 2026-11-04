# Agent Note: Android 系统返回键不再等待会话销毁冲刷

Status: implemented

## Problem

Android 输入法服务在输入框失焦时会销毁共享引擎会话。`msime_client_destroy` 除了释放当前会话，还会同步调用 `msime_engine::flush_personal_learning`，把进程里所有个人上下文模型的延迟写入一次性落盘。这个调用发生在 `:ime` 的输入线程上；低端设备在系统三键导航栏按返回键时，应用动画会等到这次磁盘写入结束，表现为约数秒的卡顿（#10553）。

## Decision

Android 的 `msime_client_destroy` 不再在调用线程上做进程级个人学习冲刷，只保留会话自身销毁时对本日志的写出。`MSIMEInputService` 在会话销毁后把 `msime_client_flush_all` 排到已有的统计工作线程；服务销毁时同一队列仍会完成最后一次冲刷。桌面宿主保持原来的同步语义，避免改变可能立即退出的宿主的持久性边界。

这条路径与[共享 Rust 层在没有变化时不再整份读写和 fsync](2026-10-27-shared-rust-idle-io-hot-path.md)中的移动端后台写入约定相同：输入线程只做必要的会话边界工作，批量落盘交给后台线程。

## Alternatives considered

- **保留同步冲刷，只把 `onFinishInput` 延迟到下一帧**：能避开返回键回调的直接阻塞，但磁盘工作仍会占用输入法主线程并打断后续动画；没有采用。
- **销毁后新建一个线程执行冲刷**：实现较小，但每次切换输入框都会创建线程，无法复用现有统计线程的串行顺序；没有采用。
- **Android 完全不再冲刷个人学习**：返回键最顺畅，但服务被系统回收时会扩大未写入窗口；没有采用，改为在统计工作线程和服务销毁路径补一次 `flush_all`。

## Consequences

- 系统返回键和应用切换不再同步等待所有个人上下文模型的磁盘写入，Android 的导航动画不被这段慢 I/O 卡住。
- Android 个人学习冲刷与统计写出共用一个后台队列，提交顺序稳定；服务销毁时仍会排队完成最终冲刷。
- Android 在后台线程结束前被系统强杀时，进程级其他日志仍可能等待引擎自身约两秒的延迟写入；这是移动端已有的后台持久性窗口，桌面宿主不受影响。

## Verification

- `platforms/android/check-host.sh`：验证 Java/JNI 声明、宿主源码和 Android 交叉编译接线。
- `scripts/test-android-jni-names.py`：179 个 Java/JNI 方法与导出清单一致。
- API 28 arm64 模拟器：`build-core-test.sh arm64-v8a` 已完成，四个 client crate 与 Engine 维护租约测试共通过 40+746+304+1475+11 项；完整 `smoke.sh --core` 因本地未准备锁定词库资源、没有可安装 APK，未能运行。
- 系统返回键的真机卡顿仍需在报告者的 vivo V2046A（Android 11）上复验。
