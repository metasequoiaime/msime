# Agent Note: AI 配置测试兼容需要思考的模型

Status: implemented

## Problem

配置测试发出的 OpenAI 兼容请求把 `max_tokens` 固定为 1，同时只接受非空的 `message.content`。GLM-5.3-FLASH 等模型强制思考，微小预算可以在正文出现前耗尽；这种结果被折叠成与网络失败相同的提示，用户只能试换模型，无法知道地址和凭据是否有效。

## Decision

同一测试保留最短合成提示，将最大输出预算提高到 512，容纳简短思考与正文；预算是上限，不要求模型生成这些 token。按模型名识别文档明确的能力：GPT-6 Luna / Sol 用 `max_completion_tokens` 并关闭思考，GPT-6 Astra / GPT-6.1 Sol 使用同一预算字段和 `low` 思考；GLM-5.3 系列保留 `max_tokens` 并使用 `low`，不能关闭强制思考。识别时保留原模型名发请求，只处理用于判断的命名空间和快照后缀，未知模型不附加推理参数。注入传输返回 `Result<u16, ProbeFailure>`，宿主的 `{ok, message}` 结构保持原样。HTTP 成功仍要求合法回复正文，不把纯思考当成可以用于输入法的答案。将明确的预算耗尽、无效响应、超时与网络错误分开为固定中文说明；错误不透传服务端文本、凭据或返回内容，不自动重试。接口和 Token 来源规则不变。

## 历史笔记审计

- [Windows AI 设置接口](../../implemented/architecture/2026-10-11-windows-settings-ai-host-api.md) 部分重叠：共享设置与原生窗口仍调用同一配置测试，C 接口的 `{ok, message}` 保持不变。
- [AI 局域网接口](../../implemented/feature/2026-10-08-ai-endpoint-local-http.md) 部分重叠：沿用地址校验、不跟随重定向及局域网 HTTP 直连策略。
- [AI 凭据轮换](../../implemented/bug-fix/2026-10-13-ai-credential-rotation-invalidates-candidates.md) 无关：本次不改实际候选查询或凭据更新。
- [Android 润色与凭据测试](../../implemented/feature/2026-10-12-android-voice-polish-and-credential-test.md) 部分重叠：共用探测逻辑，语音润色的配置解析和协议保持原样。

## Alternatives considered

- 对 GLM 加上关闭思考参数：能减少输出量和延迟，但 GLM-5.3 与 GLM-5.3-FLASH 无法关闭思考，且聚合提供商不能仅靠 provider 判断模型能力。
- HTTP 2xx 或纯思考即算成功：网络和凭据检测更快，但正文不存在的响应无法用于候选或润色，会把“配置可用”误报给用户。保留正文校验并明确说明截断。

## Verification

9 项探测测试通过，覆盖可用正文、预算耗尽、纯思考、无效正文和错误状态。合成 HTTP 服务在思考预算耗尽时不返回正文，验证新请求确实能完成回复。真实 EveryAPI 服务上的 GLM-5.3-FLASH 用同一合成提示复现：预算为 1 时 HTTP 200、正文为空、存在思考、结束原因为 `length`；预算为 512 时 HTTP 200、正文非空、结束原因为 `stop`。请求和正文不写入日志，记录中不含凭据。增加模型参数测试，确保已知 GPT 与 GLM 使用各自支持的字段，未知模型不附加推理参数。共享凭据测试 32 项与宿主接口测试 2 项在初次修复后通过，参数适配后相关探测测试 9 项通过；Rust fmt、client-core 全目标 clippy 与笔记门禁通过。

最终参数适配通过原生 C ABI `msime_client_test_api_credential` 向真实 EveryAPI 服务测试，GLM-5.3-FLASH 与 GPT-6-Luna 均返回成功，分别约 1 秒和 7 秒。GUI 自动化遭遇窗口失去焦点、AX 路径改变，未完成按钮点击验收；这里的真实服务证据来自设置页共用的原生测试函数，当前模型与 Token 没有修改。最终设置组件构建、打包、安装与 deep strict 签名校验通过，quick 门禁通过。iOS 工程修复后的 quick 复核也通过，包含 Windows x64 原生链接、32 位编译与协议构建、Linux 83 项原生测试及 macOS 原生构建；HarmonyOS 完整 ArkTS 构建因缺少前置产物跳过。

iOS CI 的键盘测试 target 漏列候选设置页依赖的 `SettingsSaveFailure.swift` 与 `SettingsScenePhaseReload.swift`，导致找不到保存失败类型与场景重载扩展。服务测试 target 中的应用图标设置页同样依赖场景重载扩展，因此该 target 也需注册 `SettingsScenePhaseReload.swift`。在 `project.yml` 与对应 Xcode 工程补齐两个测试 target 的源注册；工程 plist 校验、两份文件在测试 target 中各注册一次的检查，以及 iOS 模拟器 SDK 上的 Swift 类型检查通过。完整 iOS 模拟器套件仍由 CI 验证。

## Consequences

思考模型的测试可能比旧请求耗费更多 token，实际只产生短合成回复；总超时仍有界限。512 token 并非所有模型和提示都必定足够，截断要有明确提示，不能把连接成功与实际正文可用混为一谈。此改动只修测试，不声称实际 AI 候选请求已完成全部模型适配。

## Sources

[GLM 思考模式](https://docs.z.ai/guides/capabilities/thinking-mode) 明确 GLM-5.3 与 GLM-5.3-FLASH 为强制思考；[对话请求参数](https://docs.z.ai/api-reference/llm/chat-completion) 仍使用 Chat Completions 的消息结构及 `max_tokens`。

[OpenAI 推理参数](https://developers.openai.com/api/docs/guides/reasoning)、[GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna) 与[对话请求字段](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create) 用于限定 GPT 探测参数。
