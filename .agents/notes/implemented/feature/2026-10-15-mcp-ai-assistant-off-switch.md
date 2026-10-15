# Agent Note: MCP 可以关掉 AI 助手，但不能打开

Status: implemented

## Problem

AI 助手开着时，每次组字都在候选第 2 位留一个座位，等待模型时显示「…」（见 [AI 座位占位](2026-10-11-ai-seat-placeholder.md)）。模型慢或请求失败时，用户只看到一个迟迟不出结果的「…」或「—」。用户通过 MCP 找代理排查这类问题时，代理查得到原因却改不了：MCP 的偏好白名单（`crates/mcp-server/src/preferences.rs`）没有 AI 助手的开关，只能让用户自己去设置里关，而 MCP 的说明要求代理替不懂技术的用户把步骤做掉。

## Decision

- `get_preferences` 多一个布尔 `ai_assistant`，即 `preferences.ai_assistant.enabled`。接口地址、模型、提示和密钥照旧不出现在白名单里。
- `update_preferences` 接受 `ai_assistant`，但只能关：`false` 写入开关，其余 AI 设置原样保留，用户在设置里重新打开就能接着用；本来关着时给 `true` 被拒绝，错误说明只有用户能在设置里打开；本来开着时给 `true` 不改变任何东西。
- 工具说明写明这个开关的用途：用户嫌 AI 候选慢或碍事时关掉它。

## Alternatives considered

- **开关两个方向都给。** 最强的理由是对称简单，用户让代理「打开 AI」时不用再去设置页。但打开会把用户之后打的拼音和前文发给外部服务并按次计费，这和白名单里 `learning` 只读的理由一样：这类涉及隐私和花费的决定只能由用户做，代理只读或只能往保守方向改。
- **只读，不给写。** 风险最低，但这正是现在的问题：代理查清了「…」的原因，却只能让用户自己去关。关掉没有隐私或费用风险，配置也完整保留，没有理由不让代理做。

## Consequences

- **收益**：代理能在排查「AI 候选慢、占位置」时直接替用户关掉 AI 助手，改动在几秒内生效，与其他偏好相同。
- **代价与已知上限**：想让代理打开 AI 的用户仍需自己去设置页。白名单只有总开关，不能改模型或座位规则；如果以后要让代理换模型，需要单独决定是否暴露模型名（不暴露密钥）。

## Verification

- `crates/mcp-server/src/preferences.rs` 的 `the_ai_assistant_can_be_turned_off_but_not_on`：读取时视图里没有模型和密钥；开着时给 `true` 不改变；关掉后模型和密钥原样保留；关着时给 `true` 被拒绝，修订号和开关都不变。`cargo test -p msime-mcp-server` 全部通过（单元 58 项、stdio 11 项）。
