# Agent Note: Android 导出本地备份前要求系统认证

Status: implemented

## Problem

本地备份包含输入记录和输入习惯，文件是未加密的。原来的「导出备份」按钮直接打开系统文件选择器并写出备份，已解锁设备上的其他操作者可以把这些数据导出到任意位置。

## Decision

每次点击「导出备份」先调用 AndroidX `BiometricPrompt`，允许强生物识别或锁屏凭据（`SystemAuthenticators.allowed()`，Android 9、10 上为 AndroidX 支持的弱生物识别或锁屏凭据，理由见[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)）。只有认证成功后才打开 SAF 文件创建器；用户取消、失败或设备没有可用认证器时不打开文件选择器，也不写出文件。认证期间页面按钮保持忙碌，视图销毁时取消认证并清理引用。恢复流程不增加认证，因为它只读取用户主动选择的文件，并已有完整性校验和确认对话框。

导出是否需要认证由 `BackupExportAuthenticationPolicy` 单独表达并以 smoke 覆盖，页面负责认证 API、文件选择器和生命周期衔接。

## Alternatives considered

- 在选定目标文件后再认证：可以避免无效认证，但文件选择器已经暴露了导出动作并可能创建空文件；认证紧邻导出按钮更容易保证未授权时没有文件副作用。
- 只使用指纹：没有强生物识别的设备无法导出；系统设备凭据是 AndroidX 提供的兼容后备路径。
- 导出前显示应用内密码框：需要自行保存或校验密码，不能复用设备锁屏策略，也会增加敏感输入处理，放弃。

## Consequences

导出多一个系统认证步骤，用户取消时保持原页面状态，不产生备份文件。新增 AndroidX Biometric 依赖和一条策略 smoke；认证成功后仍复用原有 SAF 和 `LocalBackup.export`，备份格式与恢复流程不变。真实 Android 模拟器验收仍需按 Android 改动门禁在合并前完成。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，包含 267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查。
