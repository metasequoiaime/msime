# Agent Note: Android 手机上隐藏大屏键盘设置

Status: implemented

## Problem

手机无法使用横屏分离式键盘，物理键盘候选栏跟随光标也只面向大屏设备。把这两项显示在手机的键盘设置里会让用户看到当前设备无法使用的选项。

## Decision

设置页沿用键盘宿主已有的 `smallestScreenWidthDp >= 600` 大屏边界，只在大屏设备显示这两项。手机上不改动已保存的偏好，切换到大屏设备后仍可恢复显示并继续使用。

## Alternatives considered

- 始终显示两项：实现最简单，但手机会展示当前设备无法使用的设置。
- 按当前横竖屏隐藏：手机横屏仍然不能使用分离式键盘，且设置项会随旋转跳动。
- 仅隐藏分离式键盘：物理键盘候选栏同样是面向大屏的设置，手机仍会留下不适用的选项。

## Verification

`KeyboardFormFactorPolicySmoke` 覆盖 599/600 dp 边界；`platforms/android/check-host.sh` 负责编译 Android Java 宿主并运行该冒烟测试。

## Consequences

手机设置页不显示横屏分离式键盘和物理键盘候选栏跟随光标；大屏设备继续显示并使用原有偏好。已保存的手机偏好不被清除。
