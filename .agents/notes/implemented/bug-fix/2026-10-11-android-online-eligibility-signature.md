# Agent Note: Android 在线候选资格状态签名

Status: implemented

## Problem

Android 在线候选 provider 用会话、缓存键、身份、云候选开关和 AI 配置组成去重签名，却遗漏了 Engine 查询里的 `cloud_eligible` 与 `ai_eligible`。同一组字中一个 provider 的资格变化而另一个仍可用时，服务会复用旧签名：新启用的 provider 不会请求，已禁用 provider 的迟到结果也可能继续落地。

## Root cause

`OnlineCandidatePolicy.signature` 的字段集合比 `requestsCloud` / `requestsAi` 使用的输入少了两个资格布尔值。`MSIMEInputService.scheduleOnlineProviders` 因而无法把资格边界变化传递给 `onlineEpoch` 和请求取消逻辑。

## Decision

把两个资格布尔值纳入 Android 签名，并在策略冒烟中分别断言每个资格变化都会生成新签名。会话代次、缓存键、身份、云开关和 AI 配置字段保持原有编码。

## Alternatives considered

- 只在 `requestsCloud` / `requestsAi` 结果变化时手动清理签名：调用方容易漏掉其中一个 provider，且迟到回调仍可能跨边界。
- 每次渲染都强制请求：会重新发送完全相同的查询，放大网络和隐私开销。
- 只依赖 Engine 结果代次：资格状态变化未必推进候选代次，不能阻止旧请求复用。

## Consequences

资格变化会进入统一的签名、epoch 和迟到结果保护路径；仅推进 Engine 代次而未改变查询身份仍不会重复请求。

## Verification

- `platforms/android/tests/candidate/OnlineCandidatePolicySmoke.java`：旧 API 先因资格断言编译失败，修复后通过。
- `bash platforms/android/check-host.sh`：205 个 Android JVM 冒烟、79 个设备源编译通过。
- `bash scripts/verify-local.sh --quick`：通过。
- `git diff --check`
