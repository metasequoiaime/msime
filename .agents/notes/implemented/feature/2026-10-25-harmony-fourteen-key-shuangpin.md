# Agent Note: HarmonyOS 双拼 14/18 键

Status: implemented

## Problem

HarmonyOS 已经支持全拼 14 键和 18 键，共享引擎也支持双拼的字母组码网格，但宿主只在 `quanpin` 下打开网格。设置页因此只显示全拼的 14/18 键，双拼用户无法使用窄屏组码键面。

## Decision

HarmonyOS 手机和平板沿用共享偏好 `touch_shuangpin_layout`，提供双拼 26/14/18 键布局。设置页切换这个布局只写 `touch_shuangpin_layout`，保留当前 `scheme`、`shuangpin_profile` 和启用列表；切换双拼方案时布局也保持不变。双拼 14 键使用 C ABI 网格值 `2`，双拼 18 键使用网格值 `5`，全拼 18 键继续使用网格值 `4`。

宿主会话从偏好读取双拼布局，创建、重建和获得焦点时重新设置网格；`KeyboardView` 对双拼复用现有 `FourteenKeyLayout` / `EighteenKeyLayout`，并在 L 键显示 `L;`，让微软双拼保留分号入口。英文、密码框、网址框、本地模式和 2in1 继续画 26 键，统计仍按当前双拼 profile 记录。

设置预览为双拼 14/18 键绘制独立的 `L;` 键面。全拼、双拼以外的方案不继承拼音组码网格。

本决定扩展了[全拼 14 键](2026-10-10-fourteen-key.md)中“只给全拼”的 HarmonyOS 范围；Android 和 iOS 的独立实现分别见[Android 14 键双拼](2026-10-23-android-fourteen-key-shuangpin.md)和[iOS 14 键双拼](2026-10-24-ios-fourteen-key-shuangpin.md)。

## Alternatives considered

- **继续只支持全拼。** 改动最少，但共享引擎和 Android/iOS 已有双拼组码能力，HarmonyOS 会继续出现同一套设置在不同移动宿主缺失的缺口，因此不采用。
- **把双拼 14/18 键写进 `touch_keyboard_layout`。** 这样能复用全拼方案选择器，但会把“当前双拼方案”和“键盘几何”绑定到同一字段，切换小鹤、自然码或微软时会破坏用户的布局选择，也无法与 Android/iOS 的共享偏好契约对齐，因此不采用。
- **双拼 18 键复用全拼网格值 `4`。** 可以少加一个常量，但引擎用网格值区分全拼和双拼 18 键；复用会把双拼组码按全拼解码，因此使用 C ABI 规定的值 `5`。

## Consequences

双拼用户在鸿蒙上可以独立选择 14/18 键，并在切换双拼 profile、重启会话和回到中文时保留选择。键表、长按和候选交互继续与全拼键面共享，减少了重复实现；代价是 HarmonyOS 需要同时维护共享偏好字段、宿主网格门控和设置预览三处投影。旧版鸿蒙客户端不认识新偏好值时会按其既有兼容策略回落到 26 键，当前版本的本地选择不会改变其他方案或桌面键盘。

## Verification

- `bash platforms/harmony/tests/run.sh`
- `pnpm --filter @msime/harmony typecheck`
- `pnpm --filter @msime/desktop typecheck`
- 真机签名 HAP 安装到 HarmonyOS 设备，切换双拼 14 键和 18 键，确认键面与日志中的会话网格加载。
