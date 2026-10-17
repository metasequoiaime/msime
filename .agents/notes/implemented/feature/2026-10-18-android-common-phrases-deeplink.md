# Agent Note: Android 键盘常用语管理深链

Status: implemented

## Problem

从键盘常用语面板进入「管理常用语」时，输入法窗口会收起，但部分 Android 11 定制系统没有把设置宿主带到前台。问题发生在输入法进程通过包管理器取得 launcher alias 并启动它的路径；调用不一定抛异常，用户只看到键盘消失。

## Decision

`HostDeepLink` 直接指定始终启用的 `HomeActivity`，保留 `NEW_TASK | CLEAR_TOP | SINGLE_TOP` 深链标记。launcher alias 只负责桌面图标入口，不再参与输入法进程的设置页跳转。

## Alternatives considered

- 继续使用 `getLaunchIntentForPackage`：在受影响设备上可能得到已经被图标切换流程禁用的 alias，无法保证 `startActivity` 把宿主带到前台。
- 延迟 `requestHideSelf`：只能改变键盘收起时序，不能修复目标 Activity 解析不稳定，且会让宿主和输入法窗口短暂重叠。

## Verification

静态 Android host 契约检查、Java 编译和 API 28/35 模拟器设备路径用于验证；设备验证以实际构建产物可用性为准。

## Consequences

主屏图标切换仍由 alias 控制，键盘打开设置页和其他宿主深链统一落到同一个真实 Activity，页面参数与任务栈行为不变。
