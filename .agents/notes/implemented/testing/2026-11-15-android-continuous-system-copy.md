# Agent Note: Android 连续复制验收经过系统监听

Status: implemented

## Problem

[#11448](https://github.com/metasequoiaime/msime/issues/11448) 范围已确认为全平台。Android 已在输入法服务中注册系统剪贴板变化监听，但既有容量测试直接调用历史存储，无法证明其他应用连续复制时输入法真正收到并保存了每一条。

## Decision

在既有设备容量入口后执行独立的 `ClipboardContinuousCopyDeviceChecks`。专用模拟器的真实编辑器使用系统无障碍复制操作，面板始终关闭，连续复制三条不同的合成文本，再复制第一条并重绑输入法。通过共享存储读取落盘结果，要求三条记录完整、顺序正确、重复复制只移动对应记录。

三次复制间隔 150 ms，期间不等待历史写入，不调用产品捕获方法，不直接写历史。测试保存并恢复偏好、主模式历史、访客模式标志与系统剪贴板；先停止输入法再恢复历史文件，避免恢复动作被采样覆盖。该用例不承诺恢复输入法未运行时已经被系统覆盖的正文。

## Existing notes audit

[设备门禁](../../implemented/testing/2026-10-08-android-device-gates.md)保留真实宿主验收要求；[移动剪贴板容量](../../implemented/feature/2026-10-15-mobile-clipboard-history-limit.md)和[历史容量](../../implemented/feature/2026-10-17-clipboard-history-capacity.md)继续负责容量规则，本项补齐其无法覆盖的系统监听链路，不改变产品默认值。

## Alternatives considered

直接调用 `captureClipboard` 能稳定覆盖宿主方法，却绕过监听注册、进程生命周期和其他应用发起的系统复制，因此继续使用实际系统事件。仅验证存储容量的测试成本更低，但不能作为本 issue 的证据。

## Verification

完整发布 APK 与 288 个设备测试源码编译成功，353 项 JVM 冒烟通过。专用 API 28（`emulator-5566`）和 API 35（`emulator-5580`）均通过连续三次系统复制、重复移动、重绑输入法后保留的验收，输出 `MSIME_DEVICE_SMOKE_PASSED`。远端 `20261115-010123` 的完整 `bash scripts/verify-local.sh --quick` 成功，笔记门禁通过。测试编辑器显式使用多行输入，保留原容量测试前置条件；仅新增系统链路证明，Android 产品监听逻辑无需改变。

## Consequences

设备容量入口增加真实编辑器生命周期验收，执行时间略增。测试只能在隔离的专用设备运行，合成内容不写入用户设备。若系统在应用读取前覆盖了正文，通知本身不能还原已经丢失的内容。
