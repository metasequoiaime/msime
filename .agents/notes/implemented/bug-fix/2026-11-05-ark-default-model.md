# Agent Note: 预设火山方舟的默认模型换成方舟现行的 Seed 2.0 LTS

Status: implemented

## Problem

#10876：预设「火山方舟 · 豆包」的默认模型是 `doubao-seed-1.6-flash`，备选 `doubao-seed-1.6`，由 7fa9f5d094 从 Windows v0.11.1 逐字搬来（见 [服务商目录补齐](../feature/2026-10-22-provider-catalog-expand.md)）。选了这个预设、没有改模型的用户，文本润色和 AI 联想的「测试」都回 HTTP 404。[对话配置测试的 404](2026-11-05-chat-probe-404-hint.md) 已经把 404 的提示改得具体，但没换模型，维护者在 issue 里决定换成方舟现行可用的模型（https://github.com/metasequoiaime/msime/issues/10876#issuecomment-6650507944）。

2026-11-05 查了方舟的两份官方文档：

- 模型下线公告（https://docs.volcengine.com/docs/82379/1350667）第九批：`doubao-seed-1-6-251015`、`doubao-seed-1-6-flash-250828`、`doubao-seed-1-6-250615`、`doubao-seed-1-6-flash-250615` 在 2026-07-10 停止新购，2026-09-21 14:00（UTC+8）停止服务。flash 的建议迁移和到期自动替换都是 `doubao-seed-2-0-mini-260428 [LTS]`，1.6 的都是 `doubao-seed-2-0-lite-260428 [LTS]`。
- 模型列表（https://docs.volcengine.com/docs/82379/1330310）：文本模型里 `doubao-seed-2-0-mini-260428`、`doubao-seed-2-0-lite-260428` 标 LTS；同系列的 `-260215` 版本标「即将下线」（公告第十批，EOS 2026-10-22）。整张表没有带点的 `doubao-seed-1.6` 写法，也没有 Seed 1.6 系列。

所以旧名字在方舟上已经不存在，回 404 `InvalidEndpointOrModel.NotFound` 是预期行为。设置页选预设时会把默认模型写进偏好文档（AI 联想的 `ai_assistant.model`、文本润色的 `voice_input.polish_model`），只改目录救不回已经存下旧名字的用户。

## Decision

- 预设火山方舟的默认模型改为 `doubao-seed-2-0-mini-260428`，备选 `doubao-seed-2-0-lite-260428`，正好是公告给两个旧名字指定的接替模型，都是 LTS。mini 是 Seed 2.0 里最快、最便宜的一档，适合润色和联想这类短请求；lite 能力更强，作为备选。
- 改动的目录：`crates/client-providers/src/ai/provider_catalog.rs`、`crates/client-providers/src/voice/provider_catalog.rs`，以及从它们导出的 `packages/ui/src/settings/ai-provider-catalog.json`、`packages/ui/src/voice/voice-provider-catalog.json`、`shared/ai/AiProviderCatalog.h`、`platforms/ios/App/Services/AIProviderCatalog.swift`、`platforms/harmony/.../VoiceProviderCatalog.ts`、`platforms/linux/scripts/msime_voice_provider_catalog.py`（`export_*_provider_catalog --check` 通过）；手写的 `shared/voice/VoiceProviders.cpp` 的 `default_polish_model`；断言默认值的三处测试。
- 已存的旧名字在读偏好时换成接替模型，文件本身不改写，等下一次保存：`Preferences::retire_unavailable_volcengine_models` 在 `PreferencesStore::read_locked` 和 `Preferences::salvaged_from` 里调用，紧跟 EveryAPI 识别模型的同类退役（[EveryAPI 识别默认模型](2026-10-12-everyapi-asr-default-model.md)）。它看三处：`polish_provider` 为 `volcengine` 时的 `polish_model`；`ai_assistant.provider` 为 `volcengine` 时的 `model`；`ai_assistant.models["volcengine"]` 槽位（配 `endpoints["volcengine"]`）。
- 只在模型名去掉首尾空白后正好是那两个旧名字、而且接口地址留空或主机属于 `volces.com` 时才换。火山方舟槽位里填了别家网关的地址，那边可能真有同名模型，不动；别的服务商下的同名值、用户自己选的模型（包括 `ep-` 开头的接入点 ID）都不动。
- 写接替模型的名字而不是清空：AI 联想的 `model` 为空会被当成没配置（`chat_completion_body` 拒绝、`settle_translation_ai_selection` 认为未配置），润色为了一致也写名字。
- 经共享偏好读设置的宿主（Windows、macOS、Linux、Android、HarmonyOS 以及 iOS 键盘）都走这条路。iOS App 自己的服务配置（UserDefaults 里的 `service.ai.*`）没有火山方舟预设，`AIProviderPreset` 不含它，不需要另做迁移。

## Alternatives considered

- 只改目录、不迁移：改动最小，新用户立即能用。不采用：选过这个预设的老用户文档里存的正是旧名字，他们升级后照样 404。
- 读出时清空旧名字，让各宿主补默认值（EveryAPI 那次的做法）：以后默认值再变时不必再迁移。不采用：AI 联想的 `model` 留空就是未配置，不会回退到目录默认；而且 `doubao-seed-1.6` 是备选不是默认，清空会把选了 lite 一档的人降到 mini。
- 默认改成 `doubao-seed-2-1-lite-260915` 或 `doubao-seed-2-1-turbo-260628`（模型列表里标「推荐」）：版本更新。不采用：它们没有 LTS 标记，下线公告给旧名字指定的接替也不是它们；选 LTS 能少一次很快到来的再迁移。
- 同时给方舟的请求加 `thinking.type = disabled`：Seed 2.0 默认开深度思考（深度思考文档 https://docs.volcengine.com/docs/82379/1449737 写明 mini、lite 支持 enabled（默认）和 disabled），关掉会更快。这次不做：方舟对话 API 文档（https://docs.volcengine.com/docs/82379/1494384）把 `max_tokens` 定义为「模型回答最大长度」，把包含思维链的上限另列为 `max_completion_tokens`，宿主发的是前者，512 的上限不会被思考挤掉，所以不影响能不能用；关思考要改六个平台的请求构造（Rust、共享 C++、Linux 两个脚本、HarmonyOS、探测），而方舟上有只支持 enabled 的模型（如 `glm-5-3-flash-260828`），按主机一律发 disabled 会让选了它的用户被拒。应该按模型而不是按主机判断，单独做。

## Consequences

- 选预设火山方舟的新用户开箱即用，老用户升级后自动换到接替模型。
- 默认开着思考，联想和润色的延迟会比关思考时长；没有方舟密钥，实际延迟、AI 联想 8 秒预算内能否稳定返回都没有在真实服务上量过。
- Linux 的私有凭据文件（`ai-provider.json`、`voice-provider.json`）把密钥绑定在保存时的模型上，运行时要求偏好里的模型与它一致。偏好换成接替模型后两边不一致，Linux 的 AI 联想和润色会停用，设置页显示「已保存的凭据绑定……与上方设置不一致」，点一次保存（不用重填密钥）就恢复。这次没有改 Linux 脚本去认旧名字。
- 读偏好时多两次字符串比较。默认值仍散在 Rust 目录与手写的 `VoiceProviders.cpp` 两处，下一次方舟下线 Seed 2.0 时要再加一行接替关系。

## Verification

- `cargo test -p msime-client-core`（含 `retired_volcengine_models_read_as_their_ark_successors`：旧默认、旧备选、方舟其他地域、留空地址都换；接入点 ID、别家网关地址、`volces.com.example`、`custom` 服务商下的同名值都不动；槽位与 `salvaged_from` 路径同样生效）、`cargo test -p msime-client-providers`、两个 `export_*_provider_catalog --check`、`cargo clippy -p msime-client-core -p msime-client-providers --all-targets -- -D warnings` 经 rbuild 通过。
- `apps/desktop` 的 vitest：`tests/voice/voice-catalog-projection.test.ts`、`tests/settings/ai-catalog-projection.test.ts`、`tests/settings/settings.test.tsx`、`tests/settings/provider-presets.test.tsx` 通过。
- `platforms/linux/tests/voice/provider_catalog.py`、`platforms/harmony/tests/run.sh` 通过。
- 没有方舟密钥，新模型在真实服务上的 200 响应和延迟没有验证；Windows、macOS、iOS 原生构建交给 CI。
