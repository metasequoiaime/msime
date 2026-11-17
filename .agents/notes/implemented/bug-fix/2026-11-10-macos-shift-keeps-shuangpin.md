# Agent Note: macOS 英文按 Shift 回中文时保留双拼

Status: implemented

## Problem

#11245：macOS 用户使用小鹤双拼时处于英文输入状态，按 Shift 切回中文后方案偶尔变成全拼。此时控制器已经按当前方案请求系统显示「双」，但 macOS 还会迟到报告「中」。`systemDidReportInputMode:client:` 把这份回执当成用户在水杉里从「双」选了「中」，按现有规则把方案改成全拼。

## Decision

英文状态切回中文后，在下一份被采纳的系统模式报告中，若报告的是「中」，把它视为这次中英切换的回执，保留原中文方案。报告处理完成后清除一次性标记；后续再报告「中」仍按用户主动选择处理并切到全拼。第一次按键和失焦也清除未消费的标记，避免一份没有到达的回执影响下一次独立选择。

标记保存在 `MSIMEInputController`，只由中英状态从英文变成中文的本地切换设置。`systemDidReportInputMode:client:` 在采纳报告前读取它，在处理完报告后清掉；控制器请求模式时同步回来的、仍处于 `selecting` 保护期内的回报不会提前清掉它；报告本身触发的 `setEnglishInputMode:` 也不会把标记带到下一次报告。切回输入法的 `switchedIn` 规则保持不变，系统输入菜单直接报告「中」时也保持原有“选中「中」代表全拼”的行为。

## Historical audit

- [macOS 从别的输入源切回时报告「中」不改双拼](2026-10-29-macos-keep-shuangpin-on-return.md)：部分重叠。那篇处理输入源切回后的 `switchedIn` 报告，本篇只处理已在水杉内从英文切回中文的迟到回执，两条标记独立存在。
- [macOS 切进水杉时的模式报告不把默认英文改回中文](2026-11-10-macos-switch-in-keeps-starting-english.md)：部分重叠。那篇处理切入输入法时的中英起点，本篇处理同一输入法内的中英切换，保留其起点规则。

## Alternatives considered

- 让所有「中」报告都保留当前方案：实现简单，但会破坏用户在输入菜单里从「双」明确选择「中」切换到全拼的既有行为。
- 只把 `MSIMESelectSystemInputMode` 的同步请求长期标记为“等待回执”：能覆盖系统迟到报告，但没有报告时会阻塞后续真实的输入菜单选择，且无法区分输入源切回和同一输入法内的中英切换。
- 仅修改 `MSIMESchemeForReportedInputMode`：该函数只知道报告模式和方案，不知道这份报告是否由本地中英切换触发，无法安全区分回执与用户选择。

## Consequences

英文用户从 Shift、Caps Lock 或快捷键切回中文时，双拼方案不会被滞后的「中」报告改成全拼；用户随后在输入菜单里明确选「中」仍能切到全拼。一次性标记依赖系统最终送达至少一份模式报告；若系统完全不回报，第一次后续按键会清掉它并恢复正常选择语义。

## Testing

`TestSwitchBackKeepsShuangpin` 新增英文到中文的控制器回归：中英切换后的第一份「中」报告保留双拼，第二份「中」报告改为全拼；原有输入源切回、激活校准和按键后的路径保持覆盖。代码通过 `git diff --check`；完整 macOS 构建与 CTest 需要在有可用磁盘的 macOS 构建环境执行。
