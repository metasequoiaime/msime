# Agent Note: macOS 切进水杉时的模式报告不把默认英文改回中文

Status: implemented

## Problem

用户报告：macOS 15、0.59.1、五笔，在设置的「中英模式」里把默认模式设成英文，新焦点仍然是中文，卸载、清理、重装都没用。

2026-11-10 在维护者的 0.59.1 上复现（macOS 27，五笔，默认英文，按应用范围）。不切输入源时，新打开的 TextEdit 文稿敲 `gg ` 得到字面的 `gg `，是英文；用 TIS 切到 ABC 再切回「水杉五笔」后，同样的新文稿得到「五」，是中文。诊断日志只在切回后记了一行 `input_mode_report mode=Wubi action=adopted scheme=wubi target=wubi switched_in=1`。切回之后再打开另一个没用过的应用（脚本编辑器）是英文。

原因链在 `platforms/macos/src/input/InputController.mm` 的 `systemDidReportInputMode:client:`。切到别的输入源时输入源监视器清掉记住的模式，`MSIMEResetSystemInputModeState` 清掉共享模式记录，`switchedIn` 置为真。切回时 `activateServer:` 先按「应用规则 → 记忆 → `default_ime_mode`」算出英文，随后系统报告它落在的入口（只装了主模式的用户就是方案自己的「五」），这次报告因为 `current` 为空一定被采纳，末尾的 `setEnglishInputMode:(mode == English)` 把英文改成中文。`setEnglishMode:` 还会把它记进当前应用的记忆、并顶掉这个应用的英文规则。按应用范围下，每次切进时所在的那个应用变成中文；全局范围下记忆是全局的，此后每个新焦点都是中文，设置看起来完全无效。进程启动也算切进，所以重装、更新、重启之后同样如此。用户常用 Ctrl+Space 或 Caps Lock 在 ABC 和水杉之间切换时，几乎每次进来都是中文。

## Decision

切进水杉后的第一次报告（`switchedIn` 为真）不把英文改回中文。控制器在激活时算出的起点是英文（应用规则是英文，或者没有规则、默认模式是英文）时，报告「五」「双」「中」等非英文入口，中英状态保持英文，方案照旧跟随报告，末尾的同步把菜单栏选回「英」（没有加「英」入口时菜单栏停在方案的模式，菜单栏状态项显示「英」）。报告的是「英」时照样进英文，那是用户在输入菜单里明确选的。起点是中文时行为不变。

实现是一行：`setEnglishInputMode:mode == MSIMEInputMode::English || (switchedIn && english)`，`english` 是报告处理前控制器的中英状态，`switchedIn` 是采纳前读到的共享标记，两者在函数里原本就有。切进之后（按过键或采纳过一次报告），用户在输入菜单里选的「中」照常把英文改成中文。

这和 Windows 一致：TIP 激活时先查应用规则，查不到用 `default_ime_mode` 作起点（见 Windows 对齐那篇），切进输入法就是按设置重新起步，而不是按输入菜单停在哪个入口。

## Historical audit

- [macOS 从别的输入源切回时报告「中」不改双拼](2026-10-29-macos-keep-shuangpin-on-return.md)：本篇翻转它的一部分。那篇的备选方案里定下「只放宽方案规则，中英文状态照常跟随报告」，理由是用户从 ABC 切回时可能在「英」和「中」之间做选择。本篇在起点是英文时改为不跟随非英文的报告；起点是中文、以及报告「英」的情况仍按那篇的规则。`switchedIn` 标记的定义、置位和清除时机都沿用那篇，不改。
- [Windows 中英文切换体验对齐 macOS](../feature/2026-10-10-windows-input-mode-parity.md)：部分重叠。那篇定下 macOS「规则 > 记忆 > 默认」的顺序和 Windows TIP 激活时的起点；本篇让 macOS 切进时的系统报告不再推翻这个起点。
- [macOS 恢复输入源时避免重复选择模式](2026-10-21-macos-input-source-restore.md)：无关。唤醒和解锁时系统恢复输入源的报告走 `restoring` 分支，只记录、不改中英状态，本篇不碰它。

## Alternatives considered

- 切进时的报告一律不改中英状态，报告「英」也不采纳：规则最对称，切进时完全按设置起步；但「英」只可能是用户在输入菜单里选的（主模式之外的入口要用户自己加），默认中文的用户从 ABC 明确选「水杉英文」却落在中文，是新引入的错误。
- 切进时把报告和起点比较，只要不同就按起点重新选回菜单栏：和本篇的效果接近，但要在报告处理里另开一条「拒绝采纳」的路径，`current` 已经被 `MSIMEAdoptReportedInputMode` 写成报告的模式，还要回滚它；本篇只改传给 `setEnglishInputMode:` 的值，末尾原有的同步自然把菜单栏选回「英」。
- 在 `activateServer:` 之后延迟一轮再按起点覆盖一次中英状态：不碰报告处理；但报告和激活校准谁先到都会发生（见 2026-10-29 那篇），延迟覆盖会把切进之后用户马上在菜单里选的「中」也一起覆盖掉。
- 只在设置页提示「默认英文要配合在输入菜单里加『英』入口」：不改代码；但加了「英」入口也没用，系统切回时落在哪个入口由系统决定，落在「五」上照样被改成中文。

## Testing

`platforms/macos/tests/input/ShortcutTest.mm` 的 `TestSwitchInReportKeepsStartingEnglish`，在 `AvailableModesController`（所有模式都可选，CI 上结果确定）上跑：按应用和全局两种范围、默认英文、五笔，切进后报告「五」，中英状态仍是英文、方案仍是五笔、菜单栏被选回「英」、`switchedIn` 已清掉，下一个应用也从英文开始；切进处理过之后报告「中」改成中文。默认中文加这个应用的英文规则时，切进的「五」报告不顶掉规则。起点是中文时切进报告「五」仍是中文、菜单栏不动；切进时报告「英」进英文。修复前这个用例在第一处断言失败（报告「五」后 `englishMode` 为 NO），修复后整个 `shortcut-test` 和 CMake 注册的 12 个 `shortcut-*` 变体都通过。

## Consequences

收益：设了默认英文（或给应用设了英文规则）的用户，从 ABC 切回、重装、更新或重启输入法之后，进来就是英文；全局范围下不再从此一直是中文。

代价：默认英文的用户在 ABC 里从输入菜单明确选「水杉五笔」或「中」想直接打中文，这一次进来是英文，要按一次 Shift 或在菜单里再选一次。系统切回时落在的入口和用户明确选的入口在报告里分不出来，本篇按设置优先。

没有在报告者的 macOS 15 上验收：维护者的机器是 macOS 27，复现走的是同一段代码路径，修复后的构建没有装机做端到端验证，靠控制器级单测覆盖。合并后要请报告者在 0.59.x 之后的版本上复验，尤其是全局范围。
