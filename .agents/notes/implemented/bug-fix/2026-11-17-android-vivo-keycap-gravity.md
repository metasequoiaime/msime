# Agent Note: Android vivo 键帽文字显式居中

Status: implemented

## Problem

最初根据 Android 0.8.0 的 vivo 截图推测主字符被主题默认布局推到键帽边缘；该推测没有在设备上复现。用户确认 0.8.1 仍然三排主字母全空、角标正常后，这一根因判断撤回，实际复现与修复见[角标不能挤掉主字母区域](2026-11-17-android-key-hint-main-label-width.md)。普通键帽沿用系统 `Button` 的 gravity；现有 `normalizeKeyCap` 只清除 padding、字体留白和最小尺寸，未覆盖厂商主题的 gravity 差异。

## Decision

`KeyboardTypographyPolicy.normalizeKeyCap` 在处理所有普通键帽、提示键和图标键时显式设置 `Gravity.CENTER`，再应用键帽自己的 padding。主标签因此始终在可用键帽区域水平、垂直居中，角标和底部提示仍由各自的绘制逻辑占用独立区域。

设备侧的双拼键面几何检查不再自行设置 gravity，而是断言产品键帽构造后的水平、垂直 gravity，避免测试把 vivo 主题布局问题掩盖掉。

## Alternatives considered

- 只给符号层和数字层的调用点设置 gravity：改动范围较小，但同一厂商主题仍可能在字母层、底行或后续新增键帽上复现，且会让键帽创建契约分散。
- 在自定义皮肤中调整文字颜色或重新计算对比度：当时没有颜色证据，改颜色还会改变用户皮肤的既有选择，因此未采用；后续切换内置皮肤仍复现。
- 继续依赖系统主题的默认 gravity：普通 Android 主题上可能看起来正常，但无法约束 vivo 等厂商主题，不满足跨设备键帽布局契约。

## Consequences

所有经过 `normalizeKeyCap` 的键帽都不再受 OEM `Button` 默认 gravity 影响，主字符与提示区域的相对位置保持现有设计。显式 gravity 会覆盖调用方若有意设置的按钮默认对齐；当前键帽工厂和自绘图标均以居中为契约，未发现需要保留其他对齐方式的键帽。设备真机截图仍需在报告者的 vivo 环境复验。

## Verification

- `ANDROID_SDK_ROOT=$HOME/Library/Android/sdk bash platforms/android/check-host.sh` 通过，包含 362 项 JVM smoke、300 个设备套件源码、Java/API、清单和资源检查；`ShuangpinKeyFaceChecks` 由设备套件编译接纳新的 gravity 断言。
- 笔记树、格式和归档封印校验通过（2601 篇笔记、7 个归档封印）。
- 当前 `adb devices` 没有可见设备，未构建完整 APK，也未在 API 28/API 35 模拟器或报告者 vivo 真机上运行设备 smoke 和改前/改后截图；这些缺口需在 PR CI 和 vivo 复验中补齐。
