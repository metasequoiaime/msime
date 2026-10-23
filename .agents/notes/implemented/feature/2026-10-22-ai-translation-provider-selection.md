# Agent Note: 候选翻译的「AI 辅助」是显式选择的服务

Status: implemented

## Problem

Windows v0.11.1 的翻译服务可选 `ai`，复用「AI 与云联想」页的接口地址、Token 和模型做候选词翻译。新架构的请求管线在 #7078 时已就位（`TranslationQuery.ai_assistant`、各宿主的 `aiTranslationHTTPRequest`），但设置页的选择器只有 `none|tencent|niutrans|custom|account`，用户无法选择它。更关键的是，`selected_translation_services` 当时把「AI 联想开着且有凭据」隐式推导为 AI 翻译——用户选「关闭」时若联想开着仍会发 AI 翻译请求，none 不是 none；反过来选了 AI 但配置没填好时会被推导成 Off，与「关闭」不可区分，设置页无法提示。

## Decision

新增顶层布尔偏好 `translation_ai`，语义与 `translation_account` 同构：用户显式选择「AI 辅助」翻译时为真，随选择器互斥写入（选其他服务时清除）。`selected_translation_services` 用它而不是 AI 联想开关来判定 `TranslationService::Ai`：联想开着但没选 AI 翻译时 provider 为 `none`；选了 AI 但配置不可用（缺 Token、模型或接口地址不合规）时 provider 仍是 `ai`、`ai_assistant` 一栏留空——与 NiuTrans 缺凭据「保持选中」的既有惯例一致，宿主不发请求、设置页显示「请先在「AI 与云联想」页面填写接口地址、Token 和模型」，而不是静默不翻译。

与 Windows 一致，AI 翻译独立于联想总开关：`ai_assistant.enabled` 不参与选中判断；填入查询的配置快照把 `enabled` 置真，因为这一栏的语义是「允许用这份配置发 AI 请求」，宿主和 `chat_translation_http_request` 都按它门禁。填入条件与 Windows `IsUsableConfig` 对齐：服务商在共享目录内、模型非空且有界、接口地址通过 `ai::endpoint::is_allowed`、按来源键找得到可用 Token（Ollama 不鉴权除外）或会话注入的凭据匹配同一 endpoint。

传输复用既有 Chat Completions 路径（`msime_client_ai_translation_http_request` → `chat_translation_http_request` → `HttpTransport`），不新建第二套客户端。凭据测试 `translation.ai` 在 `credential::test_service` 里路由到与 `ai.assistant` 相同的对话探测，Linux provider socket 的 `test_credential` 白名单同样放行。设置页选中 AI 辅助时显示复用说明和成本/延迟提示（AI 模型逐条生成释义，按 token 计费且比专用翻译接口慢），与 Windows 「复用 AI 辅助配置」区块的文案对齐，另加一行「当前服务商与模型」摘要。

## Existing notes

[候选翻译复用 AI 配置](2026-10-17-ai-candidate-translation.md)是本次翻转的对象之一：那篇建立请求管线时用「AI 联想开着且有凭据」隐式选中 AI 翻译，本篇改为显式 `translation_ai`，管线本身不变，两篇互链。[翻译凭据偏好](../../implemented/simplification/2026-10-16-translation-credential-preferences.md)确立互斥开关推导服务的模型，`translation_ai` 沿用同一模型而非新增独立 `translation.provider` 字符串。

## Alternatives considered

- **沿用隐式推导（AI 联想开着即翻译）**：零改动，但「关闭」挡不住 AI 翻译请求，且无法区分「未选择」和「已选但配置不可用」，不满足任务要求的未配置提示，不采用。
- **新增 `translation.provider` 字符串单选字段**：与 Windows 配置面最贴近，但打破新架构「enabled 互斥 + `translation_account` 标志」的既有模型，迁移和回读都要为单一字段开特例；`translation_ai` 布尔位以最小增量表达同样的显式选择，不采用。
- **要求 `ai_assistant.enabled` 才算选中**：实现最小，但把两个功能绑死——Windows 明确支持「只想用翻译、不开 AI 联想」，且关联想会顺带掐掉翻译的选择会让用户困惑，不采用。
- **AI 翻译专属 HTTP 客户端**：任务硬约束禁止另起一套；共享探测与请求路径已经覆盖需求，不采用。

## Verification

`cargo test -p msime-host-api translation_query_carries_the_selected_ai_assistant_config`：AI 开着未选时 provider 为 `none`；选中后 provider 为 `ai` 且 `ai_assistant` 快照带 endpoint/model/enabled/token；配置不可用时 provider 仍为 `ai`、凭据留空。该测试在回退 `ffi/providers.rs` 至基线后失败（隐式推导给出 `"ai"` 而非期望的 `"none"`）。

`apps/desktop` vitest `translation-candidate-settings.test.ts` 新用例：选中 AI 辅助写入 `translation_ai: true` 并关掉其余服务，回显为 `ai`；AI 缺模型或 Token 时选择仍为 `ai` 且 `aiTranslationReady` 为假。该用例在回退 `use-translation-settings.ts` 至基线后失败（`translation_ai` 为 `undefined`）。

## Consequences

`translation_ai` 是新偏好键，旧文档缺省为假、行为同「关闭」，无迁移负担。行为变化：此前「AI 联想开着」会顺带让候选翻译走 AI，现在必须显式选择——这修复了 none 不挡 AI 请求的问题，但若有人依赖隐式行为，升级后需在设置页重新选择。macOS、Windows 新架构与 Linux provider socket 已消费 `ai_assistant` 一栏，无需宿主侧改动；选中 AI 且配置齐全时查询字段让三端直接生效。
