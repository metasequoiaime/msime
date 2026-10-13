# Agent Note: Android 隐私会话结束时清理待重建标志

Status: implemented

## Problem

无痕输入在活动键盘里切换时先立即启用 Java 隐私门禁，再把 Engine 会话重建延到键盘收起。若收起同时结束了输入会话，`onFinishInput()` 会直接销毁旧会话；旧的待重建标志若留下来，会被下一次会话的 `onFinishInputView()` 误读，导致已经按新隐私设置创建的会话再次重建。

## Decision

把输入会话结束视为隐私设置重建的完成边界：`onFinishInput()` 清掉待重建标志，下一次会话按磁盘上的最新设置创建；只有活动会话在键盘收起时仍存在，才执行延后的重建。

## Alternatives considered

- 在下一次 `onStartInputView()` 里重新判断并保留标志：会让旧会话状态跨越输入框，仍可能在新会话第一次收起时重复重建。
- 在 `stop()` 里无条件清理：隐私重建本身也调用 `stop()`，会误删仍需执行的延后请求；输入会话结束是更准确的边界。

## Consequences

输入会话结束后不会把旧的隐私重建请求带到下一次会话；活动会话内收起键盘仍按原设计重建一次。无痕输入的 Java 门禁仍在切换时立即生效。

## Verification

- `bash platforms/android/check-host.sh` 通过，运行 230 个 Android JVM 冒烟并编译 91 个设备套件源文件。
- `git diff --check` 通过。
