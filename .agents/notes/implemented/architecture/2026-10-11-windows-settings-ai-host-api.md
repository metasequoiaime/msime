# Agent Note: 原生设置窗口 AI 页用到的 C 接口

Status: implemented

## Problem

原生设置窗口接下来要把 AI 页做成原生页。共享 AI 页在 Windows 上要的宿主能力有四样：接口地址下面的说明和保存 Token 用的来源键（`ai-endpoint-policy.ts` 的 `aiEndpointHint`、`aiEndpointOrigin`），「获取模型」（Tauri 的 `ai_models`），「测试」润色（Tauri 的 `ai_test`），和「测试 AI 辅助配置」（Tauri 的 `test_api_credential`，`ai.assistant`）。窗口只能经 `msime_client.h` 的 C 接口调 Rust。

查下来，「测试配置」已经在：`msime_client_test_api_credential` 和 Tauri 调同一个 `credential::test_service`，`ai.assistant` 落到 `credential::probe::test_chat`。地址规则的权威实现在 client-core 的 `ai::endpoint`，但设置页的说明文案只在 TypeScript 里，也没有 C 导出。获取模型和润色测试整个写在 Tauri 的 `ai.rs` 里（reqwest、不跟随重定向、1 MiB 响应上限、错误码），列表地址的换算在 Tauri 的 `shared/ai_url.rs`。

## Decision

- 两个请求搬进 client-core 的新模块 `ai::settings`：`fetch_models`、`test_polish`，连同 Token 与文字的校验、响应体上限、`parse_models`、`parse_test_reply`、`test_body`。错误是 `AiSettingsError`，`code()` 给出 Tauri 一直返回的 `ai_invalid`、`ai_models_unavailable`、`ai_models_invalid`、`ai_test_unavailable`、`ai_test_invalid`。逻辑逐行照搬：同样的超时（连接 5 秒、总共 15 秒）、`endpoint::blocking_client_builder`（不跟随重定向，https 只走 https，http 直连不走代理）、同样的上限和过滤。
- 列表地址的换算 `models_url` 搬进 `ai::endpoint`，删掉 Tauri 的 `shared/ai_url.rs`，iOS 用的 `mobile_ai` 也改调它（保留查询参数，与原来一样）。
- `ai::endpoint::hint` 给出与 `aiEndpointHint` 逐字相同的两句说明，地址规则本身两边已经共用 `shared/contracts/ai-endpoint/cases.json`。
- Tauri 的 `ai_models`、`ai_test` 只剩把阻塞调用挪出异步运行时、把错误换成码；原来在 Tauri `tests.rs` 里的四个 AI 测试搬进 client-core（Tauri 的库测试在 Windows 上编不过，搬过去后这几项在 Windows 上也真能跑）。
- 新导出三个：`msime_client_ai_endpoint_check`（`{origin, problem, hint}`，不做 I/O），`msime_client_ai_models`（`{endpoint, token}` → 模型 id 数组），`msime_client_ai_test`（`{endpoint, model, prompt, token, text}` → 回复文字）。后两个的错误串就是上面的码，请求上限 256 KiB（三段 16 KiB 文字加转义），头文件写明会联网、最长约 15 秒、不要在界面线程上调用、不要记日志。「测试 AI 辅助配置」继续用已有的 `msime_client_test_api_credential`。

## Alternatives considered

- **原生窗口自己用 WinHTTP 发这两个请求** — 不用动 Rust；但不跟随重定向、http 不走代理、响应上限和解析规则就有了第二份，改一边另一边不会跟着改，Token 泄露到别的主机的那道防线也要重写。
- **在 host-api 里另写一份，Tauri 不动** — 改动小；但两个入口会慢慢分叉，这正是前几步把 `settings_dictionary`、`credential::test_service` 收进共享 crate 时要避免的。
- **地址说明留在 C++ 里** — C++ 只需要来源键；但 `translation_endpoint_issue` 已经把翻译页的说明放在 Rust 里，文案和规则放在一起才不会对不上。

配置测试的输出预算与固定错误说明见[AI 配置测试的思考预算与错误分类](../bug-fix/2026-10-13-ai-credential-reasoning-probe.md)；请求传输和凭据来源规则沿用本篇。

## Consequences

- 原生 AI 页能按共享页的规则显示地址说明、按同一个来源键存 Token，并用和 Tauri 完全相同的请求获取模型、做润色测试。
- Tauri 的两个命令行为不变，错误码不变；共享页对这些错误仍只显示它自己的兜底文案（`CommandError` 不是 `Error`），原生页照做即可。
- 以后改请求细节只改 `ai::settings` 一处。

## Verification

- `cargo test --locked -p msime-client-core --lib ai::` 通过（25 项，不联网），含新加的 `ai::settings` 与 `ai::endpoint` 用例。
- `cargo test --locked -p msime-host-api --lib -- ai_settings_exports ai_endpoint_check_export credential_test_export` 通过（3 项，只用请求前就被拒绝的输入，不联网）。
- `cargo build --release --locked -p msime-host-api`、`cargo check --locked -p msime-desktop` 通过，警告都是改动前就有的；`cargo test -p msime-desktop --lib` 在 Windows 上因 `account_helpers.rs`、`macos_input_source.rs` 里的 unix 专用代码编不过，与本次改动无关，所以 `mobile_ai` 的测试没在这台机器上跑。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`），没有启动窗口。没有对真实 AI 服务发过请求：这一步没有界面，联网路径只是从 Tauri 原样搬过来。
