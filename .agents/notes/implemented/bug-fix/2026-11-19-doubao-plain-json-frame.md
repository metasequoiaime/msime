# Agent Note: 豆包流式识别接受未压缩 JSON 回包

Status: implemented

## Problem

豆包 SeedASR 2.0 的流式 WebSocket 会返回第三字节为 `0x10` 的 JSON 未压缩回包，而现有 `decode_json_frame()` 只接受 `0x11` 并始终按 gzip 解码。设置页测试和实际语音识别因此把有效回包判为无效。错误帧已经允许未压缩编码，这里的 JSON 结果帧却仍保留了 gzip 的硬性判断。

## Decision

结果帧只接受 JSON 序列化的 `0x10` 和 gzip 压缩的 `0x11`。`0x10` 直接读取声明长度范围内的 JSON 字节，`0x11` 沿用现有 gzip 解码、完整消费和解压上限检查。协议版本、消息类型、帧长度、声明长度和 1 MiB 载荷上限继续生效，其他序列化或压缩编码仍拒绝。

## Alternatives considered

- 只修改请求帧，要求服务端返回 gzip：报告中的真实回包已经证明服务端仍会返回 `0x10`，客户端无法依赖请求侧声明改变结果编码。
- 对所有第三字节都按原始 JSON 读取：可以接住当前服务，但会把未知编码当作 JSON，失去协议边界检查，也可能让压缩数据进入上层。
- 在每个宿主分别兼容未压缩回包：会让 macOS、Windows、Android 和设置页测试各自维护协议分支，重复实现且容易产生行为差异。本次在共享 Rust 解码器统一处理。

## Consequences

- SeedASR 2.0 返回未压缩 JSON 时，设置页测试和实际流式识别都能继续解析结果。
- gzip 回包和既有错误处理保持原样，未知编码、截断帧、长度不一致、拼接 gzip 和超过上限的载荷仍会被拒绝。
- 测试使用合成 JSON，不需要真实账号、语音或网络。真实 SeedASR 服务验收仍需报告者或维护者在 macOS 上补做。

## Verification

- `cargo test -p msime-client-providers voice::doubao_frame` 覆盖 gzip 和未压缩 JSON、未知编码、长度边界和损坏 gzip。
- `git diff --check` 通过。
- 现有解码器调用方继续使用同一返回结构，无需修改宿主协议。

## 既有笔记审计

- [Android 豆包流式边录边读回包](2026-10-29-android-doubao-streaming-receive.md) 部分相关，继续复用共享帧解码和错误码边界，本次只补 JSON 结果的编码兼容。
- #10904 与 #11270 是同一服务协议族的既有问题记录，本次不改变错误帧和 Android 收发线程的既有决定。
