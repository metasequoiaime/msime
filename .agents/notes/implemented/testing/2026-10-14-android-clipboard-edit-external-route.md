# Agent Note: 从外部编辑器验收 Android 剪贴板编辑

Status: implemented

## Problem

#7806 的录屏显示长按历史条目后点「编辑」，键盘收起但没有编辑页出现。诊断导出不含页面启动结果；[图标组件修复](../../implemented/bug-fix/2026-10-14-android-launcher-alias-target-enabled.md)的设备回归只覆盖设置、词库、反馈和关于，不能证明带条目键的剪贴板编辑链路可用。

## Decision

准备阶段只在宿主 UID 中暂存合成历史和偏好，然后结束 instrumentation。从独立编辑器 UID 长按真实键盘卡片、点编辑，检查长于深链参数上限的置顶记录原文完整显示、页面跨过组件延迟处理窗口仍存在、保存后回到原编辑器，再打开面板能插入改后的文字。短记录取消后保持不变。最后在宿主 UID 核对时间戳、置顶状态及历史条数，并恢复历史、偏好、图标、引导状态、系统剪贴板和默认输入法。

历史存储与编辑设计仍归[原有功能笔记](../../implemented/feature/2026-10-09-android-clipboard-edit.md)；视图销毁后的保存状态归[生命周期修复](../../implemented/bug-fix/2026-10-14-android-clipboard-save-result.md)。本篇覆盖外部进入与返回，跨视图保存的队列阻塞回归见该生命周期笔记。

## Alternatives considered

- 在宿主 UID 中直接调用深链打开编辑页：容易准备存储并核对结果；但附着在宿主进程上的 instrumentation 会改变真实的后台启动和进程生命周期条件，绕过本 issue 要检查的边界。
- 只复用四个普通页面的启动回归：已覆盖全部图标的组件状态；但没有剪贴板条目键、异步加载、保存或返回原应用的行为，证据范围不足。

## Verification

API 28/35 专用 arm64 AVD 上，已安装含图标组件修复的拼音版在经典与林绿图标下均完成打开、保存、返回、重新插入及取消；原文超过 256 个 UTF-16 单元，完整预填；页面保持十五秒；结果与合成历史的时间戳、固定标志和两条记录一致。`clipboard-edit.sh` 全流程通过，包括恢复阶段。`build-editor.sh` 与本地 quick 中的 Android 宿主源码检查通过。

测试源码避免 `Files.readString` 和 `String.repeat`，以 `readAllBytes` 和显式追加兼容 API 28。脚本在改动设备前检查当前活跃 instrumentation；遇到其他设备套件明确退出，等待该套件结束后再运行。

## Consequences

模拟器没有 vivo 的后台启动策略，回归通过不能证明原机型已修复。PR 使用 `Refs #7806`，不自动关闭原报告。测试备份只留在专用 AVD 的应用缓存，失败时也执行恢复；输出只含固定阶段名与检查结果，不输出剪贴板文字或配置快照。

回归增加两次十五秒稳定性等待，并在准备和核验阶段启动宿主 instrumentation；设备矩阵必须独占 AVD 的无障碍自动化连接。未验证完整版与 vivo 真机，不把这两项计作通过。
