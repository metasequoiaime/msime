# Agent Note: AI 候选翻译复用配置校验

Status: implemented

## Problem

候选翻译复用 AI 配置后，`chat_completion_http_request` 会拒绝未知 `provider`、空 `model` 和超长模型名，但 `chat_translation_http_request` 没有同一层校验。用户把 AI 服务切到自定义并清空模型，或通过旧配置留下未知 provider 时，翻译描述器仍会带着凭据和无效模型发出请求，服务只会返回错误。

## Decision

AI 翻译描述器在构造凭据请求前复用 `AI_PROVIDERS` 白名单，并要求 `model` 非空且不超过 256 个 Unicode 字节边界文本上限，与普通 AI 请求体保持一致；不合规配置返回 `InvalidConfiguration`，不构造请求。

## Alternatives considered

- **只依赖设置页禁用保存**：设置页能挡住常见输入，但旧偏好、手工编辑文档和 FFI 调用仍可绕过，且普通 AI 路径已有共享层防线；因此不采用。
- **让翻译服务照常发送并交给服务端报错**：会泄露本应被本地拒绝的候选文本和凭据使用尝试，错误还因服务商不同而不一致；因此不采用。
- **把翻译和普通 AI 共用一个完整请求体构造函数**：两者提示词和输入结构不同，强行合并会扩大改动面；本次只同步配置边界。

## Consequences

无效 AI 配置在候选翻译路径和普通 AI 路径得到相同的本地拒绝结果，不再产生无效网络请求。合法自定义模型名和已有服务商行为不变。

## Verification

新增 `translation_descriptor_rejects_invalid_provider_and_model` 回归测试，先在旧实现上失败，再在修复后通过；`cargo test -p msime-client-providers --locked translation_descriptor_rejects_invalid_provider_and_model` 通过。
