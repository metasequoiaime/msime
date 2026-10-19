# Agent Note: 云端转写拒绝明确失败的文字

Status: implemented

## Problem

macOS 与 Windows 共用的 `parse_transcription` 按 `text`、`transcription`、`result.text`、聊天候选依次返回非空文字，却没有检查 HTTP 200 回复里的顶层 `error` 或首候选的 `finish_reason`。明确报错、截断或过滤的正文因此仍能成为识别成功结果，进入润色和文字投递。

## Decision

在共享 C++ 解析入口提取任何文字之前，拒绝非 `null` 顶层错误，以及首候选 `length`、`content_filter`。抛出固定 `VoiceError`，沿用 `recognize_cloud_asr` 的 `CloudAsrError` 和宿主现有识别失败处理，不透传服务端错误正文。缺省、`null` 与正常结束保持兼容；转写字段顺序、请求、重试和空文字契约保持原样。本批只覆盖 macOS、Windows 共用入口，其他独立宿主分批处理。

## 历史笔记审计

- [润色完整性](../../implemented/bug-fix/2026-10-19-voice-polish-incomplete-reply.md)：部分重叠，状态规则相同，但润色失败保留完整转写，转写失败不得投递正文。
- [百炼请求格式](../../implemented/feature/2026-10-09-bailian-asr-request-format.md)：部分重叠，保留 `chat_audio` 请求格式和录音上限，补上响应状态检查。
- [Windows 原生语音对齐](../../implemented/feature/2026-10-10-windows-native-voice-mac-parity.md)：部分重叠，沿用宿主识别失败回调，不修改空转写分类与投递规则。

## Alternatives considered

- 只在读取聊天正文之前检查 `finish_reason`：检查位置与 Chat Completions 字段更接近，但同一回复附带转写别名时会提前成功。明确失败状态应约束整个回复。
- 只接受 `finish_reason: stop`：完成状态最严格，但会拒绝省略或置空字段的既有兼容服务。因此只拒绝明确报告的不完整状态。

## Verification

`shared-voice-http-transport` 通过真实 curl 回环请求覆盖 48 种回复，包含四种转写字段、六种非 `null` 错误、截断与过滤，以及缺省、`null`、正常结束。旧实现误接受了 32 种明确失败回复，修复后全部通过；原有上传格式、重试、重定向与响应大小回归保持通过。

macOS `http-voice-request` 增加顶层错误、聊天截断和带文字别名的过滤回复，确认回调发生在主线程、正文为空且返回固定错误说明，不触发润色。旧实现错误进入润色，修复后通过。不访问外部服务、不录音、不使用真实输入或凭据。

macOS 原生宿主与全部 CMake 目标构建通过。独立代码与安全审查未发现阻塞问题，审查者复跑两项 HTTP ctest 和 `git diff --check` 通过。

本地 `bash scripts/verify-local.sh --quick` 通过，包含 Rust workspace、Android 与 WASM 目标检查、Android 宿主 Java、Linux 桌面外壳与原生 IBus/Fcitx5 构建及 90 项测试、Windows 157 项可移植测试、macOS 构建和共享 Apple 桥接层。Windows 完整构建因缺少 MinGW 跳过；HarmonyOS 真打包因本工作区未暂存依赖跳过。笔记与差异门禁通过。

## Consequences

- 明确失败的转写不会进入润色或投递，宿主沿用现有失败提示；不增加日志或服务端错误正文透传。
- 省略结束状态的服务依然不能证明结果完整，未知结束原因沿用现有兼容行为。明确失败时舍弃服务已经生成的部分文字。
- 本批仅覆盖共享 C++ 的 macOS 与 Windows 转写入口，不覆盖其他平台的独立解析路径、流式识别或凭据探测。
- 本机缺少 Windows 完整交叉工具链，共享测试不能替代 Windows 产品构建与系统输入法验收；也未验证外部 HTTPS、麦克风录音或完整语音交互。

## Sources

[OpenAI Chat Completions 响应说明](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/retrieve) 定义 `length` 为输出预算耗尽、`content_filter` 为过滤造成内容省略。[官方 Python SDK](https://github.com/openai/openai-python/blob/main/src/openai/lib/_parsing/_completions.py) 在解析正文前拒绝这两种状态。兼容缺省与 `null` 是项目既有服务兼容决定。
