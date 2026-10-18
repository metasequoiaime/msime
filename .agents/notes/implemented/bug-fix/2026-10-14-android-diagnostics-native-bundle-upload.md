# Agent Note: Android 上传器读取共享层诊断包格式

Status: implemented

## Problem

排查 #7837 并核对相关 Android 附件时，发现 `DeveloperPage.upload` 先调用共享层生成 ZIP，再由 `DiagnosticsApi.readBundle` 读成上传请求。共享层固定写出 `input_events.ndjson`、`performance_logs.ndjson`、`crash_logs.ndjson`，读取器却只识别旧 `.jsonl` 和 `.crash`，上传成功时三类日志仍为空，只有配置快照被读到。这是诊断传递缺陷，不能解释原报告中的输入连接失败或页面关闭。

## Decision

上传器识别共享层三个固定条目，崩溃 NDJSON 按 `at`、`message`、`stack` 字符串字段重建，拒绝字段类型、数量或长度不合规的记录；保留旧别名、选择开关与现有条目、字节、记录数上限。`DiagnosticsBundleDeviceSmoke` 从安装后的产品类加载器调用原生导出、读取器和请求编码，使用合成日志，不联系云端；`smoke.sh --diagnostics` 可与 API 28 的 `--core` 组合执行。

## Historical audit

[诊断归档条目上限](2026-10-09-android-diagnostics-archive-entry-bound.md)部分重叠，继续保持 128 个条目、每类记录有界；这里修正读写格式契约。[反馈页连续输入回归](../testing/2026-10-14-android-feedback-input-stability.md)部分重叠，继续保留 #7837 现场根因未确认的边界。

## Alternatives considered

- 共享层改回旧文件名：可以省去 Android 适配，但会改变跨平台统一导出格式和已有附件契约，崩溃数据也仍有 JSON 与文本的差别，因此修读取器。
- 直接把 ZIP 或原始输入日志上传：传递步骤少，但会改变服务端接口或绕开共享层脱敏流程，不采用。

## Verification

API 28 专用 arm64 AVD：原宿主生成的输入记录通过共享层过滤，但设备回归失败于上传请求列表为空；修复后的原生 full 宿主通过生成、读取、请求编码全链路，验证三类记录值与崩溃原时间戳、隐私过滤、旧别名、未选类别、异常行和 50 条崩溃记录上限。`smoke.sh emulator-5566 --core --feedback --diagnostics` 同时通过上屏退格与反馈页 50 次上屏、五轮删除和收起重开；新增上限与时间戳检查后单独设备回归再次通过。

合并最新 `develop` 后，保留反馈、诊断与剪贴板保存重建三组设备入口；`build-apk.sh` 重新编译 arm64 JNI 并构建原生 full 宿主，签名、资源校验、16 KB 对齐及安装通过。API 28 的 `--core --feedback --diagnostics` 再次通过三类输入与诊断回归，以及新合入的剪贴板视图重建、Activity 重建和保存失败重试回归。本地 quick 覆盖合并结果，Linux 83 项单测、Windows 原生与两种 pipe-only 交叉构建、Android、macOS 与共享桥检查通过。

新增诊断设备回归本轮只在 API 28 执行，不把此前反馈输入的 API 35 结果扩展为诊断上传验证。测试结束清理合成临时目录，不提交真实输入或原始日志。

## Consequences

Android 开发者日志上传保留共享层已过滤的三类记录，不再静默上传空列表。新增崩溃条目沿用现有读取上限和脱敏来源约束，仍需要解析有界的 NDJSON 文本；不把上传漏读修复宣称为 #7837 原故障修复。原现场失败类别仍未知，已有附件没有页面启动结果或输入连接失败原因，后续排查不能把空崩溃列表等同于没有故障。

[输入失败类别](2026-10-14-android-input-failure-categories.md)补上宿主丢弃失败来源的诊断缺口，只输出固定 logcat 类别，不改变原停止回退或 P19 上传格式；原报告根因仍未确认。

从 #8201 迁移计划 5c 起上传不再经共享层 ZIP 转一道：Rust 的 `diagnostics_upload` 收到与导出同一份 bundle，自己读三类记录并按服务端规则重建，Java 的读取器和请求编码删掉，`DiagnosticsBundleDeviceSmoke` 只留导出和隐私过滤那一半。见 [Android 设置页面的账号与云端操作改调 Rust](../architecture/2026-10-18-android-settings-pages-through-rust.md)。
