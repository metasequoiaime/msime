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
- 凭据跟着换名。逐个平台查了密钥按什么存：Windows、macOS、Android、HarmonyOS 的密钥在共享偏好的 `ai_assistant.token`/`tokens`、`voice_input.polish_token`/`polish_tokens` 里，按服务商或接口来源存，不看模型；iOS 键盘 AI 的钥匙串条目按接口来源（`KeyboardAIService.query`），iOS App 的按 `scope|来源`（`ServiceTokenStore`），也不看模型。只有 Linux 把密钥写在私有文件里并绑定保存时的模型：`ai-provider.json` 的每个条目带 `endpoint`、`model`、`token`，online provider 的 `ai_config_matches` 要求设置页传来的 provider、endpoint、model 三项都和条目相等，请求发的也是条目里的模型；`voice-provider.json` 的润色条目同理，voice provider 的 `matching` 比 `polish_model`。偏好换了名、文件没换，两边就对不上，Linux 的 AI 联想、释义、润色都停用。所以：
  - 换名表 `RETIRED_VOLCENGINE_MODELS` 和 `volcengine_successor_model` 改成 `pub`，`export_voice_provider_catalog` 把它导出成 `msime_voice_provider_catalog.py` 的 `VOLCENGINE_SUCCESSOR_MODELS`，换名规则只写在 Rust 一处，Python 那份由 `--check` 守着。
  - 两个 provider 共用的 `msime_provider_runtime.py` 加了 `volcengine_successor_model`，online provider 的 `load_ai_config` 和 voice provider 的 `load_config` 读文件时对 `volcengine` 条目照表换名，接口地址的条件与 Rust 相同。这样匹配对得上，请求发的也是接替模型，密钥原样用。
  - 桌面设置（`linux_provider_credentials.rs` 的 `ai_status`）报给设置页的绑定同样换名，设置页不再提示「已保存的凭据绑定……与上方设置不一致」。语音那边不用改：设置页只能为 openai、groq、siliconflow、deepseek 保存润色凭据，`volcengine` 的润色条目只可能是手写的，provider 读文件时已经换过名。
  - 文件本身都不改写，等用户下一次保存，与偏好的迁移一致。
- 方舟上的豆包 Seed 请求一律发 `thinking.type = disabled`。依据是 2026-11-05 查的两份官方文档：深度思考文档（https://docs.volcengine.com/docs/82379/1449737 ，页面更新于 2026-11-03）的支持列表里，`doubao-seed-2-0-mini-260428`、`doubao-seed-2-0-lite-260428` 以及列出的其他 `doubao-seed-*`（2.1 pro/lite/turbo、2.0 pro/code、character、evolving）都是「支持 enabled（默认）、disabled」，也就是默认开着思考；对话 API 文档（https://docs.volcengine.com/docs/82379/1494384 ，页面更新于 2026-11-02）把 `thinking.type` 定义为 enabled / disabled 两个取值，disabled 是「关闭思考模式，模型直接回答问题，不进行思考」。联想有 8 秒预算、润色有 3 到 60 秒不等的预算，都是用户在等的短请求，关掉思考是对的。
  - 判定是「接口地址的主机属于 `volces.com`（地址缺失时看 `volcengine` 标签）并且模型名去掉首尾空白、转小写后以 `doubao-seed-` 开头」。不按主机一律发：同一份支持列表里 `glm-5-3-flash-260828` 只支持 enabled（「始终启用思考，不再支持禁用思考」），`kimi-k3-1-pro-261024` 不用 `thinking` 参数，给它们发 disabled 会被拒。`ep-` 开头的接入点 ID 看不出背后是哪个模型，不发。别家网关上的同名模型也不发，`thinking` 是方舟的字段。
  - 只发文档里有的 `thinking.type`，不发 `reasoning_effort`（文档里它是调节思考长度的另一项，值域和默认值各模型不同），也不改 `max_tokens`。
  - 每个平台各自拼请求体，所以每个拼对话请求的地方都改了，规则相同：Rust 的 `chat_completion_body`（AI 联想，Windows、macOS、Android、iOS 键盘、HarmonyOS 共用）、`chat_translation_http_request`（AI 释义）、`credential::probe::test_chat`（配置测试）、`ai::settings::test_body`（AI 页的润色测试，签名加了接口地址）；共享 C++ 的 `polish_cloud_text`（Windows、macOS 的润色，新增 `disables_ark_seed_thinking`）；Linux online provider 的 `ai`、`ai_translation`、`ai_credential_test`、`ai_polish_test` 和 voice provider 的 `polish`、`polish_credential_test`；Android 的 `AiPolishHttpTransport`；HarmonyOS 的 `HarmonyVoicePolisher` 和设置页的 `aiTest`、`chatCredential`（判定放在 `VoicePolishRequestPolicy.disablesArkSeedThinking`）；iOS 的 `AppServicesBridge.polishBody`（App 的润色，签名加了接口地址）和键盘扩展的 `CustomServiceClient`（AI 回复）。DeepSeek 已有的关思考规则原样不动。

## Alternatives considered

- 只改目录、不迁移：改动最小，新用户立即能用。不采用：选过这个预设的老用户文档里存的正是旧名字，他们升级后照样 404。
- 读出时清空旧名字，让各宿主补默认值（EveryAPI 那次的做法）：以后默认值再变时不必再迁移。不采用：AI 联想的 `model` 留空就是未配置，不会回退到目录默认；而且 `doubao-seed-1.6` 是备选不是默认，清空会把选了 lite 一档的人降到 mini。
- 默认改成 `doubao-seed-2-1-lite-260915` 或 `doubao-seed-2-1-turbo-260628`（模型列表里标「推荐」）：版本更新。不采用：它们没有 LTS 标记，下线公告给旧名字指定的接替也不是它们；选 LTS 能少一次很快到来的再迁移。
- 方舟的思考先不管，另开 issue：对话 API 文档把 `max_tokens` 定义为「模型回答最大长度」，含思维链的上限另有 `max_completion_tokens`，宿主发的是前者，512 的上限不会被思考挤掉，请求照样能成功。不采用：能用不等于好用，默认模型换成默认思考的 Seed 2.0 以后，每次联想和润色都要先等一段思维链，8 秒的联想预算很可能不够；而按模型系列判断的规则一次就能写清楚。
- 按主机一律发 `thinking.type = disabled`：规则最短。不采用：方舟上有只支持 enabled 的 `glm-5-3-flash-260828`，发 disabled 会被拒。
- 用 `reasoning_effort = minimal` 或 `none` 关思考：深度思考文档里这两个值也是「关闭思考」。不采用：它是调节思考长度的参数，各模型的取值范围和默认值不同，支持表的排版也看不清每个模型对应哪一档；`thinking.type` 是对话 API 文档里写明的开关，所有 `doubao-seed-*` 都支持。
- Linux 凭据只改设置页的提示，或者让匹配忽略模型：改动小。不采用：provider 发请求用的是文件里的模型，不换名的话即使对上了，发出去的还是已下线的名字，照样 404；忽略模型又会放掉「密钥只给保存时那一套接口和模型用」这条约束。
- 读到旧名字时直接改写 Linux 的私有文件：以后不必再换。不采用：provider 每次请求都只读这个文件，在读路径上写用户的私有文件要处理并发保存和权限，偏好那边也是只在内存里换、等下次保存，两边保持一致更简单。

## Consequences

- 选预设火山方舟的新用户开箱即用，老用户升级后自动换到接替模型。
- 方舟上的豆包 Seed 请求都关着思考。没有方舟密钥，实际延迟、AI 联想 8 秒预算内能否稳定返回都没有在真实服务上量过。
- 方舟上的非 Seed 模型（GLM、Kimi、DeepSeek 等）和 `ep-` 接入点照旧不带 `thinking`，用各自的默认值；以后方舟的支持表变了（比如某个 Seed 模型不再支持 disabled），要改的是各平台的 `disables_ark_seed_thinking` 这一个判断。这条规则现在散在 Rust、C++、Python、Java、ArkTS、Swift 六处，各有测试钉住同一组例子。
- Linux 用户升级后不用重新保存凭据，AI 联想、释义和润色照常可用，请求发接替模型。私有文件里留着旧名字，直到下一次保存。
- 读偏好时多两次字符串比较，Linux provider 每次读私有文件时对 `volcengine` 条目多一次比较。默认值仍散在 Rust 目录与手写的 `VoiceProviders.cpp` 两处，下一次方舟下线 Seed 2.0 时要再加一行接替关系，Python 那份由导出脚本跟着更新。

## Verification

- `cargo test -p msime-client-core`（含 `retired_volcengine_models_read_as_their_ark_successors`：旧默认、旧备选、方舟其他地域、留空地址都换；接入点 ID、别家网关地址、`volces.com.example`、`custom` 服务商下的同名值都不动；槽位与 `salvaged_from` 路径同样生效）、`cargo test -p msime-client-providers`、两个 `export_*_provider_catalog --check`、`cargo clippy -p msime-client-core -p msime-client-providers --all-targets -- -D warnings` 经 rbuild 通过。
- `apps/desktop` 的 vitest：`tests/voice/voice-catalog-projection.test.ts`、`tests/settings/ai-catalog-projection.test.ts`、`tests/settings/settings.test.tsx`、`tests/settings/provider-presets.test.tsx` 通过。
- `platforms/linux/tests/voice/provider_catalog.py`、`platforms/harmony/tests/run.sh` 通过。
- 凭据换名：`linux_provider_credentials.rs` 的 `reports_a_retired_ark_model_binding_as_its_successor`；`platforms/linux/tests/provider/credential_test_contract.py` 的 `test_ai_retired_ark_model_bindings_read_as_their_successors`（绑在旧名字上的密钥能通过匹配、请求发接替模型，别家网关和 `volces.com.example` 不换，文件不改）和 `test_voice_retired_ark_polish_model_reads_as_its_successor`。
- 关思考：`ai::tests::ark_seed_models_get_thinking_disabled`、`credential::probe::tests::credential_probe_turns_off_thinking_for_ark_seed_models`、`ai::settings::tests::test_body_turns_off_thinking_for_ark_seed_models`；共享 C++ 的 `shared-voice-provider-routing`；Linux 的 `test_ark_seed_requests_turn_off_thinking`（覆盖六个请求构造）；Android 的 `AiPolishHttpTransportSmoke`；HarmonyOS 的 `keyboard-logic.test.ts`；iOS 的 `testPolishBodyTurnsOffThinkingOnlyForArkSeedModels`。正例都是方舟上的 Seed（含上海地域和大小写），反例都是 `glm-5-3-flash-260828`、`ep-` 接入点、别家网关和 `volces.com.example`。
- 没有方舟密钥，新模型在真实服务上的 200 响应和延迟没有验证；Windows、macOS、iOS 原生构建、Linux 上的 `msime-desktop` 测试、Android 宿主冒烟交给 CI。
