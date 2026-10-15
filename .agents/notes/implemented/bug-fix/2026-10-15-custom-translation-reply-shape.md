# Agent Note: 自定义翻译成功码也必须带支持的结果结构

Status: implemented

## Problem

#8029：共享 `translation_response_failed` 遇到数值或字符串 `200` 就提前判为回答，绕过已有的结果结构校验。`{"code":200}`、`data:null` 或非文本结果被当作空回答，Android 将其记入当前路由的回答集合，macOS 批次将其写入 `answeredTexts` 后负缓存八分钟，异常回复因此不能按原有失败路径重试。

## Decision

非成功码仍直接失败；成功码继续经过已有结果结构校验。回复必须有解析器支持的字符串、文本对象或数组结果；合法空字符串、空数组、空文本对象仍是回答。显式 `error` 继续优先，不改变解析器、请求格式、缓存寿命和退避。

DeepLX 上游现为 `OwO-Network/DLX`，真实默认分支 `main` 的固定提交 [0f2b50dd69fea3012d95a8d0e7a4ae8dca9ff5b2](https://github.com/OwO-Network/DLX/blob/0f2b50dd69fea3012d95a8d0e7a4ae8dca9ff5b2/README.md) 中，成功回复带 `code:200` 和字符串或数组 `data`；没有只凭成功码判定空回答的协议依据。历史提交 `1c723a260a`、`ce9be2e0a5` 已校验无状态码回复，本次把同一门槛应用到带成功码的回复。

## Alternatives considered

- **保留成功码豁免**：最宽容地接纳自建服务的成功信号；但缺少结果字段时解析器无法得到译文，把异常回复负缓存会阻止恢复，且上游正常回复不需要此豁免。
- **解析不到可显示译文就判失败**：可以直接复用解析结果；但合法空回答和被长度限制过滤的结果也会反复请求，破坏现有负缓存对额度的保护。
- **在 Android、macOS 各补结果判断**：直接守住缓存入口；但其他共享层调用者仍会收到错误判定，而且重复已有共享职责。

## Verification

共享 Rust 和真实 C ABI 回归在旧实现下因 `{"code":200}` 被当作回答而失败；macOS 原生批次回归也在旧实现下失败：缺少结果的词进入 `answeredTexts`。测试只替换传输，真实执行桥接、共享解析和批次逻辑，全部回复为合成数据。

修复后共享翻译模块的 35 项测试、C ABI 的 24 项相邻翻译回归及 macOS `custom-translation-batch` 原生批次测试通过。`cargo clippy --locked -p msime-client-core -p msime-host-api --all-targets -- -D warnings`、`cargo fmt --all -- --check` 与 `pnpm run verify-notes` 通过。

## Consequences

异常结构恢复各宿主既有失败重试路径，合法空回答继续负缓存。曾依靠只有成功码来表示空回答的自建服务应返回合法空字符串或空数组。没有真实服务凭据，合成回复测试不能证明远端服务可用。

## Existing notes audit

检索活跃及被否笔记中的自定义翻译、`translation_response_failed`、DeepLX、空回答和负缓存。

- [自定义服务识别 DeepL](../../implemented/feature/2026-10-11-custom-translation-deepl-api.md)部分重叠，负责请求协议；本篇记录成功码回复边界，与其互链。
- [Android 用户翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)部分重叠，负责路由和缓存寿命；本篇不修改其决定。
- [小牛翻译结果结构](../../implemented/bug-fix/2026-10-15-niutrans-reply-shape.md)部分重叠，负责另一服务的结果字段；保留并关联，不能替代本篇的自定义协议边界。
- 来源过滤、账号缓存作用域及离线释义笔记不负责回复结果结构，保留。
