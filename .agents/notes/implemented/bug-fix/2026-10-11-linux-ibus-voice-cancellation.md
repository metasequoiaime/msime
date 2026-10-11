# Agent Note: IBus 语音 worker 取消传播

Status: implemented

## Problem

IBus 的 `MsimeVoiceWorker` 已经在取消时置位共享原子标志，但 stream worker 调用的是没有取消探针的旧 Host API 入口。provider 不响应时，关闭语音或失去焦点仍可能等待完整的 730 秒 socket deadline。

## Decision

扩展 IBus 的 `VoiceStreamContext`，把 worker 的取消原子暴露为无异常 C 回调，并改用 Host API 的可取消 stream 入口。Host API 已把该回调转成 input-runtime 的 provider 读取探针，因此 IBus 和 Fcitx5 共享同一套 100ms 轮询取消语义。

## Alternatives considered

- 只继续发送 `voice_cancel` socket 控制请求：provider 可能不响应，stream 连接仍会占满读取 deadline。
- 在 IBus 主线程同步等待 worker：会把 provider 延迟带回输入事件循环。
- 给 worker 增加固定超时：超时返回会遗留仍在运行的 provider 线程和回调生命周期问题。

## Verification

`python3 scripts/test-linux-ibus-voice-cancellation.py` 通过。`bash platforms/linux/build-container.sh` 完成 IBus/Fcitx5 构建，79/79 个 Linux 测试通过；其中包含修改后的 `ClientEngine.cpp` 和 Fcitx5 共享 Host API ABI。

## Consequences

IBus 取消会在下一次 provider 读取轮询内结束 stream worker，仍由 `MsimeVoiceWorker` 的生命周期逻辑负责回调安全和结果丢弃，不阻塞 IBus 事件循环。
