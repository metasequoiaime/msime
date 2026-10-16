# Agent Note: AI 候选请求在转换协议的网关和不支持 JSON 模式的模型上也能拿到回答

Status: implemented

## Problem

AI 候选请求（`crates/client-providers/src/ai.rs` 的 `chat_completion_body`，Linux 是 `msime-linux-online-provider` 的 `ai`）带 `response_format: {"type": "json_object"}`，只在 system 提示里要求输出 JSON，user 消息是不含 "json" 一词的请求对象。回答由 `parse_chat_completion_response` 按严格 JSON 解析正文。两种常见配置因此拿不到任何 AI 候选，AI 座位上的「…」等完只会换成「—」：

- 把 Chat Completions 转成 Responses API 的网关（实测 EveryAPI 上的 `gpt-6-luna`）把 system 消息放进 `instructions`，JSON 模式的 "json" 检查只看 input，于是每次请求都是 HTTP 400 `Response input messages must contain the word 'json'`。用同一请求体连发 3 次，3 次都是 400。
- 不支持 `response_format` 的模型（实测 EveryAPI 上的 `claude-haiku-4-5`）照常回答，但把 JSON 包在 ```` ```json ```` 代码块里，严格解析失败，回答被当成无效。

## Decision

- 请求的 user 消息在原有的 `segmented_pinyin`、`context`、`candidate_limit` 之后加一个 `"output_format": "json"` 字段。Rust 用带 `#[serde(flatten)]` 的局部结构体序列化，字段顺序不变；`AiSuggestionRequest` 本身不变，宿主传入的请求形状不受影响。`response_format` 保留。
- 解析回答前先去掉包住整段正文的 Markdown 代码块（`strip_code_fence`）：去掉首尾空白后以 ```` ``` ```` 开头且以 ```` ``` ```` 结尾时，去掉围栏和第一行信息串（如 `json`），单行围栏没有信息串。其他情况原样交给严格 JSON 解析：围栏不闭合、围栏前还有说明文字的回答仍然无效。
- Linux 的 Python provider 按同样的规则改请求和解析。Android 的请求体由 client-core 生成，回答却由 `MSIMEInputService.aiCandidateTexts` 自己解析，所以 `OnlineCandidatePolicy.stripCodeFence` 按同一规则补上去围栏（用 `trim()`，`String.strip()` 在 minSdk 28 上没有）。三处的测试用例一一对应。

## Alternatives considered

- **去掉 `response_format`，只靠提示要求 JSON。** 最强的理由是彻底绕开网关的 "json" 检查，实测 `gpt-6-luna` 不带它也回合法 JSON。但支持 JSON 模式的模型就失去了服务端的格式保证，提示被用户自定义（`prompt_custom_*`）后更容易回出解析不了的正文；保留它并在 input 里点名 JSON，两边都满足。
- **把 system 提示改成 user 消息发。** 同样能让 "json" 出现在 input 里，但改变了所有服务商看到的角色结构，自定义提示原本按 system 指令写，效果不可预期；加一个字段的改动面小得多。
- **从正文里找第一个 `{` 到最后一个 `}` 截取。** 能容忍更多种包法（前后带说明文字），但也会把夹在解释里的半截 JSON 当成回答；只认整段围栏，规则和失败都可预期。

## Consequences

- **收益**：EveryAPI 这类转换协议的网关上的 GPT 系列不再 400；Claude 这类无视 JSON 模式的模型的回答可以用了。macOS、Windows、iOS、HarmonyOS 的请求和解析都走 `chat_completion_http_request` 与 `msime_client_parse_ai_response`，一起生效；Android 共用请求、单独改解析；Linux 走自己的 provider 同步修改。
- **代价与已知上限**：发给模型的 user 消息多了一个字段，用户自定义提示若逐字段描述输入，会看到这个新字段。修好请求不等于够快：同一次实测 `gpt-6-luna` 的回答要 2.5 到 40 秒不等（EveryAPI 统计 24 小时平均 17.5 秒），常常超过宿主 8 秒的 AI 超时，座位仍会落到「—」；输入法联想要选一两秒内回答的模型，这是配置问题，不在本修复范围。

与 [AI 座位占位](../../archived/feature/2026-10-11-ai-seat-placeholder.md)（已由 [AI 候选坐第一页最后一格](../feature/2026-10-16-ai-candidate-last-slot.md) 取代） 部分重叠：本篇修的是座位等不到回答的两种原因，座位规则不变。与 [AI 配置测试兼容思考模型](2026-10-13-ai-credential-reasoning-probe.md) 部分重叠：那篇只改配置测试的探测请求，本篇改实际候选请求，推理参数仍未加到候选请求上（实测 `gpt-6-luna` 默认不产生推理 token，加 `reasoning_effort: none` 不改变延迟）。与 [AI 空候选缓冲](../testing/2026-10-11-ai-empty-candidate-buffer.md) 部分重叠：解析器的容量预留规则不变，只在解析前多一步去围栏。

## Verification

- `cargo test -p msime-client-core -p msime-host-api` 全部通过；新增 `response_accepts_json_wrapped_in_a_code_fence`，`request_body_matches_windows_shape_without_credentials` 增加 `output_format` 断言。两者在去掉生产改动后失败，恢复后通过。`cargo clippy -p msime-client-core --all-targets -D warnings`、`cargo fmt --check` 通过。
- Linux `platforms/linux/tests/candidate/ai_candidate_cache.py` 新增 `test_candidate_request_names_json_outside_the_system_prompt` 和 `test_candidate_answer_in_a_code_fence_is_read`，在旧 provider 上失败，新 provider 上 26 项全部通过。
- Android：`ANDROID_SDK_ROOT=~/Library/Android/sdk bash platforms/android/check-host.sh` 通过，`OnlineCandidatePolicySmoke` 新增围栏用例；没有在模拟器上验收。
- 端到端：用 `chat_completion_body` 生成的真实请求体（默认提示、`ni hao`）打 EveryAPI，`gpt-6-luna` 和 `claude-haiku-4-5` 都返回 200，回答交给 `parse_chat_completion_response` 都解析出 3 个候选；修复前同一请求 `gpt-6-luna` 为 400，`claude-haiku-4-5` 的围栏回答解析失败。没有在装好的输入法宿主里做真机验收。
