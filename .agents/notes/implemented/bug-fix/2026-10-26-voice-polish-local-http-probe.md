# Agent Note: 语音润色配置测试复用 AI 的本机 HTTP 端点规则

Status: implemented

## Problem

macOS 语音润色的自定义服务填入本机 HTTP 地址并提供 Token 后，「测试配置」在联网前返回「请填写有效的 HTTPS 接口地址和模型名」。`credential::probe::test_chat` 对 `ai.assistant` 使用共享 AI 端点策略，对 `voice.polish` 却调用 HTTPS 专用校验；同一函数使用的实际传输 `ChatProbeTransport` 已支持本机和局域网 HTTP。macOS `HTTPVoiceRequest.mm` 的实际润色路径也已允许回环 HTTP，因此测试与实际使用不一致。

## Decision

两个对话测试入口统一调用 `ai::endpoint::is_allowed`，复用既有模型名校验和 HTTP 传输。完整 URL 原样作为请求地址；不自动拼接 `/v1/chat/completions`。凭据要求、模型名、默认值、识别服务与翻译服务的端点规则保持现状。本次处理截图中「测试配置」错误拦截本机 HTTP 的问题，不声称其他原生宿主的实际润色传输都已支持局域网 HTTP。

## Existing notes audit

- [AI 本机与局域网 HTTP 策略](../../implemented/feature/2026-10-08-ai-endpoint-local-http.md)部分重叠，持有地址规则与直连策略，本篇复用而不另定一套规则。
- [各宿主端点策略经 C ABI 查询](../../implemented/architecture/2026-10-17-ai-endpoint-check-via-c-abi.md)部分重叠，继续持有原生宿主边界，本篇只改变共享配置测试。
- [配置测试失败原因](../../implemented/bug-fix/2026-10-25-credential-test-failure-causes.md)部分重叠，HTTP 结果提示和凭据测试返回结构继续沿用。

## Alternatives considered

- 只给润色测试额外放行 `127.0.0.1`：能解决截图中的地址且改动小，但会再维护一套地址例外，与同一传输已经使用的共享策略分叉，所以直接复用 AI 策略。
- 将语音识别的 HTTPS 校验 helper 整体改成接受 HTTP：可以减少调用差异，但会连带扩大识别服务的规则，超出本次润色测试缺陷范围。

## Verification

旧生产实现运行新增回归：`credential_chat_endpoint_policy_matches_the_shared_contract` 在 `voice.polish` 的第一个本机地址失败，`credential_chat_budget_allows_the_reasoning_server_to_finish` 在真实回环 HTTP 的润色入口失败；两者均返回截图中的 HTTPS 提示，AI 控制路径通过。

端点契约回归复用无需首尾空白清理的共享用例，覆盖 HTTPS、本机与局域网 HTTP、公网 HTTP 和无效 URL；测试入口原有的 `trim` 语义保持。真实 HTTP 用例复用原有合成服务，并检查 POST 路径、合成 Bearer Token 与原模型名，通过公开 `credential::test_service` 调用两个服务入口。C ABI 的离线错误断言同步为共用说明。修复后在 macOS 上执行：

- `cargo test --locked -p msime-client-providers --lib credential`：45 项通过。
- `cargo test --locked -p msime-client-providers --lib ai::endpoint`：4 项通过。
- `cargo test --locked -p msime-host-api --lib credential_test_export_answers_incomplete_configurations_offline`：1 项通过。
- `cargo clippy --locked -p msime-client-providers --all-targets -- -D warnings` 通过；Rust 格式检查通过。
- 按原生宿主使用的静态链接方式构建 C ABI 探针，调用 `msime_client_test_api_credential`。合成本机 HTTP 服务分别在根路径与 `/v1/chat/completions` 接收 AI、润色测试请求，四次均成功；合成命名空间模型与 Bearer Token 保持。公网 HTTP 在联网前被拒绝，识别服务仍拒绝本机 HTTP。
- 构建 macOS 原生 `voice-polish-request-test`、`http-voice-request-test`，对应两项 ctest 通过，验证实际润色返回、失败回退与 HTTP 识别请求。没有覆盖 GUI 按钮点击，也没有安装或替换用户正在运行的应用。

`bash scripts/verify-local.sh --quick` 已使用 Python 3.15 执行，Rust 工作区（因未暂存原生 bundle 排除 `msime-desktop`）、189 项本机可运行的 Windows 原生测试、共享 Apple 桥和笔记校验通过。整条门禁未通过：本机仅安装 Command Line Tools，iOS 豆包传输测试找不到 XCTest，macOS 完整应用构建找不到 SwiftUIMacros 插件；相应平台源码相对本次基线无改动。Android 更新任务取消检查最初因系统 Java 桩未找到 JDK 失败，找到本机已有的 JDK 17 并加入 `PATH` 后，该检查独立重跑通过。缺少 Android SDK/NDK、MinGW、Docker 和鸿蒙打包工具的阶段按脚本跳过。这些环境缺口不计为通过，也没有为本次 PR 修改门禁或失败基线。

## Consequences

收益是润色配置测试不再错误拒绝本机 HTTP，端点策略与实际探测传输共用同一实现。代价是两个对话入口的错误提示同步为允许本机与局域网 HTTP 的说明。

配置测试仍要求 Token，免鉴权本地服务不在本次范围。用户只填基地址时不会自动补齐接口路径。此次没有修改其他原生宿主的实际语音润色路径，测试放行局域网地址不等于已在所有设备完成验收。
