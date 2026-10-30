# Agent Note: Linux 服务在请求线程启动期间安全响应停止信号

Status: implemented

## Problem

全平台崩溃采集 PR 的 Debian 10 amd64 CI `41123740534` 在语音降级用例等待 `SIGTERM` 退出时超时。Python 3.7 的主线程可能正在 `Thread.start` 内持有非重入的 `_active_limbo_lock`，原信号处理器再启动关闭线程会重复获取同一把锁。隔离、断网的官方 Python 3.7 容器在真实服务持锁语句处送达信号，稳定取得重复 `Thread.start` 的阻塞栈；普通复跑通过无法排除此竞态。在线服务使用同一停止模式。

## Decision

两个 Unix 服务复用共享停止基类。信号处理器只赋值停止标志，在 `socketserver` 的公开 `service_actions` 安全点退出服务循环，随后执行原有会话、子进程、套接字和锁清理。保持正常退出语义，避免退出异常进入产品崩溃采集；不延长十秒测试期限。

回归在真实请求线程临界区送达 `SIGTERM`，要求注入发生且服务正常退出。先保留原实现失败，再以同一回归验证修补。

## Related notes

[套接字就绪](../../implemented/bug-fix/2026-10-09-linux-voice-provider-socket-ready.md)覆盖 bind/listen 窗口，与本次线程重入锁不同，保留原决定。[原生崩溃采集](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)覆盖正常退出和辅助进程故障归属，本修复补齐其验证遇到的停止竞态，不修改授权或后台协议。

## Alternatives considered

- 信号中抛异常：可立即打断循环，但异常可能落在 Python 内部 weakref 回调而被吞，原代码已记录这一失败模式。
- 提前启动轮询关闭线程：避免在信号内建线程，但增加每个服务的常驻线程和定时唤醒；现有服务循环已有公开安全点。
- 延长超时或只重跑 CI：不能解除同线程重入锁，无法修复实际挂起。

## Verification

隔离、断网的官方 `python:3.7-slim-buster` 容器中，新回归叠加原 HEAD 的三个产品文件时，语音和在线两个子用例均在原十秒期限超时。修补后相同注入和四个既有降级用例全部通过，五项合计约一秒。原失败栈与独立探针第一次配置前提失败均保留在 `target/legacy-stop-race/`，没有通过放宽期限或跳过注入改写失败结果。同一源码快照的完整 `scripts/verify-local.sh --quick` 与笔记门禁均退出 0，验证前后全部 12,665 条源码的字节与执行位一致；Linux 原生 119/119 和设置壳、仓库契约、Rust workspace 编译通过，原测试未放宽。缺少 SDK/原生工具的 Android、wasm、鸿蒙、Apple、Windows x86 语法、可移植 Windows 与完整 Windows 阶段明确跳过。远端恢复原干净 e746 源码并移除租约；证据为 `target/provider-stop-quick-evidence/`，Python 3.7 定向结果与 quick 分开记录。

## Consequences

两个服务的停止处理器只设置标志，避免旧 Python 线程锁重入；异常在公开服务循环安全点被明确捕获，随后继续原有会话、子进程、socket 与锁清理，不进入崩溃采集。没有新增轮询线程或依赖。

安全点响应受现有服务循环轮询间隔限制；停止后仍须等原有已界定的录音和音频恢复清理完成。
