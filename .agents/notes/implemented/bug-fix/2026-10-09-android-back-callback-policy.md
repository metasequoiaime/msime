# Agent Note: Android 返回回调使用专用启用接口

Status: implemented

## Problem

Android 导航策略重构把 `OnBackPressedCallback` 传给只接受 `View` 的 `ViewPolicy.setEnabled`，导致完整版本的 Java 编译失败，设置页和引导页无法打包。

## Decision

保留 `ViewPolicy` 用于真正的 `View`，返回回调恢复使用其自身的 `setEnabled` 接口。

## Alternatives considered

- 扩展 `ViewPolicy` 接受回调：会让 View 专用策略承担另一种生命周期对象，增加错误调用面。
- 强制转换为 `View`：类型不兼容且运行时不安全。

## Consequences

返回按钮状态行为保持不变，Android 完整 Java 编译重新通过。

## Verification

`bash platforms/android/check-host.sh` 通过；该门禁编译 55 个设备套件源文件并运行 187 个 JVM 冒烟用例。
