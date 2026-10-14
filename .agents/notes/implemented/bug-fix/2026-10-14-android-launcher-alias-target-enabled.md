# Agent Note: Android 图标只切别名，宿主 Activity 始终启用

Status: implemented

## Problem

继续验收 #7809 时，官方 0.4.0 拼音版在 API 28 上偶发从更多进入反馈后退回编辑器，键盘已收起。独立编辑器 UID、真实触摸注入的重复验收抓到系统 `force-stop`：切到林绿约十秒后，系统主动结束已经通过林绿别名恢复到前台的宿主，没有 Java 崩溃堆栈。

`AppIcons.select` 禁用经典图标时禁用了真实的 `HomeActivity`。Android 9 的 `ActivityManagerService.cleanupDisabledPackageComponentsLocked` 会处理延后的包组件变更广播，`ActivityStack.finishDisabledPackageActivitiesLocked` 按 `realActivity` 匹配被禁用的类；即使启动 Intent 指向启用的别名，实际 Activity 仍是被禁用的 `HomeActivity`，系统照样关闭它。

这解释了模拟器上的一次确定失败，尚未证明 vivo Android 11 原报告的全部原因。两份原诊断包没有页面启动或系统拦截记录；仅凭末尾的 `ime_finish` 不能归因到权限或图标。

## Decision

经典图标也使用独立的 `MainActivityClassic` 别名。五种图标只切换各自别名，真实 `HomeActivity` 始终启用。宿主和输入法服务启动时统一协调组件状态：旧版已启用的彩色别名优先保留，恢复旧版可能禁用的真实 Activity，再保证只有所选图标别名可用。仅在状态真的需要变化时写包管理器，避免每次启动重复触发延迟广播。

## Historical audit

[启用图标深链](../../implemented/bug-fix/2026-10-13-android-host-deep-link-enabled-icon.md)部分重叠：保留按当前 launcher 组件启动的决定；本篇补齐其未覆盖的系统异步关闭问题，并采用它当时因迁移成本未采用的独立经典别名。其余 `AppIcons`、图标禁用、launcher alias 检索命中仅涉及界面入口，无相同组件生命周期决定。

## Alternatives considered

- 只经启用别名启动：改动最小，确实避开直接启动禁用组件的异常；现有发布包已这样做，但系统仍按实际 Activity 清理，不能挡住本次异步关闭。
- 选彩色图标时重新启用带 launcher 的 `HomeActivity`：能保住宿主，但会留下经典与彩色两个图标；独立经典别名把图标可见性与宿主可用性分开。
- 切换后等待十秒再打开页面：可让延迟广播先执行，便于对照定位，但用户操作会卡住，且旧组件一直禁用仍可能被后续广播清理，不作为修复。
- 用 `PendingIntent` 绕后台启动：适用于另一些系统拦截，但这里已经成功恢复宿主，随后被禁用组件清理；换发起方式不能修正组件状态。

## Verification

官方 0.4.0 拼音包在 API 28 的独立编辑器 UID 回归中，林绿设置页先出现，随后在十五秒延迟窗口内消失；新包的同一回归通过。API 28/35 都覆盖安装在旧版林绿状态上，由输入法服务首次启动恢复真实组件，检查保留林绿、唯一 launcher 与真实 Activity 启用；并逐项从真实键盘触摸进入设置、词库、反馈与关于。

设备回归 `tests/device/host-launch.sh` 将宿主 UID 的准备与外部 UID 的页面验收分开，避免 instrumentation 改变启动权限。每种图标检查单一 launcher，并让设置页持续可见十五秒；退出时恢复原图标、引导标志、默认输入法和系统键盘。准备恢复用 `Files.readAllBytes`，不依赖 Android 不提供的 Java 11 `Files.readString`。API 28/35 拼音版的五种图标、每种四个键盘入口和退出恢复均通过；每种设置页都跨过十五秒窗口。Gradle 拼音 release 编译、APK 签名与锁定资源哈希、仅 arm64 ABI 核验、230 个 JVM 冒烟与 95 个设备套件源文件编译、本地 quick 与笔记门禁均通过。

分支更新到 `develop` 后使用对应 core 重建的 arm64 JNI，再次通过 API 28/35 全部五图标入口与退出恢复，以及 quick；源文件清单保留上游新增套件。

## Consequences

旧版组件启用状态会跨覆盖安装保留，单改 manifest 不会恢复被禁用的真实 Activity；服务与宿主两条启动路径都必须做协调。原 vivo 权限及系统日志尚不可得，PR 使用 `Refs #7809`，不宣称原设备已完整修复。
