# Agent Note: Linux 配置测试按模型族改写请求体，支持 GPT-6 家族

Status: implemented

## Problem

#11833：Linux 在「AI 辅助」「语音输入 → 润色」里把 OpenAI 的模型改成非默认的 `gpt-6-luna`、`gpt-6-sol` 之类后，点「测试配置」永远失败，改回默认模型就能通过。重新保存 API Key 也没用。

根因在 Linux 的 Python provider，不在 Rust。桌面宿主的配置测试走 `crates/client-providers/src/credential/probe.rs`，那里已经按模型族把 `max_tokens` 换成 `max_completion_tokens` 并加 `reasoning_effort`（`gpt-6-luna`/`gpt-6-sol` 用 `none`，`gpt-6-astra`/`gpt-6.1-sol`、`glm-5.3` 用 `low`）。Linux 的 `msime-linux-online-provider` 的 `ai_credential_test` 与 `msime-linux-voice-provider` 的 `polish_credential_test` 一直只发 `{"max_tokens": 1}`，GPT-6 家族不认这个字段、且强制思考，服务直接拒绝，于是测试恒为失败。0.16.1 到当前 develop 都没有改过这两处。

## Decision

- 在 `platforms/linux/scripts/msime_provider_runtime.py` 新增 `probe_chat_body(model, max_tokens=1)`：照 `probe.rs` 的同一条规则——先取模型名最后一个 `/` 之后的部分并小写，`gpt-6-luna`/`gpt-6-sol`（含 `-` 后缀）用 `reasoning_effort: none`，`gpt-6-astra`/`gpt-6.1-sol`、`glm-5.3` 用 `low`；命中时移除 `max_tokens`、写 `max_completion_tokens: 512` 与 `reasoning_effort`，其余模型保持 `max_tokens` 的小探测预算。
- 两个 provider 的配置测试改用这个辅助：在线 provider 的 `ai_credential_test`、语音 provider 的 `polish_credential_test`。deepseek、火山方舟豆包、siliconflow 的 `thinking` 处理保持原样，照旧在辅助之后追加。
- 只是配置测试。真实的候选、释义、润色请求仍发 `max_tokens`，与本 issue 的现象无关；要不要一并适配是另一项改动，本次不做。

## Alternatives considered

- **把所有请求都换成 `max_completion_tokens`**：OpenAI 兼容网关里仍有一些只认 `max_tokens`，统一改会打断那些服务；`probe.rs` 也是只对列明的家族改写。
- **把探测预算从 1 提到 512 给所有模型**：能修好这一例，但每次测试都会多花输出预算，而 `probe.rs` 的注释已经把「这是上限、不要求生成 512 个 token」写清楚了——只有强制思考的家族需要留余量。
- **只在在线 provider 里改，语音润色不改**：issue 报的就是两个入口都失败，漏一个等于只修一半。

## Consequences

- 非 GPT-6 家族、非 glm-5.3 的模型，请求体与之前逐字节相同。
- GPT-6 家族的配置测试会按 `reasoning_effort` 与 512 的 `max_completion_tokens` 发请求；测试本身只问一句 `Reply OK`，不读正文。
- 未验证：真实 OpenAI 账号上的 `gpt-6-luna`/`gpt-6-sol` 测试（本机没有凭据）；本机 macOS 也没有 Linux 的宿主库，`credential_test_contract.py` 里依赖宿主库的既有用例在本机跑不起来（与本次改动无关），新加的 4 个用例不依赖宿主库。

## Verification

- `python3 platforms/linux/tests/provider/credential_test_contract.py ProbeChatBodyTest`：4 个新用例通过，覆盖 GPT-6 家族改写、其他模型保持小预算、在线 `ai_credential_test` 与语音 `polish_credential_test` 发出的请求体。
- `python3 -m py_compile` 两个 provider 脚本通过。
- 未跑：Linux 上的完整 `credential_test_contract.py`（既有 `test_ark_seed_requests_turn_off_thinking` 需要宿主库，在本机 macOS 上本来就失败）、真实 OpenAI 端点的配置测试。合并后请报告者在 Fedora 上用 `gpt-6-luna` 复验。
