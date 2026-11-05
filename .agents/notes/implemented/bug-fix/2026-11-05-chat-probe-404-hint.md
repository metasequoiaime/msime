# Agent Note: 对话配置测试的 404 说清地址和模型各该填什么

Status: implemented

## Problem

#10876 报告 macOS 文本润色的「测试」不论选预设的「火山方舟 · 豆包」还是自定义服务商，都提示「服务拒绝请求，请检查接口地址和模型配置（HTTP 404）」，用户确认地址、密钥、模型名在别处能用。没有该用户的配置和密钥，无法在真实服务上复现。

能确定的有两处。其一，对话探测把 `endpoint` 原样当完整 URL 发 POST，不补路径；别的客户端（如各类 OpenAI SDK）要的是 `https://ark.cn-beijing.volces.com/api/v3` 这样的基础地址，用户照那里的填法填进来，请求打到基础路径上，服务回 404。其二，预设火山方舟的默认模型是 `doubao-seed-1.6-flash`（备选 `doubao-seed-1.6`），是 7fa9f5d094 从 Windows v0.11.1 逐字搬来的（见 [服务商目录补齐](../feature/2026-10-22-provider-catalog-expand.md)）。2026-11-05 查方舟官方模型列表（`docs.volcengine.com/docs/ark/model-list`），文本生成表里的 Model ID 全是连字符加日期后缀的形式（如 `doubao-seed-2-0-mini-260428`、`doubao-seed-2-1-lite-260915`），没有带点的写法，也没有列出 Seed 1.6 系列；方舟错误码文档（`docs.volcengine.com/docs/82379/1299023`）写明模型或接入点不存在、无权访问时回 404 `InvalidEndpointOrModel.NotFound`，账号未开通该模型时回 404 `ModelNotOpen`。两种原因落到同一句「检查接口地址和模型配置」上，用户核对了「地址能用、模型名在别处能用」，就再没有线索。

## Decision

- 对话探测（AI 辅助、语音润色）收到 HTTP 404 时，提示固定为 `CHAT_NOT_FOUND_MESSAGE`：「服务找不到所填的接口地址或模型（HTTP 404）。接口地址要填完整的对话接口，通常以 /chat/completions 结尾；模型要填服务商控制台列出的模型 ID，并确认账号已开通该模型。」这句话对预设和自定义服务商都成立：所有预设的对话地址都以 `/chat/completions` 结尾，自定义服务商最常见的 404 正是只填了基础地址。其余状态码照旧走 `rejected_status_message`。
- `rejected_status_message` 本身不改：整句识别探测 `credential::asr::test` 也用它，转写接口不是 `/chat/completions`，不能给它这句话。
- iOS 的 `CredentialStatusPolicy.chatNotFound`（只在 `kind == .ai` 时用）和 HarmonyOS 的 `CredentialStatusPolicy.CHAT_NOT_FOUND` / `chatRejected` 与 Rust 逐字相同；macOS、Windows、Linux、Android 的设置页都经 `credential::test_service` 走 Rust，自动得到新文案。
- 预设火山方舟的默认模型这次不改，留给维护者确认（见 Consequences）。

## 历史笔记审计

- [配置测试说出余额不足、服务端故障和豆包握手被拒](2026-10-25-credential-test-failure-causes.md) 部分重叠：它确立了「HTTP 被拒按状态码给一句话、不带服务端正文、iOS 与 HarmonyOS 各有一份逐字相同的副本」，本篇在同一原则下把对话探测的 404 从「其余状态」里分出来。
- [服务商目录补齐](../feature/2026-10-22-provider-catalog-expand.md) 部分重叠：火山方舟的默认模型来自那次逐字搬运，本篇记下它与方舟现行 Model ID 不符。

## Alternatives considered

- 地址不以 `/chat/completions` 结尾时自动补上：能直接救活只填基础地址的用户；但有些兼容服务的对话路径不叫这个名字（如 Anthropic 的 `/v1/messages`），自动补会把本来对的地址改错，而且探测和键盘的实际请求得同步改，影响面超出一次文案修复。
- 把预设火山方舟的默认模型换成官方列表里的 `doubao-seed-2-0-mini-260428`（LTS）：能让预设开箱即用；但 Seed 2.0 是否默认开启深度思考、润色的输出预算够不够、要不要像 DeepSeek 一样发 `thinking.type = disabled`，都没有密钥在真实服务上验证过，而且要同步 Rust、TS、C++、Swift、Python 共十余处目录和断言。选哪个模型是产品决定，先交给维护者。
- 把服务端的错误文字（方舟的 `InvalidEndpointOrModel.NotFound` 之类）转给设置页：最能说清是地址还是模型；但违背「不透传服务端正文」的既有约定。

## Consequences

用户看到 404 时知道要把地址填到 `/chat/completions`、模型填控制台里的 Model ID，自定义服务商只填基础地址的情况能自行改对。预设火山方舟仍带着不在官方列表里的默认模型 `doubao-seed-1.6-flash`，选它的用户测试仍会 404，只是提示会指向模型 ID；换成哪一个现行 Model ID 需要维护者决定并在真实服务上验证。文案多了一句，iOS 和 HarmonyOS 的副本要与 Rust 同步。

## Verification

- `cargo test -p msime-client-providers --lib credential::probe`：先改断言，旧实现在 404 处失败（得到「服务拒绝请求，请检查接口地址和模型配置（HTTP 404）」）；改后 `cargo test -p msime-client-providers` 全部通过。
- `cargo clippy -p msime-client-providers --all-targets -- -D warnings` 通过。
- `bash platforms/harmony/tests/run.sh` 通过，`credential-status.test.ts` 新增 404 一组。
- iOS：`CredentialStatusPolicy` 抽出单独用 `swiftc` 编译，核对 `chatNotFound` 与 Rust 逐字相同；`MSIMEServiceTests` 的用例改为期望 `chatNotFound` 并补 422，完整的 xcodebuild 测试交给 CI。
- 没有方舟密钥，真实服务对基础地址、带点模型名的响应没有验证。
