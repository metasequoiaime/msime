# Agent Note: macOS 从别的输入源切回时报告「中」不改双拼

Status: implemented

## Problem

#10199：macOS 0.55.0 上默认用小鹤双拼的用户，从系统英文输入法切回水杉后有时变成全拼，并且之后一直是全拼。

维护者在 issue 里按代码给出的是「最可能的原因」，并向报告者问了三个确认问题（怎么切回、装了哪几个入口、变全拼后图标是「中」还是「双」），报告者还没有回复，也没有设备复现。下面的原因链是这个假设，前提「系统设置里『中』『双』两个入口都加了」待报告者确认。

原因链在 `platforms/macos/src/input/` 里。切到 ABC 等别的输入源时 `input_method_main.mm` 的 `switchedAway` 回调调用 `MSIMEResetSystemInputModeState`，共享模式记录 `current` 变成 nil；切回来后的第一次 `setValue:forTag:client:` 报告因此一定被 `MSIMEAdoptReportedInputMode` 采纳。两个入口都加了时，macOS 切回时可能落在「中」上。`MSIMESchemeForReportedInputMode` 原来的规则是：当前中文方案有自己的、系统可用的模式（双、五、粤、注、笔），而报告的是「中」，说明用户越过那个模式选了「中」，改用本版本默认方案（full 是全拼）。`systemDidReportInputMode:client:` 随后把全拼写进偏好。切回时的「中」并不是用户在「中」和「双」之间挑过，这条规则在这里误判了意图。

## Decision

从别的输入源切回水杉、系统报告「中」、而「双」也已安装时，保留当前的双拼方案，并把菜单栏重新选回「双」。只有切回之后已经在水杉里按过键或做过选择，再从「双」切到「中」，才像现在一样改用全拼。issue 里维护者已表态会调整切回时的处理；按仓库规则 PR 照样写 Fixes，合并后要请报告者用两个入口的配置复验。

实现上，`MSIMESchemeForReportedInputMode` 多一个显式参数 `switchedIn`。它为 YES 时，「中」不再触发「越过自有模式即改用默认方案」这条规则，其余规则不变：从日、韩、越、藏回来仍回到进入前的中文方案，报告的「双」「五」「日」等仍选各自的方案。

`switchedIn` 是共享记录 `MSIMESystemInputModeState` 里单独的字段，不从 `current == nil` 推断。初值为真，`MSIMEResetSystemInputModeState`（离开水杉时调用）把它恢复为真；`systemDidReportInputMode:client:` 在采纳前读它、采纳后清掉它；`handleEvent:client:` 收到第一次按下的键时也清掉它。激活后的校准 `MSIMERecordRestoredSystemInputMode` 和控制器自己的 `MSIMESelectSystemInputMode` 都不碰它。

必须单独记，是因为激活后的校准和报告谁先到都会发生（2026-10-21 那篇把校准延后了一轮主队列）。校准先到时，它把系统落在的「中」写进 `current`，紧接着的 `syncSystemInputModeForClient:` 按双拼算出「双」，经 `MSIMESelectSystemInputMode` 把 `current` 改成「双」；随后到达的「中」报告和记录不同，会被采纳。若只看 `current == nil`，这次报告已经不算切回，照旧改成全拼。用单独的标记，这次报告仍按切回处理，方案不动，同步再选回「双」。

方案不动之后，报告处理末尾原有的 `syncSystemInputModeForClient:` 按双拼算出「双」，经 `MSIMESelectSystemInputMode` 请求系统选中它，菜单栏回到「双」，不需要新的同步路径。

进程启动也算切进：Sparkle 在切走时装更新、进程退出，切回时系统从新 bundle 拉起，这正是切回场景；登录后第一次用、崩溃重启也一样处理。

按键只看 `NSEventTypeKeyDown`：切换输入源用的 Caps Lock 和 Ctrl 松开是 flagsChanged，可能先于切回时的报告送到，算进去会提前清掉标记。

## Historical audit

- [macOS 恢复输入源时避免重复选择模式](2026-10-21-macos-input-source-restore.md)：部分重叠。那篇决定激活时延后一轮主队列、按 TIS 实际输入源校准记录；本篇因为它带来的两种到达顺序，才把「刚切进」单独记成标记，不改变它的时序。
- [macOS 菜单栏模式状态项](../feature/2026-10-12-macos-input-mode-status-item.md)：无关。状态项显示控制器算出的模式，本篇不改。

## Alternatives considered

- 切回时一律忽略「中」报告（不采纳）：最简单，也能保住双拼；但「中」报告同时决定中英文状态，用户从 ABC 切回并在「英」和「中」之间选择时，忽略它会让控制器停在英文。只放宽方案规则，中英文状态照常跟随报告。
- 用 `current == nil` 判断切回（本 PR 第一版）：不用新字段；但激活校准先到时校准和随后选回「双」都已写过 `current`，滞后的「中」报告会被当成水杉里的选择，#10199 在这种时序下照样出现。
- 把判断放进 `MSIMEAdoptReportedInputMode`，让它返回或清掉「是否切回后的第一次报告」：少一处控制器代码；但它的契约是「是否采纳」，混进第二个含义会让控制器和测试里的每个调用方都要理解它。控制器在采纳前读标记、采纳后清掉，`ShortcutTest` 用控制器级用例守住这个顺序。
- 维持原规则，只在 issue 里建议用户删掉「双」入口：不改代码；但两个入口都加是合理的配置，只要加了就会反复出现。

## Testing

`platforms/macos/tests/input/InputModeIdentifiersTest.mm`：`switchedIn` 为 YES 时，「中」在双拼、五笔、全拼上都不改方案，从日文回来仍回到进入前的双拼，「双」「日」仍选各自方案；新记录和清空后的记录都标记为刚切进；按控制器的顺序（采纳前读标记、采纳后清掉）覆盖两种到达顺序：报告先到时「中 → 方案不动 → 选回双 → 再报告中 → 改全拼」，校准先到时「校准记中 → 选回双 → 滞后的中 → 方案不动 → 再报告中 → 改全拼」；校准记下的「中」被重复报告时不采纳、标记保留。原有 `switchedIn` 为 NO 的断言保持不变。

`platforms/macos/tests/input/ShortcutTest.mm` 的 `TestSwitchBackKeepsShuangpin` 在真实控制器上跑同样的两种顺序，并覆盖「按过键后报告中改全拼」。控制器多了 `inputModeAvailability` 入口，测试子类换成合成的可用性，所以 CI 上「双」没有启用也能区分修复前后。

## Consequences

收益：从 ABC 等输入源切回不会再把双拼、五笔、粤拼、注音、笔画用户改成全拼，偏好不再被意外改写，菜单栏回到方案自己的模式。

代价：两个入口都加了的用户，想在切回水杉时直接从 ABC 选「中」改用全拼，需要切回后在水杉里再选一次「中」，或从方案菜单换。进程刚启动、还没按键时在输入菜单里明确选「中」，这一次同样不改方案。

已知没覆盖的时序：离开回调在通知送达时才读当前输入源，用户很快地切走又切回（连按两次 Caps Lock 或 Ctrl+Space）、或者通知送达得晚，读到的已经是水杉，就不会清空记录、也不会标记刚切进，这时切回报告的「中」仍按水杉里的选择处理，和修复前一样。要覆盖它需要一个不依赖通知时序的离开信号，IMK 的 deactivateServer 在客户端之间换焦点时也会调用，分不出这两种情况。

没有真机验证：系统在切回时报告「中」、随后控制器选回「双」的实际顺序，以及报告者的配置是否真是两个入口都加了，都只能在装机后用两个入口的配置验收。
