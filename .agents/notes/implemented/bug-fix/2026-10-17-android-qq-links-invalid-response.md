# Agent Note: Android 绑定列表拒绝 malformed 响应

Status: implemented

## Problem

`GET /v1/users/me/qq-links` 的响应契约是 `{"links":[...]}`。Rust 客户端和 HarmonyOS 桥在外层缺少 `links` 数组或类型错误时返回 `account_unavailable`，Android 的 `TypingUploadApi.links()` 却把 `optJSONArray` 得到的 `null` 当成空列表。服务端临时返回错误形状时，Android 页面会把已有绑定显示成“没有绑定”，用户无法区分服务异常与真实空列表。

## Decision

Android 先要求响应包含数组类型的 `links` 字段；缺少该字段或类型错误时抛出 `CloudApi.Failure(200, "invalid_response", ...)`。数组内部仍沿用跨平台约定：跳过无法验证的单行，只保留最多 10 条合法编号和时间。这样只收窄 malformed 外层响应的处理，不改变合法空列表或部分坏行的兼容行为。

## Alternatives considered

- **继续把缺少字段当成空列表**：最宽容，但会静默丢失绑定并误导用户，和 Rust、HarmonyOS 的错误语义不一致，因此不采用。
- **只要有一行坏数据就拒绝整个列表**：能更早发现服务端问题，但会让单行损坏遮蔽其余合法绑定，偏离现有 Rust 解析器的逐行过滤契约，因此不采用。

## Consequences

Android 的账号页会在 malformed 外层响应时进入统一的服务不可用提示并支持重试；合法的 `{"links":[]}` 仍显示空列表，合法行和坏行混合的响应仍按既有上限处理。新增设备回归覆盖真实 Android `org.json` 解析路径，避免只在主机桩环境中测试到 `Stub!`。

## Verification

- 先在修复前运行设备回归，确认 malformed 响应被错误当成空列表而失败。
- 修复后在 API 28 模拟器 `emulator-5566` 运行 `BackendAccountRefreshDeviceSmoke`，通过。
- `ANDROID_SDK_ROOT=/Users/xiaomo/Library/Android/sdk bash platforms/android/check-host.sh`，248 个 JVM 冒烟、设备套件源编译和 Android 契约检查通过。
- `git diff --check` 通过。
