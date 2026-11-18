# Agent Note: macOS 面板输入帧读取等待

Status: implemented

## Problem

macOS 的面板输入 socket 监听端是非阻塞的。Darwin 接受连接时可能继承监听 socket 的 `O_NONBLOCK`，大帧在下一段数据尚未到达时会让 `recv` 返回 `EAGAIN`。原来的帧读取把这个结果当成连接失败，导致 12000 字节剪贴板帧在 Sanitizers 下收到零字节响应。

## Decision

`DesktopInputSession` 在读取帧头、正文和帧尾时遇到 `EAGAIN` 或 `EWOULDBLOCK`，先用 `poll` 等待可读、挂起或错误事件，再继续读取；等待沿用 3 秒的 socket 接收预算。这样即使连接仍带有非阻塞标志，分片帧也会等到后续字节或对端 EOF，超时后仍按读取失败处理。

## Alternatives considered

- 只在 `accept` 后清除 `O_NONBLOCK`：这是现有路径，但 Sanitizers 的大帧回归仍暴露出读取阶段可能先遇到 `EAGAIN`，单靠这一步不能覆盖实际连接状态。
- 放宽测试的响应断言或增加整体等待：会掩盖生产读取失败，无法保证面板协议收到完整正文。

## Consequences

分片的大剪贴板帧可以在 Darwin socket 状态短暂非阻塞时完整读取，候选帧和现有拒绝路径继续使用同一个超时边界。恶意对端没有后续数据时最多等待一次接收预算，仍会被拒绝。现有 `desktop-input-session` 回归覆盖 12000 字节 UTF-8 剪贴板帧，Sanitizers job 用这条路径验证修复。
