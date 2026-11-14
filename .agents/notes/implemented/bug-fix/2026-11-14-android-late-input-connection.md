# Agent Note: Android 输入连接晚到时恢复引擎会话

Status: implemented

## Problem

#11142 的截图显示键盘停在“直接输入”。`onStartInput` 以当时的 `getCurrentInputConnection()` 判断是否启动引擎；部分 WebView/Compose 宿主可能在回调时尚未交出连接，输入法会把普通文本框误判为不需要引擎。键盘真正显示后连接已经可用，但原有 `onStartInputView` 只恢复已经排队的启动，不会重新判断这次输入框。

## Decision

在 `onStartInputView` 重新读取当前连接：只有已知连接为空、当前连接非空、当前没有会话且 `EditorPolicy.useEngine` 允许组字时，才按原有 `startInputRuntime` 路径补排一次启动。密码、数字、已有会话和已经有连接的输入框不触发恢复，也不改变 `EditorPolicy` 的隐私边界。

## Alternatives considered

- **放宽密码或可见密码字段，让它们统一走引擎。** 这会破坏密码框不读上下文、不学习和直输的安全契约；没有 `EditorInfo` 证据证明 #11142 是密码框前不采用。
- **在无会话时发送按键事件作为通用退路。** 这会把启动失败、密码框和真正的终端输入混在一起，且九键无法恢复中文组字；不改变既有直接输入语义。

## Consequences

连接确实晚到的普通文本框会在键盘显示阶段进入共享引擎；正常宿主的生命周期不变。若连接始终为空或运行时准备失败，仍显示原有的直接输入/未就绪状态。

## Verification

`InputViewRefreshPolicySmoke` 覆盖晚到连接恢复、已有连接、已有会话及非引擎编辑器的边界；待 Android 宿主门禁和 API 设备回归完成后再宣称报告者环境复现已闭环。
