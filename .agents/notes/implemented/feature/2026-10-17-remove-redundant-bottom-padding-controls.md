# Agent Note: 移除 Android 底部留白设置控件

Status: implemented

## Problem

Android 键盘底栏已经根据系统导航栏自动让出底部空间，设置页仍提供“底部留白”开关和高度滑块，用户需要理解两个相互重叠的避让机制。此前新增的高度设置决定见[独立调整底部留白高度](../../implemented/feature/2026-10-16-android-bottom-padding-height.md)。

## Decision

从原生 Android 键盘设置页移除这两个控件，并从设置搜索关键词中移除。保留 `AndroidLocalSettings` 的旧字段和键盘运行时读取，避免升级时丢失旧配置或改变已有设备的布局；新用户不再有入口打开这套重复控制。

## Alternatives considered

- 继续显示两个控件：保留完整控制，但与键盘底栏的自动避让重复，增加设置复杂度。
- 只删除高度滑块：仍留下一个与自动底栏行为重叠的开关，无法解决重复入口。
- 删除字段和运行时逻辑：能彻底收缩代码，但会让已有用户升级后改变键盘位置，超出本次设置界面清理范围。

## Verification

`KeyboardOptionsPage` 不再创建两个控件，设备冒烟 `BottomPaddingSettingsDeviceSmoke` 改为确认它们不出现在原生设置页；`platforms/android/check-host.sh` 通过 Android 宿主契约、JVM 冒烟和设备套件源码检查。

## Consequences

Android 设置搜索不再返回底部留白控件，旧的本地偏好仍可被键盘兼容读取。底部留白策略和相关设备回归继续保留，直到另有决定移除运行时行为。

「不给入口」这一点已被[恢复单一的「键盘底部高度」滑块](2026-11-01-android-keyboard-bottom-height-slider.md)部分取代（#10447）：底栏不出现的设备没有其他抬高底行的手段，设置页改为只提供一个合并计算底栏的滑块；保留旧字段与运行时读取的决定仍然成立。
