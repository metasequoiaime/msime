# Agent Note: 自定义翻译回复的结构与文本字段回退

Status: implemented

## Problem

#8029：共享 `translation_response_failed` 遇到数值或字符串 `200` 就提前判为回答，绕过已有的结果结构校验。`{"code":200}`、`data:null` 或非文本结果被当作空回答，Android 将其记入当前路由的回答集合，macOS 批次将其写入 `answeredTexts` 后负缓存八分钟，异常回复因此不能按原有失败路径重试。

对象结果也有空字段遮蔽问题：`{"data":{"text":"","translation":"synthetic"}}` 的 `text` 被 `find_map` 选中后才过滤为空，后续 `translation` 或 `data` 中的译文不再检查。连接测试因此误报失败，候选回复则被当作合法空回答。

## Decision

非成功码仍直接失败；成功码继续经过已有结果结构校验。回复必须有解析器支持的字符串、文本对象或数组结果；合法空字符串、空数组、空文本对象仍是回答。显式 `error` 继续优先，请求格式、缓存寿命和退避保持。

对象内仍按 `text`、`translation`、`data` 的优先级取第一个非空字符串，空字符串在选取时跳过。顶层字段优先级、只读取数组首项、真实空回答与显式错误的含义保持；不递归搜索未知回复结构。

DeepLX 上游现为 `OwO-Network/DLX`，真实默认分支 `main` 的固定提交 [0f2b50dd69fea3012d95a8d0e7a4ae8dca9ff5b2](https://github.com/OwO-Network/DLX/blob/0f2b50dd69fea3012d95a8d0e7a4ae8dca9ff5b2/README.md) 中，成功回复带 `code:200` 和字符串或数组 `data`；没有只凭成功码判定空回答的协议依据。历史提交 `1c723a260a`、`ce9be2e0a5` 已校验无状态码回复，本次把同一门槛应用到带成功码的回复。

## Alternatives considered

- **保留成功码豁免**：最宽容地接纳自建服务的成功信号；但缺少结果字段时解析器无法得到译文，把异常回复负缓存会阻止恢复，且上游正常回复不需要此豁免。
- **解析不到可显示译文就判失败**：可以直接复用解析结果；但合法空回答和被长度限制过滤的结果也会反复请求，破坏现有负缓存对额度的保护。
- **在 Android、macOS 各补结果判断**：直接守住缓存入口；但其他共享层调用者仍会收到错误判定，而且重复已有共享职责。

- **在连接测试或某个宿主里补一次回退**：可以直接修复该入口，但其他入口仍会遗漏相同译文；共享解析器本来就持有所有支持字段的优先级，应在这一处跳过空值。

## Verification

共享 Rust 和真实 C ABI 回归在旧实现下因 `{"code":200}` 被当作回答而失败；macOS 原生批次回归也在旧实现下失败：缺少结果的词进入 `answeredTexts`。测试只替换传输，真实执行桥接、共享解析和批次逻辑，全部回复为合成数据。

修复后共享翻译模块的 35 项测试、C ABI 的 24 项相邻翻译回归及 macOS `custom-translation-batch` 原生批次测试通过。`cargo clippy --locked -p msime-client-core -p msime-host-api --all-targets -- -D warnings`、`cargo fmt --all -- --check` 与 `pnpm run verify-notes` 通过。

对象别名回退新增共享解析两项、连接测试一项及 C ABI 一项回归，旧实现四项都失败，修复后通过。用例同时验证字段优先级、数组首项、全空回答和错误拒绝边界；完整 client-core、client-providers 和 host-api 测试共 1233 项通过、3 项忽略。独立审查复验共享解析、连接测试、ABI 及相邻用例，无阻断；本批三 crate 的 Clippy（`--all-targets --locked -- -D warnings`）、格式、笔记门禁、差异检查及 quick 均通过。quick 覆盖 Rust 工作区、Android/WASM 编译、Android Java 测试、macOS 原生宿主与共享 Apple 桥接层构建；Linux、Windows 交叉编译和 Harmony 真编译因本机工具或依赖不齐跳过。

## Consequences

异常结构恢复各宿主既有失败重试路径，合法空回答继续负缓存。有有效对象别名的回复可以正常展示并通过连接测试，不再因同一对象前面的空字段而消失。曾依靠只有成功码来表示空回答的自建服务应返回合法空字符串或空数组。没有真实服务凭据，合成回复测试不能证明远端服务可用。

## Existing notes audit

本次扩充前检索 `value_as_text`、`empty_response_fields`、空字段与译文回退；归属仍是本篇回复结构决定，原地扩展字段选择边界。实现沿用 Rust [`Iterator::find_map`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.find_map) 与 serde_json [`Value::as_str`](https://docs.rs/serde_json/latest/serde_json/value/enum.Value.html#method.as_str)，只在查找时把空字符串当作未命中。

检索活跃及被否笔记中的自定义翻译、`translation_response_failed`、DeepLX、空回答和负缓存。

- [自定义服务识别 DeepL](../../implemented/feature/2026-10-11-custom-translation-deepl-api.md)部分重叠，负责请求协议；本篇记录成功码回复边界，与其互链。
- [Android 用户翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)部分重叠，负责路由和缓存寿命；本篇不修改其决定。
- [小牛翻译结果结构](../../implemented/bug-fix/2026-10-15-niutrans-reply-shape.md)部分重叠，负责另一服务的结果字段；保留并关联，不能替代本篇的自定义协议边界。
- 来源过滤、账号缓存作用域及离线释义笔记不负责回复结果结构，保留。
