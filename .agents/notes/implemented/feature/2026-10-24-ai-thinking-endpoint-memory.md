# Agent Note: DeepSeek 思考强度与 AI 每提供方接口地址/模型记忆

Status: implemented

## Problem

Windows 端 `ai_assistant.thinking`（取值 `disabled|low|high|max`）控制 DeepSeek 的思考强度，`voice_input.polish_thinking` 控制语音润色同款能力；`ai_assistant.endpoint_<id>`/`model_<id>` 共 12 对槽位让每个服务提供方各自记住接口地址和模型。新架构两处都缺：AI 候选和润色的请求体写死 `{"type":"disabled"}`，endpoint/model 只有当前提供方的一对平铺字段，切走再切回来自定义值被目录默认值顶掉。

## Decision

偏好层 `AiAssistantPreferences` 新增 `thinking: String`（默认 `disabled`）、`endpoints`/`models: BTreeMap<String, String>`（与 `tokens` 同构，键为提供方 id）；`VoiceInputPreferences` 新增 `polish_thinking: String`，全部 `serde(default)` 兼容旧文档。

请求体形状按旧 Windows 实现（`_refs/msime-windows` 的 `ai_assistant_request.cpp` 和 `voice_providers.cpp`）确认，不是按 API 文档推断：

- DeepSeek 且档位非 `disabled`：`thinking:{"type":"enabled"}`、`reasoning_effort:<档位>`，`max_tokens` 从 512 提到 8192（思考 token 计入输出上限），请求预算从 8s 提到 30s。
- DeepSeek 且 `disabled` 或非法档位：仍写 `{"type":"disabled"}`。
- 其他厂商：不写 `thinking`。厂商判定按接口地址的主机（`deepseek.com` 及其子域），不看设置页标签——标签是 `deepseek` 但地址在别家的槽位不发厂商字段（DeepSeek 的 `thinking.type` 会被 MiniMax 等服务整包拒绝）；只有地址缺失时才让标签说话。`AiProviderConfig` 缓存键计入 `thinking`，档位切换使在途旧档回答失效。

`chat_completion_body` 签名加 `endpoint`/`thinking` 两个参数；翻译路径照旧只按标签写 disabled，不受新偏好影响。`shared/voice` 的 `polish_cloud_text` 加 `thinking` 参数，按同一套地址归属逻辑处理 DeepSeek；SiliconFlow 的 `enable_thinking:false` 同步改成按地址归属判定。macOS（`HTTPVoiceRequest.mm`/`InputController.mm`/`SharedVoicePreferences.h`）与 Windows（`VoiceInputSession`、`server_main`）宿主都接入了 `polish_thinking`。

槽位语义照 Windows：切换提供方先把旧提供方的现值写回槽位（与目录默认相同就删键，空槽自动跟上默认值升级），再按新提供方的槽位回填，没有槽位用目录默认值；编辑接口地址/模型时同步当前提供方的槽位。共享设置页 `ai-provider-update.ts`、Windows 原生设置的 `provider_update`/`select_ai_provider`/编辑处理器、Android 经 `msime_client_ai_provider_update` FFI 的 `provider_update` 三处同一语义；`AiProviderFields` 带上 `endpoints`/`models`，Java 侧 `switchProvider` 把槽位随补丁写回文档。`test-reference-config-coverage.py` 登记参考模板 12 对 `endpoint_*`/`model_*` 与 `thinking`/`polish_thinking` 到共享字段的映射。

槽位缺键或值为空字符串都使用目录默认值，接口地址和模型独立回填。共享设置页不能用 `??` 把空字符串当成自定义配置；切换补丁保留其他槽位且不修改输入对象。公共 C 头文件说明完整的六字段响应，旧请求省略两个映射仍可用；宿主必须随补丁保存 `endpoints`/`models`，否则再次切换会丢掉记忆。真实 C ABI 回归覆盖旧请求、空槽回退、两个服务商的自定义配置往返、严格响应字段及凭据字段拒绝，共享页回归覆盖两个空槽和分别只有一个空槽。

设置页给两个真实控件：AI 页「思考强度」下拉只在提供方选中 DeepSeek 时显示；语音润色页的「润色思考强度」同样只在 DeepSeek 时显示，说明里写明 30 秒超时直接上屏原文。

## Existing notes

未检索到覆盖同一决定的旧笔记；A7/A8 与既有凭据槽位设计同族（`tokens`/`polish_tokens` 的先例），本篇沿用其「平铺字段为当前值、map 为每提供方记忆」的形状。

## Alternatives considered

- 按提供方标签判定厂商字段（标签即厂商）：实现最省，但自定义槽位可以装任何 OpenAI 兼容地址，标签和地址不一致时会给不支持的服务发 `thinking.type`——Windows 专门为此按地址判定，保持一致。
- 槽位存全量值（连默认值也写）：文档更噪，且提供方默认值升级后旧槽位压着新默认值；只在自定义时写键与 Windows `AiAssistantStoredValue` 空值语义等价。
- 把 thinking 做成枚举类型：落盘值与 Windows 模板键一致（字符串档位），字符串字段加使用端 `IsValidThinkingLevel` 校验已足够，非法值按关闭处理。
- Linux provider 脚本接入 thinking：Linux 润色/AI 走各自的 Python 脚本，本次只覆盖共享 Rust/C++ 路径；Linux 脚本仍写死 `thinking.type=disabled`，且 Linux 的 8 秒请求预算不需要放大——`test-cloud-request-budget.py` 因此按普通档核它的预算。
- 翻译请求也读 `ai_assistant.thinking`：Windows 翻译路径恒为 `{"type":"disabled"}` 且只看标签，保持一致不改。

## 结果与代价

- `chat_completion_body` 与 `polish_cloud_text` 的签名各加了一个参数，`AiProviderFields` FFI 契约多了两个 map 字段（serde 默认兼容旧请求），`AiAssistantProviderConfig` 缓存键变长——旧版本读新文档不会出错，新版本读旧文档按默认档处理。
- 切换提供方不再「带着自定义值走」：自定义值归原提供方所有，这是 A8 语义本身，不是回退。
- 思考开着时单次 AI 请求最慢到 30 秒（普通档仍是 8 秒），润色沿用既有 30 秒预算；翻译路径与 Linux 脚本行为不变。

## Evidence

先红后绿：基线上 `cargo test -p msime-client-providers --lib deepseek_thinking` 失败（`left:{"type":"disabled"} right:{"type":"enabled"}`）；`cargo test --test ai_provider_settings` 基线编译失败（`AiProviderFields` 无 `endpoints`/`models` 字段）；`vitest run tests/settings/ai-provider-update.test.ts` 基线 3 条全红。实现后 `cargo test -p msime-client-providers -p msime-client-core`、`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets`、`pnpm --dir apps/desktop exec tsc --noEmit`、vitest 6 条全绿。未对真实 DeepSeek 端点验证请求被接受，形状以旧 Windows 实现为准。

空槽审计回归在旧共享页下收到空地址，未得到 OpenAI 目录默认地址；修复后共享页 4 项、真实 C ABI 4 项通过。旧四字段 ABI 测试已按现行六字段契约更新，并继续检查凭据字段被拒绝。桌面与 Harmony 设置页的类型检查、构建通过；这里验证的是配置纯函数和接口边界，不涉及真实网络请求。
