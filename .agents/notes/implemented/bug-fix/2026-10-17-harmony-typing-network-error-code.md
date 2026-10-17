# Agent Note: 鸿蒙打字统计传输异常统一返回网络错误码

Status: implemented

## Problem

HarmonyOS 的打字统计上传和删除调用 `AccountCloudBridge.typingStatisticsRequest`。账号桥接的正常 HTTP 响应会转换为稳定的 `account_*` 错误码，但传输层抛出异常时原实现把 `Error.message` 直接塞进响应的 `error` 字段。页面和共享账号契约只识别稳定错误码，因此超时、连接断开等异常会显示成平台或库的内部文本，并且无法按 `account_network` 处理。

## Decision

`typingStatisticsRequest` 捕获传输层异常后一律返回 `account_network`，不向页面暴露异常对象的文本。HTTP 响应仍沿用现有的 `mapStatus`，账号未登录仍返回 `account_unauthorized`。上传和删除共用这一映射，保持两个入口一致。

## Alternatives considered

- **继续返回异常消息**：能保留底层诊断文字，但它不是稳定的账号协议，可能包含平台实现细节，页面无法可靠翻译或分类；不采用。
- **在每个调用方分别判断异常类型**：可以区分超时和连接错误，但会重复协议映射，并让上传和删除逐渐产生不同错误码；不采用。

## Consequences

传输失败在鸿蒙设置页与其他宿主一致地显示网络失败文案，内部异常文本不再穿过账号桥。调试时需要查看宿主日志或传输层日志，而不是依赖页面响应携带原始异常；HTTP 状态错误和成功响应的行为不变。

## Verification

`platforms/harmony/tests/account-qq-typing.test.ts` 用会抛出合成传输异常的假传输层覆盖上传和删除；修复前测试失败，修复后通过。另跑 HarmonyOS 逻辑测试脚本和 TypeScript 编译。
