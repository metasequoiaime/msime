# Agent Note: Android 统计页面进入时要求系统认证

Status: implemented

## Problem

统计页面展示本机输入习惯、按键分布和成就等聚合数据。原页面进入后直接读取并显示，离开后再次进入也沿用页面状态，没有系统级身份确认。

## Decision

统计页每次视图进入先调用 AndroidX `BiometricPrompt`，允许 `BIOMETRIC_STRONG | DEVICE_CREDENTIAL`；认证成功后才读取和显示统计数据。认证取消、失败或不可用时不加载统计。视图销毁或离开页面后清除授权，下一次进入重新认证；认证请求随视图销毁取消，迟到回调不启动加载。

认证状态由 `StatisticsAuthenticationPolicy` 表达并以 smoke 覆盖，页面负责生命周期、系统认证和原有加载流程。

## 生命周期接入

统计页继承 `HomeTabFragment`，使用它的 `onBecameVisible()` 生命周期回调接入认证；详情页的 `ReloadingDetailPage` 仍通过共享 `onVisible()` 钩子统一重载。这样不会让统计页误用详情页的生命周期合同。
## Alternatives considered

- 只在首次打开认证并永久保持：离开页面后设备可能交给其他人，不能满足立即锁定要求。
- 认证后继续后台预加载：会在认证前读取敏感统计，违背访问边界，因此加载只放在成功回调之后。
- 应用内密码对话框：不能复用设备锁屏策略，且会增加密码处理，放弃。

## Consequences

进入统计页多一个系统认证步骤，离开后再次进入需重新确认；统计计算、上传和清空操作仍复用原有实现。新增 Biometric 依赖和生命周期状态。真实 Android 模拟器验收仍需按 Android 改动门禁在合并前完成。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，主机 smoke 和设备套件源码检查通过。

## Alternatives considered

无其他实际方案。
