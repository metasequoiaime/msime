# Agent Note: Android 统计页面进入时要求系统认证

Status: implemented

## Problem

统计页面展示本机输入习惯、按键分布和成就等聚合数据。原页面进入后直接读取并显示，离开后再次进入也沿用页面状态，没有系统级身份确认。

## Decision

本篇的「未配置系统认证也阻止查看」由[统计页按设备认证配置放行](../bug-fix/2026-10-25-android-statistics-auth-availability.md)取代；已配置凭据时的认证与生命周期约束仍生效。

统计页每次视图进入先调用 AndroidX `BiometricPrompt`，允许强生物识别或锁屏凭据（`SystemAuthenticators.allowed()`，Android 9、10 上为 AndroidX 支持的弱生物识别或锁屏凭据，理由见[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)）；认证成功后才读取和显示统计数据。认证取消、失败或不可用时不加载统计。视图销毁或离开页面后清除授权，下一次进入重新认证；回到前台时走同一条可见生命周期，确保认证后的读取失败也能在下一次进入重试。唯一的例外是暂停时认证还开着（Android 9、10 的锁屏凭据确认是独立 Activity）：那次可见是否重新认证交给 `AuthenticationPauseTracker` 按认证结果决定，成功回调自己会读，用户自己取消不再弹，认证窗口开着时离开而被系统取消仍按重新进入处理，理由见[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)。认证请求随视图销毁取消，迟到回调不启动加载。

认证状态由 `StatisticsAuthenticationPolicy` 表达并以 smoke 覆盖，页面负责生命周期、系统认证和原有加载流程。

## 生命周期接入

统计页继承 `HomeTabFragment`，使用它的 `onBecameVisible()` 生命周期回调接入认证；详情页的 `ReloadingDetailPage` 仍通过共享 `onVisible()` 钩子统一重载。这样不会让统计页误用详情页的生命周期合同。
## Alternatives considered

- 只在首次打开认证并永久保持：离开页面后设备可能交给其他人，不能满足立即锁定要求。
- 认证后继续后台预加载：会在认证前读取敏感统计，违背访问边界，因此加载只放在成功回调之后。
- 应用内密码对话框：不能复用设备锁屏策略，且会增加密码处理，放弃。

## Consequences

进入统计页多一个系统认证步骤，离开后再次进入需重新确认；统计计算、上传和清空操作仍复用原有实现。新增 Biometric 依赖和生命周期状态。真实 Android 模拟器验收仍需按 Android 改动门禁在合并前完成。

验证：JDK 21 下 `compileFullDebugJavaWithJavac` 成功；`platforms/android/check-host.sh` 成功，主机 smoke 和设备套件源码检查通过；统计页回前台契约（`scripts/test-android-statistics-reload-lifecycle.py`）要求 `onBecameVisible()` 除跨越暂停的认证外都重新认证读取，并要求认证成功回调自己读取；`AuthenticationPauseTrackerSmoke` 覆盖结果在可见之前和之后送达、成功、用户取消和系统取消各种组合。

## Alternatives considered

无其他实际方案。
