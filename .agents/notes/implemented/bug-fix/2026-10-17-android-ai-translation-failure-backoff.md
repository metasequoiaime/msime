# Agent Note: Android AI 候选翻译区分失败回复与空译文

Status: implemented

## Problem

Android 候选翻译复用 AI 配置后，`UserTranslationClient.ai` 只调用解析器取译文。AI 服务返回错误信封、格式错误或不合规内容时，解析器返回空值，客户端把它和合法的空译文一起交给 `UserTranslationMemory.answered`，该词随后永久负缓存，不再遵守已有的 30 秒失败退避。服务暂时故障因此会让同一候选在故障恢复后也不再请求。

## Decision

共享 provider 增加与已有自定义翻译、小牛翻译相同语义的 `chat_translation_response_failed` 判定：空字符串是合法回答，可以负缓存；错误信封、缺字段、格式错误、控制字符和超长结果是失败，必须重试。Host API 暴露对应的只读失败判定，Android 在解析 AI 翻译结果前先调用它；失败按单词写入 `UserTranslationMemory.failed`，有效回复继续沿用原来的空值或译文缓存。

## Alternatives considered

- **把所有解析为空的 AI 回复都记成失败。** 能避免永久负缓存，但会让模型合法地返回“没有译文”的词在每次重绘时重复消耗额度，因此不采用。
- **在 Java 中复制 Chat Completions JSON 判定。** 可以不增加 ABI，但会让 Android 自己解释服务错误、代码块和字段边界，和共享 Rust 解析器再次分叉，因此不采用。
- **让网络传输层统一重试。** 传输层不知道单个词的负缓存键，也无法区分整服务不可达和单词级错误；会破坏现有按词退避语义，因此不采用。

## Consequences

AI 翻译的合法空译文仍只请求一次；错误回复按词退避 30 秒，之后可重新请求；连接、超时和非 2xx 仍由现有传输层触发服务级退避。失败判定位于共享 Rust 层，Android 只负责调用和更新既有内存状态。Host API 与 JNI 导出面增加一个只读布尔接口，其他平台行为不变。
