# Agent Note: 连续大写输入不误触发快捷模式

Status: implemented
Archived: 2026-10-15

## Problem

#7616：用户按住 Shift 连续输入大写字母时，首个字母交给应用后，后续入口字母可能在没有组字的状态下被解释为快捷模式（例如 `MRI` 中的 `R` 打开临时日文），导致大写串被截断。

## Decision

macOS 宿主在会话返回未处理、且没有组字或候选时记录“最近一次大写字母已交还宿主”的短状态。连续的大写字母直接插入应用，不再重新打开 Shift+字母快捷模式；一旦输入非大写字符，或 Engine 已经接收字符形成组字，状态清除，单独的快捷模式入口仍按原有开关和方案能力工作。状态放在 macOS 输入控制器中，因为只有它能把未处理按键交给应用并在下一键前拦截事件；共享 Engine 的其他宿主仍保持原有入口契约。

## Alternatives considered

- **关闭 Shift+字母快捷模式总开关**：能避免冲突，但会让用户失去日期、表情和临时语言等现有入口，并改变已发布的默认行为。
- **在每个宿主记录大写按键序列**：可以只针对实体键盘处理，但各宿主的事件路径和修饰键判定不同，容易出现平台差异；共享 Engine 已经掌握快捷模式入口判定，集中处理更可靠。

## Consequences

连续 Shift+字母现在可以原样交给应用，单独按快捷模式入口仍保持原行为。Engine 会话增加一个布尔状态，重置组合时清除；它不会改变已经进入本地模式、正在组字或非大写字符的路径。

## Verification

macOS `ShortcutTest` 新增 `TestConsecutiveUppercaseDoesNotOpenShortcut`，覆盖首个 `M` 交给应用后连续 `R` 不调用 Engine、不进入快捷模式而直接插入应用。连续大写状态只接受带 `Shift` 的真实事件，避免把测试中的无修饰合成字符误判为实体键盘输入；`ShortcutTest` 在本机 CMake 构建后通过。`cargo test -p msime-engine` 全量通过；`cargo fmt --all`、`npm run verify-notes` 和 `git diff --check` 已运行。
