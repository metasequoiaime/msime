# Agent Note: Android 按键按下缩小开关

Status: implemented

## Problem

Android 键盘的按键按下反馈同时包含缩小、下移、按键动画、按键音、振动和弹出预览。现有“按键动画=无”只关闭动画，不能单独关闭按下时的缩小效果。Issue #11077 要求提供独立开关，且不能改变已有用户的默认手感。

## Decision

新增仅保存在本机的 `platform.android.press_feedback` 设置，默认值为开启。Android 设置页在“按键反馈”组中提供“按下时缩小”开关，并把说明放进标题旁的帮助提示。

开关只控制 `KeyboardPressFeedback` 的按下状态缩放和下移；关闭后，按键动画选择、按键音、按键振动和按键弹出预览仍按原设置工作。作用范围沿用 `KeyboardPressButton` 使用的统一按压反馈路径，包括使用该控件的键盘和触控操作按钮。设置读取失败时保留原来的开启行为，避免设置存储异常改变输入反馈。

## Alternatives considered

- 把选项并入“按键动画”：未采用，因为现有动画开关的语义是系统动画缩放，不覆盖按键音、振动和按压状态，合并会使用户无法表达只关闭缩小的需求。
- 改变默认值为关闭：未采用，会改变现有用户的按键手感，且没有单独的产品授权。
- 只在主键区生效：未采用，当前按压反馈由统一控件提供，拆分范围会造成键盘与同类触控按钮行为不一致；若后续需要更窄范围，应另行定义验收范围。

## Verification

- `ANDROID_SDK_ROOT=$HOME/Library/Android/sdk bash platforms/android/check-host.sh`
- `pnpm run verify-notes`
- Android API 28、API 35 设备安装和真实键盘操作仍待 PR 设备 job 与报告者复验，PR 保持 draft。

## Consequences

用户可以关闭按下时的缩小和下移，同时保留其他按键反馈。设置是 Android 本机设置，不会同步到其他设备，也不会改变默认行为。统一按压控件上的触控按钮也会读取该开关；这保持了现有反馈路径的一致性，但需要在设备验收时确认没有不希望变化的按钮。
