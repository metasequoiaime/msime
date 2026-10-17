# Agent Note: iOS 的 AI 皮肤提示词与答案解析改用 Rust

Status: implemented

## Problem

iOS App 的 AI 皮肤（`App/Services/AISkinService.swift`）自己带一份系统提示词，自己解析模型返回的三套设计：配色、圆角、背景场景的范围，对比度修正，三套是否重复，造型、材质和背景场景是否各不相同。Android、HarmonyOS 和 Tauri 用的是 client-services 的 `AI_SKIN_SYSTEM_PROMPT` 和 `plan_ai_skins`，经 `msime_client_ai_skin_plan` 给要自己发 HTTP 的宿主（头文件写明「提示词和解析器不能分开：提示词点名的就是解析器认的文档」）。两份规则已经分叉：Swift 的解析接受缺键帽造型、材质或背景场景的回答，多出的字段照收，名称里的控制字符也不拒；提示词也不是同一段话。

## Decision

- `AISkinService.messages(prompt:model:)` 调 `msime_client_ai_skin_plan` 的 `compose`，用它给的 `messages`（系统提示词加用户描述）；`path` 必须是 `/v1/chat/completions`。HTTP 暂时仍由 Swift 的 `BackendAccountClient.chat` 发，请求体（`messages`、`model`、`max_tokens: 2048`、`stream: false`）与 `compose` 给的相同。到 4c 再整个换成 `msime_client_ai_skin_generate`。
- `AISkinService.parse(_:)` 调 `parse`，把 Rust 给的三套方案（`{name, description, artworkPrompt, design}`，`design` 与 Swift 的 `CustomKeyboardSkin` 同一份 JSON 形状）解成 `AISkinProposal`。不合格时整批拒绝，提示统一为「AI 返回的皮肤方案不可用，请重新生成。」。
- `generate` 里原来在解析之后另查的「三套造型、材质、背景场景各不相同」删掉，`plan_ai_skins` 已经一并查过。
- 用户的描述先经 `AISkinService.requestPrompt`：`compose` 拒收一切控制字符、按 Unicode 标量限 500 个，而描述来自多行的 `TextEditor`，所以内部的换行和制表符连同两侧空白折成一个空格；折完仍超过 500 个 Unicode 标量或含其他控制字符时，在联网之前提示「描述太长或含有无法识别的字符，请删减后再试。」，不再先白发一次模型目录请求、再报一句误导的「无法生成抽卡灵感」。
- 抽卡灵感（`drawPrompt`）和插画的格式检查（`illustrate` 里的 mime、尺寸、8 MiB）不变，不在这一步。

## Alternatives considered

- **只把 Swift 的提示词改成与 Rust 逐字相同** — 改动最小；但解析规则仍是两份，下次改提示词又要两边一起改，而头文件把两者绑在一起正是为了不再分叉。
- **现在就换成 `msime_client_ai_skin_generate`** — 一步到位，HTTP、轮询和取消都归 Rust；但它要注入会话存储（4a、4b），而 iOS 的账号会话那时还在 Swift，这一步先换掉不依赖会话的那一半。

## Consequences

- **收益**：iOS 与 Android、HarmonyOS、Tauri 用同一段提示词、同一个解析器，修正对比度的规则只有一份。
- **代价与已知上限**：Rust 的提示词比原来 Swift 那段短，没有写出圆角、边框、阴影的数值范围和 4.5:1 的对比度要求，超出范围的回答照旧整批拒绝；模型回答里多一个字段、缺背景场景或名称带控制字符时，原来解析能过（之后还可能在 `generate` 里被拒），现在一步就拒。几种失败原来各有说法（无效配色、参数越界、背景场景无效、造型不够不同），现在是同一句。描述里的换行原来原样发给模型，现在折成空格；emoji 多、字形数不到 500 而 Unicode 标量超过 500 的描述原来能生成，现在要删减。

## Verification

`KeyboardTests/candidate/AISkinServiceTests.swift`：三套合格方案的对比度修正和保存往返、提示词来自 Rust、早先录下的没有键帽造型的云端回答被拒、多出字段与控制字符被拒、背景场景 40–100 个 Unicode 标量。`testMultilinePromptIsFoldedBeforeRust`：换行和制表符折成空格后 Rust 接受，原样的换行 Rust 拒绝，字形数不到 500 而标量超过 500 的描述在本地就拒绝。
