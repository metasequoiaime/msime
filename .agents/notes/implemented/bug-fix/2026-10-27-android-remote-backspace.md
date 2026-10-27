# Agent Note: Android 退格兼容拒绝周边删除的远程编辑器

Status: implemented

## Problem

部分远程控制类编辑器不接受 `deleteSurroundingTextInCodePoints`，但可以处理输入法发出的原始 `KEYCODE_DEL`。Android 键盘此前在前一个调用失败后直接结束，退格看起来没有作用。

## Decision

删除键在确认没有选区、读取到光标前文本且周边删除明确返回失败时，作废本地回声和快速删除状态，然后发送一组受保护的原始 `KEYCODE_DEL` 按下/抬起事件。普通编辑器成功完成周边删除时不发送回退事件，因此不会重复删除。

## Alternatives considered

- **所有删除都同时发送周边删除和原始退格**：会让正常编辑器重复删除，也会破坏选区和密码框行为，因此不采用。
- **只继续调用 `deleteSurroundingTextInCodePoints`**：无法兼容拒绝该 API 但支持键事件的远程编辑器，因此不采用。

## Consequences

远程编辑器可以通过标准退格事件完成删除，普通编辑器的成功路径不变。失败回退无法在本机模拟真实远程应用协议，设备测试使用合成 `InputConnection` 验证只发送一次按下/抬起对；报告者仍需在 UU 远程控制环境复验。
