# Agent Note: Android 云端数据汇总拒绝缺失分类

Status: implemented

## Problem

`GET /v1/users/me/data` 的共享账号契约要求 `bytes` 和 `sections`。Rust 客户端用反序列化拒绝缺失 `sections`，Android `DeviceDataApi.dataSummary()` 却把缺少或类型错误的 `sections` 当成空列表。服务端响应损坏或协议不兼容时，Android 账号页会把云端数据错误显示成没有分类，用户还可能据此执行错误的删除判断。

## Decision

Android 在展开分类前要求 `sections` 是 JSON 数组；缺失或类型错误时返回 `invalid_response`。数组长度仍受 `MAX_DATA_SECTIONS` 限制，分类行的既有过滤和计数校验保持不变；`identities` 等明确可选的字段不改变其现有回退语义。

## Alternatives considered

- **继续把缺失分类当成空列表**：可以兼容旧响应，但会把 malformed 响应伪装成真实的零分类，和共享 Rust 契约不一致，因此不采用。
- **拒绝任意坏分类行**：能更严格地发现服务端问题，但会改变现有逐行过滤行为，且不是本次缺失外层字段的根因，因此不采用。

## Consequences

Android 账号页在外层响应 malformed 时进入统一的服务不可用流程，合法的 `sections: []` 仍表示真实空分类；现有合法分类和坏行过滤行为不变。新增设备回归通过 Android 真正的 `org.json` 实现验证，并纳入设置设备套件。

从 #8201 迁移计划 5c 起汇总经 Rust 的 `data_summary`，缺少 `sections` 由 Rust 的反序列化拒绝（`AccountDataSummary.sections` 不带默认值），Java 不再解析服务端响应，`DeviceDataSummaryDeviceSmoke` 随之删掉。见 [Android 设置页面的账号与云端操作改调 Rust](../architecture/2026-10-18-android-settings-pages-through-rust.md)。

## Verification

- 修复前 API 28 `DeviceDataSummaryDeviceSmoke` 红：`{"bytes":0}` 被当成空分类。
- 修复后 API 28 `emulator-5566` 同一回归通过。
- `ANDROID_SDK_ROOT=/Users/xiaomo/Library/Android/sdk bash platforms/android/tests/device/build-editor.sh`
- `ANDROID_SDK_ROOT=/Users/xiaomo/Library/Android/sdk bash platforms/android/check-host.sh`
- `git diff --check`
