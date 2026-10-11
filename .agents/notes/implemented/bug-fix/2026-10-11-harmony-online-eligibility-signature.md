# Agent Note: HarmonyOS 在线候选资格状态签名

Status: implemented

## Problem

HarmonyOS 在线候选请求按签名去重，但签名没有纳入 Engine 查询的 `cloud_eligible` 和 `ai_eligible`。同一组字中一个 provider 的资格变化、另一个仍可用时，旧签名会跳过新请求，也不会推进用于拦截迟到结果的 epoch。

## Root cause

`OnlineCandidatePolicy.signature` 使用的字段少于 `KeyboardSession.fetchOnline` 判断云端与 AI 请求时使用的字段。`scheduleOnlineProviders` 在签名不变时直接返回。

## Decision

把两个资格布尔值纳入签名，并在键盘逻辑测试中分别验证两个资格变化。保留现有的请求去重、取消和 epoch 路径。

## Alternatives considered

- 仅在 provider 分支中单独检查资格变化：会绕开统一的取消和迟到结果保护。
- 每次渲染都重新请求：会重复发送相同的网络查询。

## Consequences

资格变化会触发新请求并使旧请求失效；其它未变化的查询仍由签名去重。

## Verification

- `bash platforms/harmony/tests/run.sh`：新增断言先失败，修复后通过。
- `git diff --check`
