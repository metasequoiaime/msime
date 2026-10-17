# Agent Note: iOS AI 候选翻译按接口来源传递凭据

Status: implemented

## Problem

iOS 候选翻译复用 AI 配置时，把 Keychain 取出的 Token 写进请求配置的扁平 `token` 字段。共享 `crates/client-providers/src/ai.rs` 以 `config.tokens[origin]` 作为 OpenAI、Anthropic、Gemini、Qwen、Kimi 和自定义服务的凭据来源，扁平字段只为 DeepSeek 保留兼容回退。因此这些 provider 在 iOS 上无法构造请求，候选翻译静默返回空结果。候选翻译复用 AI 配置的总体决定见[候选翻译复用 AI 配置并放宽整句长度](../feature/2026-10-17-ai-candidate-translation.md)，来源键的跨宿主规则见[AI 辅助接口允许本机和局域网的 http 地址](../feature/2026-10-08-ai-endpoint-local-http.md)。

## Decision

`TranslationProviderClient` 在构造 AI 候选翻译请求前调用 `AIEndpointPolicy.origin(endpoint)`，把 Token 放入 `config.tokens` 的 endpoint origin 键中，并且不再发送扁平 `token` 字段。若 endpoint 不能产生合法 origin，则直接跳过请求。这样 iOS 与共享 Rust 的来源绑定契约一致，且每个接口地址只会取得绑定到自己的凭据。

## Alternatives considered

- **继续发送扁平 `token`，扩大 Rust 的兼容回退到所有 provider**：改动宿主最少，也能让旧配置继续工作，但会让一个未绑定来源的旧 Token 被任意 endpoint 使用，重新打开跨接口泄漏凭据的边界，因此不采用。
- **在 iOS 侧按 provider 写入 `tokens[provider]`**：能满足当前多数 provider，但同一 provider 的不同 endpoint 会共享 Token，与来源键契约不一致，也不能正确隔离自定义接口，因此不采用。
- **在请求发送层临时从扁平字段转换**：可以隐藏在 `exchangeAI` 内，但会让请求配置在调用共享 Host API 前仍违反契约，并且容易让其他 iOS AI 路径重复实现转换；不采用。

## Consequences

OpenAI 等非 DeepSeek provider 的 iOS AI 候选翻译现在能够构造带 `Bearer` 凭据的请求；不同端点的 Token 继续按 `scheme://host:port` 隔离。非法 endpoint 不会尝试发送无凭据请求。旧的仅有扁平 `token` 的 iOS AI 配置不会被自动迁移，用户需要由现有 AI 设置流程按 endpoint 保存凭据；共享 Rust 的安全边界保持收窄。

## Verification

- 新增 `TranslationProviderTests.testAITranslationUsesTheEndpointScopedCredentialForEveryProvider`，旧实现红灯，修复后在 iOS 27 模拟器通过，并断言译文为 `hello`、请求头为 `Bearer synthetic-token`。
- `platforms/ios/build-native.sh simulator` 成功；XcodeGen 生成工程后，针对上述测试的 `xcodebuild test` 成功。
- 提交前运行 `pnpm run verify-notes`、`cargo fmt --all -- --check`、`bash scripts/verify-local.sh --quick`。
