# Agent Note: Android 文本编辑面板保留行级动作

Status: implemented

## Problem

文本编辑面板重排为四行网格后，`LINE_START`、`LINE_END`、`SELECT_LINE` 仍留在动作枚举和偏好兼容层，却没有可见入口。Issue #11037 要求继续保留这三个功能，并建议用文首、文末、全选键上滑承载行级动作。

## Decision

文首、文末、全选保留原来的点击行为；明显向上滑过 24 dp 且纵向位移超过横向位移 1.5 倍时，分别执行行首、行尾、选择当前行。上滑动作只在触摸结束时执行，普通点击仍交给原有 `Button` 处理；键的 tooltip 提供上滑提示，避免改变键面文字、无障碍描述和既有侧键排序。

## Alternatives considered

1. 在方向键模式增加三个常驻按键：入口直观，但需要重新压缩四行网格并改变现有布局契约。
2. 继续只保留枚举和偏好兼容，不提供入口：不满足 Issue #11037 的用户可达性要求。
3. 用长按承载行级动作：会与既有按住反馈和编辑面板长按行为竞争，且报告者推荐上滑方案。

## Consequences

三个行级动作重新可达，既有点击、排序、偏好格式和中央方向区布局不变。上滑触发需要用户明确移动超过阈值；斜向点击、向下滑和多指触摸不会触发行级动作。需要 Android API 28/API 35 设备验证文首、文末和全选三处上滑以及普通点击回归。

⇤ ⇥ 的上滑和三个键的提示气泡已由 [⇤ ⇥ 单击行首行尾、双击文首文尾](2026-11-08-android-text-edit-edge-double-tap.md) 取代（#11111）；全选上滑选择当前行和 `TextEditSwipePolicy` 的判定保留，提示改成键面角标「选行」。

## Verification

新增 `TextEditSwipePolicySmoke` 覆盖阈值、方向和横向位移边界；`TextEditPanelModelSmoke` 覆盖三个动作映射和读屏提示。尚未在本机 Android API 28/API 35 模拟器上完成设备验收。
