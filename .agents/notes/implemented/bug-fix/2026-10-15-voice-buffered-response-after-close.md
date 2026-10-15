# Agent Note: 对端关闭后保留已到达的语音响应

Status: implemented

## Problem

语音 provider 可以读完请求、写出完整最终响应并立即关闭 socket。macOS 在此后设置读取超时时可能返回 `EINVAL`，接收队列仍有合法末行；原适配器在真正读取前就返回 `None`，使缺失依赖诊断和最终文字随机丢失。既有缺失依赖测试独立进程第二轮即复现，临时诊断定位到 `set_read_timeout`，插桩不进入提交。

## Decision

正常路径继续按剩余绝对预算设置每次最多 100ms 的读取超时。设置失败时临时切成非阻塞模式读取已到达字节，然后恢复阻塞模式；无可读数据或非中断错误立即结束，`Interrupted` 继续原有取消与截止时间检查。多块 UTF-8/JSON 继续使用原行缓冲，只有完整换行且未超限的行才能交给协议解析。

使用标准库的 [`UnixStream::set_nonblocking`](https://doc.rust-lang.org/std/os/unix/net/struct.UnixStream.html#method.set_nonblocking)，不增加依赖、unsafe、对外接口或协议字段。当前适配器独占连接，模式切换不会改变其它读取者。

## Existing decisions

检索语音读取、超时、`set_read_timeout` 与取消笔记。[Fcitx5 取消传播](2026-10-11-fcitx-voice-cancellation.md)继续负责取消入口、中断重试和动态库卸载；本篇部分接管其「配置失败立即结束」分支，允许安全读取已经到达的末行，原有等待预算保持。没有同一关闭竞态的实施记录。

## Alternatives considered

- 在发送请求前只配置一次 100ms 超时：可避免首次设置时对端已经关闭，但不能随剩余总预算缩短等待；每轮仍重新设置又会重现竞态。
- 忽略设置失败并继续普通读取：能保留接收队列中的响应，但无法确认当前 socket 有有效超时，可能无界阻塞。
- 用 `MSG_DONTWAIT` 对每次读取指定非阻塞：无需恢复模式，但标准库没有提供此安全入口，为此扩大底层 socket API 或增加 feature 没有必要；独占连接上标准模式切换已满足要求。

## Verification

真实 `UnixStream::pair` 写出超过三个读取块的合成 UTF-8 末行并关闭对端，旧实现稳定返回 `None`，修复后返回完整行；空响应和缺换行的响应仍拒绝。语音相关 16 项通过，既有缺失依赖测试独立进程连续 200 次通过；完整引擎 2109 项、golden 31 项、runtime 286 项通过。取消、总截止时间和中断重检测试保持通过。

## Consequences

已到达的最终响应不再因关闭后的 socket 选项失败而丢失。失败路径多两次模式设置，只排空当前可用数据，不等待后续响应；普通流式读取的超时、取消轮询和缓冲限制不变。本机验证针对 macOS Unix socket，未做设备验收。
