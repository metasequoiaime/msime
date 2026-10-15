# Agent Note: 小牛翻译只把字符串结果记为回答

Status: implemented

## Problem

#8026：共享 `niutrans_translation::failed` 只排除非对象和服务错误字段，`{}`、仅有 `message` 的对象以及非字符串 `tgtText` 都被判为回答，解析器却不产生译文。Android 的 `UserTranslationClient` 因此记下永久空回答，macOS 的 `CustomTranslationBatch` 将词写进 `answeredTexts`，控制器随之负缓存八分钟。异常回复本来应允许重试。

## Decision

沿用解析器的结果字段契约：回复必须有字符串 `tgtText` 才算回答，`errorCode` 或 `errorMsg` 仍优先表示失败。空字符串、空白字符串以及被展示长度限制过滤的字符串仍是回答，由现有空回答策略控制请求次数。只修改共享失败判定，不改变宿主退避时长或缓存寿命。

## Alternatives considered

- **解析不到可显示译文就当失败**：可以复用解析器并覆盖所有无译文场景；但会把合法空回答也变为失败，破坏负缓存对用户额度的保护。
- **在 Android 和 macOS 各补字段判断**：可以直接在缓存入口拦下异常对象；但会复制共享层已有的职责，其他调用者仍会得到错误的失败判定。

## Verification

真实 C ABI 回归在旧实现下因 `{}` 被当作回答而失败，合法回答、服务错误优先级和长度边界控制通过。macOS 原生批次回归也在旧实现下失败：缺少结果字段的词被加入 `answeredTexts`。测试只替换传输，不替换桥接、解析和批次逻辑，全部数据为合成 JSON。

修复后 `cargo test --locked -p msime-host-api --lib niutrans` 的 7 项测试通过；macOS `niutrans-translation` 原生批次测试通过，逐项覆盖异常结果结构并确认合法空译文继续出现在 `answeredTexts`。

`cargo test --locked -p msime-host-api --lib translation_` 的 23 项相邻回归通过；`cargo clippy --locked -p msime-host-api --all-targets -- -D warnings` 和 `cargo fmt --all -- --check` 通过。

## Consequences

异常对象恢复各宿主既有的失败重试路径；合法空回答继续负缓存，服务错误继续优先。没有网络调用、凭据处理、文件读写或设备资源变更。

## Existing notes audit

检索活跃和被否笔记中的 `niutrans_translation::failed`、`tgtText`、小牛回复与负缓存。

- [Android 用户翻译服务](../../implemented/feature/2026-10-12-android-user-translation-services.md)部分重叠，负责路由和缓存寿命；本篇负责共享回复结构边界，与其互链。
- [自定义服务识别 DeepL](../../implemented/feature/2026-10-11-custom-translation-deepl-api.md)部分重叠，负责另一种服务的请求格式和错误判定，不合并。
- 来源过滤、账号缓存作用域和离线释义笔记不管理小牛回复结构，保留。

自定义翻译服务的成功码结果校验见[自定义翻译回复结构边界](2026-10-15-custom-translation-reply-shape.md)，其协议字段与小牛服务不同。
