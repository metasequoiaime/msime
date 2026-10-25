# Agent Note: Android 重置所有设置前要求系统认证

Status: implemented

## Problem

开发者选项中的「重置所有设置」会改写共享偏好和 Android 本地设置。原流程只有确认对话框，已解锁设备上的其他操作者可以直接执行不可逆的设置重置。

## Decision

在确认对话框的「重置」动作之后、执行任何写入之前调用 AndroidX `BiometricPrompt`，允许强生物识别或锁屏凭据（`SystemAuthenticators.allowed()`，Android 9、10 上为 AndroidX 支持的弱生物识别或锁屏凭据，理由见[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)）。认证成功后才进入原有异步重置流程；取消、失败或认证不可用时不改动设置。视图销毁时取消认证请求，回调只在仍有视图时启动重置。

认证规则单独放在 `SettingsResetAuthenticationPolicy`，由主机 smoke 覆盖；页面负责认证 API 和已有 `NativeClient.restoreDefaultPreferences`、`AndroidLocalSettings.restoreDefaults` 流程。

## Alternatives considered

- 仅依靠现有确认对话框：它防误触但不能证明操作者身份，无法满足安全增强需求。
- 只使用指纹：没有强生物识别的设备无法重置；设备锁屏凭据是系统可用的后备认证方式。
- 在异步重置完成后再认证：设置已经被改写，失败时无法可靠恢复，因此不采用。

## Consequences

重置操作增加系统认证步骤，认证失败时保留原设置。新增 Biometric 依赖和策略 smoke，重置的修订号、云同步脏标记、本地设置回滚和词库不受影响。真实 Android 模拟器验收仍需按 Android 改动门禁在合并前完成。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查通过。
