# Agent Note: EveryAPI 语音识别的默认模型改成它实际提供的 Seed ASR

Status: implemented

## Problem

EveryAPI 作为识别服务加进来时（905925fdf9），默认模型照搬了 Apple 客户端 `VoiceProviderPreset` 的 `openai/whisper-large-v3-turbo`，各宿主的默认值表都跟着写了这个名字。但 EveryAPI 没有这个模型的渠道：2026-10-12 用 `everyapi models list`（自动路由，覆盖账号可用的全部 11 个路由组，含 Media 路由）和 `models pricing` 查，语音识别模型只有 `volc.seedasr.sauc.duration`。按宿主实际发出的 multipart 请求（`model`、`language`、`file` 为 16 kHz 单声道 WAV）实测 `/v1/audio/transcriptions`：

- `openai/whisper-large-v3-turbo`：HTTP 503 `model_not_found`（「分组 自动 下模型 … 无可用渠道」）。
- `volc.seedasr.sauc.duration`：HTTP 200，`{"text":"你好，今天天气怎么样？"}`，与合成语音的原文一致。

所以选了 EveryAPI、没有手改模型的用户，在每个平台上都识别失败。共享设置页切到 EveryAPI 时还会把默认模型写进 `voice_input.asr_model`，只改默认值救不回这些已经存下旧值的文档。

## Decision

- EveryAPI 的识别默认模型在所有副本里都改成 `volc.seedasr.sauc.duration`，候选模型列表去掉 whisper，只剩它。改动的副本：`crates/client-providers/src/voice/provider.rs`、`shared/voice/VoiceProviders.cpp`、`packages/ui/src/voice/voice-providers.ts`、`platforms/windows/settings/VoiceSettings.h`、`platforms/macos/src/voice/VoiceProviderSettings.mm`、`platforms/linux/scripts/msime-linux-voice-provider`、`platforms/harmony/.../HttpAsrConfigurationPolicy.ts`、`platforms/ios/App/Services/CustomService.swift`，以及各自断言默认值的测试。
- 已存的旧值在读出时退役，文件本身不改写：
  - 共享偏好：`PreferencesStore::read_locked` 在校验之后调用 `VoiceInputPreferences::retire_unavailable_asr_model`。`asr_provider` 是 `everyapi`、`asr_model` 去掉首尾空白后等于旧名字时，清空 `asr_model`，由共享解析补上现在的默认值。经共享偏好读设置的宿主（Android、HarmonyOS、Windows、macOS、Linux 设置页）都走这条路。
  - iOS 的服务配置存在自己的 UserDefaults 里（`service.voice.model` 和 `service.voice.presets.everyAPI.model`），`CustomServiceConfiguration.currentVoiceModel` 在 `load(.voice)` 和 `loadVoicePreset` 里做同样的替换。
- 只认这一个旧名字，而且只在 EveryAPI 下认。用户自己填的其他模型原样保留；其他服务下同名的值不动。

## Alternatives considered

- **只改默认值，不迁移**：改动最小，新用户立即受益。不采用：共享设置页会把默认模型写进文档，现有的 EveryAPI 用户恰好全是存了旧值的那批，他们照样失败，而且看不出原因。
- **加载时把文件改写成新值并保存**：迁移一次就永久生效，不用每次读都判断。不采用：`read_locked` 是只读路径，键盘和设置页并发读同一份文件，读里夹一次写，会和 compare-and-swap 的 revision 语义打架。读出时清空的代价只是一次字符串比较，文件等用户下次保存时自然改写。
- **清空改成写入新模型名**：设置页显示得更直观。不采用：留空就是「用该服务的默认值」，以后默认再变时不必再迁移一次；各设置页对空值本来就显示默认模型。

## Consequences

- 收益：EveryAPI 选上即可识别，不用另填模型；旧用户升级后自动恢复。
- 代价：读偏好时多一次判断；默认值依然散在八份副本里，下一次改默认值还要逐个改，这次也没有把它们收拢成一份。EveryAPI 的模型目录会随渠道变化，`volc.seedasr.sauc.duration` 将来下线时需要再迁移。
- 验证：`cargo test -p msime-client-core -- preferences:: voice::provider`（含 `the_retired_everyapi_asr_model_falls_back_to_the_current_default_on_load`）；`platforms/harmony/tests/run.sh`；`apps/desktop` 的 vitest（`tests/voice`、`tests/settings`）。iOS 的 `testRetiredEveryAPIVoiceModelReadsAsTheCurrentDefault`、macOS 的 `VoiceProviderSettingsKeysTest`、`shared/voice/tests/routing.cpp` 和 `msime-desktop` 的 `ios_voice_batch_configuration_covers_everyapi_and_mistral` 本地没有跑：`msime-desktop` 的构建脚本要先有 macOS 引擎资源，其余几个要各自平台的完整构建，交给 CI。
