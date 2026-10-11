# Agent Note: 鸿蒙原生代码按码点拆字符串

Status: implemented

## Problem

鸿蒙设置 App 的 AI 对话，用 MiniMax-M3 发一句话，后端回了 HTTP 200 和正常的回复，页面却显示「聊天服务暂不可用，请稍后重试」。在账号桥里加诊断日志复现后看到：回复正文带两个 emoji（👋😊），被 `validChatText` 当成控制字符拒掉。

根因在运行时：鸿蒙的 ArkTS 运行时上 `Array.from(text)` 和 `[...text]` 都按 UTF-16 码元拆字符串，「a😊b」拆出四段，emoji 变成两个落单的代理项；只有 `for...of` 拆出三段（HarmonyOS 6.0 模拟器实测）。逐段校验时落单的代理项通不过 `TextPolicy.validUnicode`，于是：

- AI 对话的回复只要带 emoji 就整条被拒；
- `TextPolicy.validMultiline`、社区回复模板、本地语音等同样逐字符校验的地方，遇到 emoji 也会拒；
- 按字符计数的地方把一个 emoji 算成两个（反馈字数上限、候选管理里「是不是单个字」、候选宽度估算等）。

Node 上三种写法都按码点拆，`platforms/harmony/tests` 在 Node 上跑，所以单元测试一直没测出来。

## Decision

- `TextPolicy.characters(value)` 用 `for...of` 按码点拆。鸿蒙原生代码（`platforms/harmony/entry/src/main/ets`）里对字符串的 39 处 `Array.from` 和整串展开全部换成它。
- 新契约检查 `scripts/test-harmony-string-code-points.py`：这些源码里不许出现 `Array.from(` 和整串展开 `[...x]`（注释行除外），数组要复制用 `.slice()`。在原来的代码上它报出 40 处。
- 测试：对话回复样例换成 MiniMax-M3 的形状（带 emoji 和 `reasoning_content`）；新增 `TextPolicy.characters` 的用例。它们在 Node 上原本也会通过，真正拦住回潮的是契约检查。

## Alternatives considered

- **只改 `validChatText`。** 改动最小，能立刻修好对话。不用它，是因为同样的写法还有三十多处，反馈、提示词、回复模板、候选管理都会在 emoji 上出错，只修一处会留下同一个根因。
- **改成按码元扫描控制字符。** 对校验类函数也对（控制字符都在基本平面），但计数和按字符处理的地方仍然需要正确的拆分，统一用 `characters` 更简单。

## Consequences

- **收益**：带 emoji 的 AI 回复能正常显示；反馈、提示词、回复模板接受 emoji；按字符计数和候选处理把 emoji 当一个字。
- **代价**：以后在鸿蒙原生代码里按字符处理字符串必须记得用 `TextPolicy.characters`，契约检查会提醒。`apps/harmony` 和 `packages/ui` 跑在 ArkWeb（Chromium）里，那里 `Array.from` 是对的，不受这条约束。
