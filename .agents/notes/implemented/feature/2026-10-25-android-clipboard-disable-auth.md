# Agent Note: Android 关闭剪贴板历史前要求系统认证

Status: implemented

## Problem

Android 设置页关闭「剪贴板历史」后会清空本机记录。原实现直接写入关闭状态，拿到已解锁设备的人可以在没有再次确认的情况下删除这份本机数据。

## Decision

关闭剪贴板历史只在当前值为开、下一值为关时要求系统认证。认证使用 AndroidX `BiometricPrompt`，允许 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`，因此设备支持的强生物识别或锁屏凭据都可以完成确认。认证成功后才沿用原有偏好写入和清理路径；用户取消、认证失败或设备没有可用认证器时恢复开关状态，并保留剪贴板历史。

认证请求只持有当前视图内的一次待写值。视图销毁时取消请求并清理待写状态；回调在视图已经离开时不再显示提示或写入设置，避免异步认证结果触碰失效的 Fragment 上下文。

策略判断单独放在 `ClipboardHistoryAuthenticationPolicy`，由主机 smoke 覆盖四种开关转移，页面负责系统 API 和生命周期。

## Alternatives considered

- 只使用指纹认证：提示流程简单，但没有生物识别的设备会无法关闭设置；使用 `DEVICE_CREDENTIAL` 作为系统提供的后备方式可以覆盖这类设备。
- 先写入关闭状态，再认证并在失败时回滚：最强理由是 UI 可以立即响应，但清理动作可能已由键盘读到关闭状态触发，回滚不能可靠恢复已经删除的历史，因此不采用。
- 自定义密码对话框：能统一文案，但会重复实现系统认证、无法复用设备锁屏策略，也会增加敏感输入处理面，放弃。

## Consequences

关闭操作多一个系统认证步骤，用户取消时设置保持开启；认证不可用时不会静默删除历史。新增 AndroidX Biometric 依赖和一条纯 Java 策略测试，页面的认证回调需要继续遵守 Fragment 视图生命周期。认证成功后仍复用原有 `HostTask` 写入和 `ClipboardHistoryRetentionPolicy` 清理契约，没有改变存储格式。

验证：`compileFullDebugJavaWithJavac` 在 JDK 21 下成功；`ANDROID_SDK_ROOT=/Users/xiaomo/Library/Android/sdk bash platforms/android/check-host.sh` 成功，267 项 Android JVM smoke、240 项设备套件源码编译和服务 Java/API 检查通过。真实模拟器验收仍需在合并前按 Android 改动门禁完成。
