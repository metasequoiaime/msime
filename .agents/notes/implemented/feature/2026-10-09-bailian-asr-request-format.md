# Agent Note: 阿里云百炼整句识别与识别请求格式字段

Status: implemented

## Problem

#6017：用户希望多一个云端识别服务，具体点名阿里云百炼，理由是 Whisper 识别中文容易出幻觉。百炼的整句识别（Qwen3-ASR，`qwen3-asr-flash`）不是 OpenAI 兼容的 `/audio/transcriptions` multipart 上传，而是 Chat Completions：录音作为 `input_audio` 的 Base64 数据 URL 放进 user 消息，文字在 `choices[0].message.content`。在此之前所有整句服务都是 multipart，各宿主判断「怎么发请求」的方式是各自维护一份 provider 名字白名单（Rust、C++、Java、Swift、ArkTS、Python、Objective-C 各一份），加一种新请求形状要在每份白名单旁边再按名字分支一次。

## Decision

- provider id 为 `bailian`，只接整句识别，复用各宿主现有的「录完上传」批量流程。默认地址 `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions`、模型 `qwen3-asr-flash`，按 2026-10-09 百炼官方文档（`help.aliyun.com/zh/model-studio/qwen-speech-recognition` 与 `qwen-asr-api-reference`）：鉴权是 `Authorization: Bearer`，请求体是 `{"model", "stream": false, "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": "data:audio/wav;base64,..."}}]}]}`，不带 `asr_options`，语种交给模型自动识别。文档推荐业务空间专属地址（`{WorkspaceId}.cn-beijing.maas.aliyuncs.com`），并说明旧域名仍可用；默认用旧域名是因为它不需要用户先填 WorkspaceId，需要时可在接口地址里改。
- 请求格式成为显式契约：client-core 的 `voice::provider::asr_request_format(provider)` 给出 `multipart`、`chat_audio`、`doubao_websocket` 或 `local`，`MobileVoiceProviderConfiguration.request_format` 带着它，`msime_client_mobile_voice_configuration` 的 JSON 是 `requestFormat`，`tauri-mobile-platform` 的 `MobileVoiceTranscriptionRequest` 也带着它，并在 `is_valid` 里要求它就是该 provider 的格式。Android（`VoiceConfiguration` → `VoiceRecognitionActivity` → `HttpAsrRecognizer`）和 iOS 的 Tauri 插件（`MobilePlatformPlugin.swift`）按这个字段挑请求构造，不再看 provider 名字。
- 没有经过 `MobileVoiceProviderConfiguration` 的宿主各留一份同样的对应：共享 C++ 的 `asr_request_format`（Windows 与 macOS 经它调用 `recognize_cloud_asr`）、Linux 语音服务脚本的 `REQUEST_FORMATS`、HarmonyOS 的 `HttpAsrConfigurationPolicy.requestFormat`、iOS 原生设置的 `VoiceProviderPreset.usesChatAudio`。它们的识别和凭据测试也只按格式分支。
- 上传上限：百炼限制请求里的音频含 Base64 编码不超过 10 MB，所以 chat_audio 的 WAV 上限是 7,000,000 字节（client-core `CHAT_AUDIO_MAX_WAV_BYTES`、C++ `chat_audio_max_wav_bytes`、Linux `CHAT_AUDIO_MAX_WAV`），16 kHz 单声道 16 位约 218 秒。macOS、Windows、Linux 都按它截取录音：录到上限就结束录音并识别，与 multipart 录满 20 MiB 的处理相同。三处共用一个判断，C++ 是 `batch_capture_sample_limit_for(provider)`（macOS 的 `sampleLimit` 与 Windows 的采集回调都调它），Linux 是 `capture_byte_limit(provider)`。最初的版本只有 macOS 截取，Windows 和 Linux 录满批量上限（约 655 秒或 `--max-recording-seconds`）后才在上传前整段拒绝，说了四分钟的话全部丢掉，Linux 还只回一个不带说明的失败；review 时发现后改成三处一致。`recognize_cloud_asr` 与 Linux `chat_audio()` 的超限检查留着兜底。Android、iOS、HarmonyOS 的录音本来就只有 60 到 120 秒，远在上限以内。
- 回答解析各宿主都改成先取 `text`，没有时再取 `choices[0].message.content`；`text` 存在但不合法时不回退到 `choices`。
- 设置页（`packages/ui` 的 `ASR_PROVIDER_OPTIONS`、`ASR_PROVIDER_DEFAULTS`、`ASR_SERVICE_PROVIDER_IDS`）、macOS 原生语音设置、iOS 原生语音设置加上「阿里云百炼 · 千问」；`ASR_PROVIDERS` 变成 9 项。凭据测试（`credential::asr`）按格式发一秒静音。
- 不做百炼的实时流式识别，也不接「小米」：前者需要新的跨平台流式会话设计，后者没有明确的公开识别接口。

## Alternatives considered

- **只在每份白名单里加 `bailian`，再在各宿主按名字分支** — 和当初加 EveryAPI、Mistral 一样，改动最少、最容易审。但那两家是同一种请求，只加 id 就够；这一次请求形状不同，按名字分支意味着七种语言里各写一句 `provider == "bailian"`，下一家用 chat_audio 的服务又要全部再找一遍。把格式做成共享层给出的字段，下一家只需改映射表。
- **百炼的 OpenAI 兼容 `/audio/transcriptions`** — 若存在就只是加 id 的小改动。官方文档里 Qwen3-ASR 的 OpenAI 兼容模式只有 Chat Completions 形式，没有 transcriptions 端点，所以这条路不存在。
- **DashScope 原生接口（`/api/v1/services/aigc/multimodal-generation/generation`）** — 能用 `X-DashScope-SSE` 等原生参数，回答结构也更详细。但它的回答形状在文档里前后不一致（`output.text` 与 `output.choices` 两种写法），而 compatible-mode 的 `choices[0].message.content` 和各宿主已有的润色解析一致，宿主改动更小。
- **在请求里带 `asr_options.language`** — 可以让中文识别更稳。但百炼要求混合语种时不要指定语种，而输入法用户常常中英夹杂；其他服务也是不传语种时自动识别，保持一致。

## Consequences

- **收益**：多了一个对中文表现好的云端识别服务；宿主选择请求构造的依据从「provider 名字」变成共享层给出的字段，Android 与 iOS Tauri 插件不再各自维护名字白名单，格式与 provider 不一致的请求会在 `MobileVoiceTranscriptionRequest::is_valid` 被拒。
- **代价与已知上限**：Base64 让上传体积变成 4/3；选百炼时 macOS、Windows、Linux 的单次录音最长约 218 秒，录到上限就结束录音并识别已录下的部分，上传前的长度检查只作兜底。不经过共享配置的宿主（C++、Linux 脚本、HarmonyOS、iOS 原生）仍各有一份格式映射，靠各自的测试和本笔记保持一致。默认地址用旧域名，百炼若停用它，需要把默认值换成业务空间专属地址并引导用户填 WorkspaceId。没有用真实的百炼 API Key 在任何平台上听写过。

## Verification

`cargo test -p msime-client-core voice::provider credential::asr preferences`；`cargo test -p msime-tauri-mobile-platform`；`cargo test -p msime-desktop ios_voice`；共享 C++ 的 ctest `shared-voice-provider-routing` 与 `shared-voice-http-transport`（`tests/transport.py` 在回环服务器上检查 chat_audio 的 JSON 请求体和回答解析）；`apps/desktop` 的 `vitest run tests/voice`；Android 的 `HttpAsrPolicySmoke`；HarmonyOS 的 `platforms/harmony/tests/keyboard-logic.test.ts`；Linux 的 `platforms/linux/tests/voice/provider_request_format.py`；iOS 的 `ServiceTests/voice/CustomServiceTests.swift`（`testBailianSendsTheRecordingAsChatAudioAndReadsTheMessage`）与 `scripts/test-ios-project-config.py`；macOS 的 `VoiceProviderOptionsTest.mm`、`VoiceProviderSettingsKeysTest.mm`。
