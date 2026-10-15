# Agent Note: Android 编辑器连接断开时保护输入法进程

Status: implemented

## Problem

`MSIMEInputService` 通过 `EditorBridge.Sink` 调用 Android `InputConnection`。编辑器在两次跨进程调用之间失去焦点或销毁时，`begin`、`compose`、`finish`、`end` 等操作可能抛出 `RuntimeException`。原来的 `EditorBridge` 没有捕获这些异常，编辑器拒绝本次写入后，清理组字或结束批量编辑还可能把异常带出输入法主线程，导致输入法进程崩溃。已有的 [输入连接失败后自动重建](2026-10-15-android-input-connection-recovery.md) 只能处理已经进入 `fail` 的错误，不能覆盖这个更早的异常边界。

## Decision

`EditorBridge` 把 `Sink` 的运行时异常视为编辑失败：`apply` 返回 `false`，清理路径 `abandon`、`discard` 和 `settle` 尽力结束但不再向外抛出，批量编辑的 `begin` 与 `end` 也采用同一保护。这样宿主仍可按现有流程把失败归类、销毁失效会话并尝试一次恢复，不把断开的编辑器升级为输入法进程崩溃。

## Alternatives considered

- 只在 `MSIMEInputService.sink` 的每个 Android 调用点分别捕获异常：可以局部止血，但 `EditorBridge` 的清理和其他宿主仍会重复实现同一边界，容易漏掉 `end` 或新增的编辑操作。
- 在 `fail` 中包住 `stop(false)`：能遮住一次清理异常，但正常的编辑操作仍会把断开连接的异常带出 `apply`，而且 `discard`、`settle` 和失焦清理仍各有未保护路径。
- 让异常继续向上传播并依赖系统重启输入法：会丢失当前输入会话，且把编辑器的瞬态断开扩大成用户可见的输入法故障。

## Verification

- `EditorSmoke` 新增合成断开 `Sink`；修复前 `EditorBridge.apply` 直接抛出 `IllegalStateException`，修复后返回失败并完成 Android host contract 的全部 231 项 JVM 冒烟。
- 该保护只捕获 `RuntimeException`，不吞掉 `Error`；现有成功、拒写、组字清理、日语定稿和笔画清理顺序测试保持通过。

## Consequences

编辑器连接在操作中断开时，本次编辑会失败，调用方可以沿用既有的恢复和“仅直接输入”降级；失焦清理可能无法完成，但不会杀死输入法进程。清理代码多了一层统一的尽力而为语义，后续新增 `Sink` 操作必须继续在 `EditorBridge` 边界内执行。
