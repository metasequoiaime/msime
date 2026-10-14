# Agent Note: Android 宿主深链使用当前启用的图标入口

Status: implemented

## Problem

切换到林绿等主屏幕图标时，`AppIcons` 启用对应的 `activity-alias` 并禁用经典入口 `HomeActivity`。`HostDeepLink` 仍直接启动 `HomeActivity`，所以从键盘点设置、词库等页面会抛出 `ActivityNotFoundException`，但主屏幕图标仍能打开应用。手机的组件状态和直接启动命令都复现了这一差异。

## Decision

`HostDeepLink` 通过本应用的 `PackageManager.getLaunchIntentForPackage` 获取当前可用的启动组件，只采用组件名，保留原有深链参数和 `NEW_TASK | CLEAR_TOP | SINGLE_TOP` 标记。查询不到组件时保留原来的经典入口作为后备。不重新启用被用户关闭的图标组件。

## 历史决策审计

检索活跃笔记的 `HostDeepLink`、`AppIconStyle`、`AppIcons` 和图标切换关键词，没有已有笔记记录图标入口与深链之间的约束。[Android 对话 tab](../feature/2026-10-11-android-chat-tab.md)、[剪贴板编辑](../feature/2026-10-09-android-clipboard-edit.md) 等只使用深链导航参数，与本决定无关，继续保留。

## Alternatives considered

- 让 `HomeActivity` 永远启用，直达设置最稳定；当前经典图标本身就是这个 Activity 的 launcher 入口，重新启用会在主屏幕增加重复图标，必须额外改清单和迁移组件状态。
- 固定启动林绿别名，当前手机即可恢复；用户选择其他图标时林绿别名同样被禁用，不能作为通用入口。

## Consequences

从键盘打开设置或定向页面时采用当前启用的图标组件，不增加图标，也不改变用户的图标选择和导航参数。依赖包管理器返回本应用当前可用的 launcher 组件。如果所有入口都被外部禁用，经典入口后备也不能启动，继续保留现有错误提示。

## Verification

修复前真机的林绿别名启用、HomeActivity 禁用，直接启动 HomeActivity 返回组件不存在，键盘设置提示 ActivityNotFoundException。修复后 `scripts/verify-local.sh --quick`（含 Android 宿主 JVM 冒烟）通过；签名 APK 覆盖安装到 Android 12 真机。在保持林绿图标的情况下，从键盘功能面板点击「设置」实际打开宿主设置 tab，不再出现异常。其他图标和所有定向页面没有逐项真机验证。

后续[图标只切别名、宿主始终启用](2026-10-14-android-launcher-alias-target-enabled.md)部分取代组件状态前提：经当前 launcher 深链启动的决定保留，经典入口改为独立别名，并协调旧版禁用的真实宿主，补上延迟广播按真实 Activity 关闭页面的问题。
