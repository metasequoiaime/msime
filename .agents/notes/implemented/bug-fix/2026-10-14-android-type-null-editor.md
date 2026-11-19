# Agent Note: Android TYPE_NULL 自定义输入框启用组字

Status: implemented

## Problem

#7755 的抖音直播间弹幕框把 `EditorInfo.inputType` 报为 `TYPE_NULL`，宿主因此把它当成非文本框，只显示「直接输入」。九键的 `character()` 在没有会话时直接返回，26 键的 `commitText()` 也可能被这类 dummy `InputConnection` 丢弃，结果中文和英文都无法输入。

## Decision

`EditorPolicy.useEngine(0)` 现在返回 true，把 `TYPE_NULL` 视为可组字的自定义文本输入框；密码框和数字框仍按其非零类型排除。这样宿主会为这类输入框建立共享引擎会话，九键与 26 键继续走正常的组字和提交路径。

## Alternatives considered

- **只在 `session == 0` 时给每个按键发送 `sendKeyChar`。** 最强理由是改动小，也能覆盖终端类 `TYPE_NULL` 编辑器；但九键的数字键本身不是拼音字母，无法恢复中文组字，并且会让同一输入框在引擎启动失败时有两套不一致的行为，因此没有采用。
- **继续把 `TYPE_NULL` 当成直接输入。** 最强理由是 Android 约定里它常用于终端和只收按键事件的控件；但 #7755 的聊天控件明确需要中文组字，现状已经连英文也无法提交，不能满足真实聊天输入。

## Consequences

TYPE_NULL 自定义控件会多一次引擎启动和资源准备，并依赖其 `InputConnection` 接受组字提交；终端类控件仍可切到英文模式发送字面字符。密码和数字输入框不受影响。JVM `EditorSmoke` 固定了 `TYPE_NULL` 应走引擎的契约。

只认整数 0 不够：抖音直播评论框后来报 `0x20000`（类别为 0、只带多行 flag），同样落入「直接输入」。规则已扩展为按类别排除已知非文字框，见 [没有输入类别的输入框按文字框组字](2026-11-18-android-null-class-editor.md)。
