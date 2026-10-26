# Agent Note: macOS Caps Lock 输入监控权限请求入口

Status: implemented

## Problem

macOS 输入法进程在检测不到 Quartz 的无状态 Caps Lock 位时，会在后台进程中请求 HID 输入监控权限。后台进程没有稳定可见的前台授权界面，用户可能只看到 Caps Lock 仍然不能切换中英文，也没有任何授权提示。

## Decision

设置应用读取偏好时，如果 `switch_language_caps_lock` 已开启，就由前台设置应用请求 HID 输入监控权限。底层只在权限状态为 `Unknown` 时调用 `IOHIDRequestAccess`，已授权或已拒绝的状态不会重复弹出请求。输入法侧原有 HID 启动、重试和未知状态下的保守状态机保持不变。

## Alternatives considered

- 继续只让输入法后台进程请求权限：实现改动最小，但授权提示可能没有可见的前台宿主，无法解决现场问题。
- 把未知 HID 状态直接当作按键松开：可以绕开授权等待，但会把长按误判为短按，破坏现有 Caps Lock 状态机的安全边界。
- 新增独立的“请求输入监控权限”按钮，或只在开关从关闭变为开启时请求：交互更明确，但已有开关已开启且从未成功请求权限的用户仍可能错过入口；当前由读取已开启偏好触发，覆盖面更完整。

## Consequences

用户打开设置应用并读取已开启的 Caps Lock 快捷键偏好时，会在前台看到系统授权请求；输入法仍会在权限未知时避免布防短按，因此不会因长按状态不完整而误切换。权限已经确定后不会重复弹窗。当前环境缺少完整 macOS 原生构建所需的 `target/macos/EngineResources`，也没有报告者设备，需由 CI 和维护者在 macOS 实机/真实编辑器中补充验收。
