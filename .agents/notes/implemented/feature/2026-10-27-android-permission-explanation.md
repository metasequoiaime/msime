# Agent Note: Android 关于页权限说明

Status: implemented

## Problem

用户在 Android 设置中看到应用申请的权限时，不清楚权限对应的功能以及关闭后的影响。关于页已有版本、设备信息和链接，但没有统一的权限说明入口。

## Decision

在 Android「关于」页增加独立的「权限」分区，以说明文字列出录音、相机、联系人、网络、通知和震动的用途及关闭后的影响。说明只描述现有 manifest 和权限申请路径，不新增权限、不改变申请时机，也不把离线输入误说成需要网络。

## Existing note audit

- [Android 联系人导入](../feature/2026-10-17-android-contacts-import.md)：保留用户点击导入时才申请联系人权限的行为。
- [Android 语音输入](../bug-fix/2026-10-08-android-platform-voice-in-keyboard.md)：保留键盘和前台识别窗口的录音权限流程。

## Alternatives considered

- 在首次启动时弹窗说明：不用，因为会打断输入法首次使用；关于页可随时查看且不改变现有启动流程。
- 为每个权限增加独立设置开关：不用，系统权限本身已经是唯一生效来源，应用内重复开关会造成状态不一致。

## Consequences

Android 用户可以在关于页了解权限用途和撤回后的功能影响；权限申请、manifest、网络访问和离线功能行为保持不变。

## Verification

- `bash -n` 不适用于 Java；已完成 `git diff --check`。
- Android host 编译和关于页设备验收待磁盘空间恢复到仓库规定的 50 GiB 启动线后补跑。
