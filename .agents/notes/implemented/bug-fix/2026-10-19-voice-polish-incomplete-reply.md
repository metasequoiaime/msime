# Agent Note: 语音润色拒绝不完整回复

Status: implemented

## Problem

macOS 与 Windows 的原生语音流程调用共享 `polish_cloud_text`，Linux 用户会话语音服务调用自己的 `polish`，失败时都保留原始转写。这两条解析路径原先只检查 HTTP 成功和非空 `choices[0].message.content`，忽略同一条回复的 `finish_reason` 与顶层错误。因此服务返回 HTTP 200 和半段正文时，宿主会把完整转写替换成不完整结果，绕过原有回退。

## Decision

共享 C++ 解析器与 Linux `polish` 在取正文之前拒绝非 `null` 的顶层 `error`，以及首个候选的 `length`、`content_filter` 结束原因。C++ 抛出固定 `VoiceError`，由现有宿主回退保留转写；Linux 在现有 HTTP worker 返回后的 `polish` 入口检查状态，直接返回传入的原始转写，沿用独立进程的超时与取消边界。不传递服务端错误正文。未提供结束原因、结束原因为 `null`、顶层 `error: null` 和正常 `stop` 结果保持兼容。

## 历史笔记审计

- [Windows 原生语音对齐](../../implemented/feature/2026-10-10-windows-native-voice-mac-parity.md)：部分重叠，保留润色失败静默回退和 30 秒预算，本篇补齐哪些回复应当失败。
- [原生语音设置接口](../../implemented/architecture/2026-10-11-windows-settings-voice-host-api.md)：无关，配置探测与实际转写润色是不同入口。
- [macOS 语音设置裁剪](../../implemented/simplification/2026-10-16-macos-voice-settings-prune.md)：无关，本篇处理实际语音请求，不恢复旧设置控件。

## Alternatives considered

- 只检查 HTTP 成功和非空正文：能兼容更多服务并使用模型已经写出的文字，但无法告诉宿主这只是半段结果，会覆盖完整转写。
- 强制 `finish_reason: stop`：判断最简单，但既有兼容服务可能省略或置空该字段；只拒绝明确报告的截断与过滤，避免扩大兼容性变化。
- 在各宿主各自检查：靠近转写回退位置，但会复制协议规则。共享 C++ 入口是 macOS 与 Windows 的共同路径，应在这里拒绝不可用结果；Linux 用户会话服务的独立 HTTP 实现按同一规则处理。

## Verification

新增回归在旧实现下先失败：`shared-voice-http-transport` 接受了 `length` 的非空结果，`voice-polish-request` 未回退原始转写。回归通过真实共享 curl 适配器与 macOS 原生 `MSIMEHTTPVoiceRequest` 发出本机合成 HTTP 请求，不访问外部服务、不使用真实输入或凭据。修复后两项通过，独立审查复跑也通过。`pnpm run verify-notes`、`git diff --check`、macOS 完整构建和本地 quick 门禁通过；quick 包含 Linux 90 项测试和 Windows 168 项可移植测试。本机缺少 MinGW 交叉工具链，Windows 完整构建被跳过；HarmonyOS 真打包因缺少暂存库与资源被跳过。

Linux 的 `scripts/test-linux-voice-polish-reply.py` 经真实 `polish` → `polish_request` → HTTP worker 子进程访问合成回环服务，12 种回复覆盖明确失败与兼容成功响应。旧实现出现 7 个预期失败，修复后通过。测试位于自动发现的 `scripts/test-*.py`，仓库契约门禁实际执行通过；不支持 Unix 信号的主机明确跳过。回环请求不经过代理，不访问外部模型服务。独立审查与本地 quick 门禁通过，后者包含 Linux 桌面外壳编译、原生 IBus/Fcitx5 构建、90 项原生测试，以及版本安装产物检查。未进行 Linux 系统输入法中的麦克风录音和交互验收。

## Consequences

- 明确报告截断、过滤或错误的回复走现有失败回退，完整转写得以保留。失败没有新增日志或服务端错误透传。
- 未提供结束原因或提供 `null` 的旧服务继续兼容，因此仍不能保证这些服务的输出完整性。
- 覆盖范围是使用共享 C++ 润色入口的 macOS、Windows 原生流程，以及 Linux 用户会话语音服务。Android、iOS、HarmonyOS 的独立语音实现不受本篇验证覆盖。
- 请求参数、润色预算和错误提示沿用现有规则；明确失败时不再展示服务已经生成的部分正文。

## Sources

[OpenAI Chat Completions 响应说明](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/retrieve) 将 `length` 定义为输出预算耗尽，将 `content_filter` 定义为正文因过滤而省略。官方 Python SDK 的 `lib/_parsing/_completions.py` 在解析前也对这两种结束原因抛出异常。
