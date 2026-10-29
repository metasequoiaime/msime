# Agent Note: macOS 从别的输入源切回时报告「中」不改双拼

Status: implemented

## Problem

#10199：macOS 0.55.0 上默认用小鹤双拼的用户，从系统英文输入法切回水杉后有时变成全拼，并且之后一直是全拼。

原因链在 `platforms/macos/src/input/` 里。切到 ABC 等别的输入源时 `input_method_main.mm` 的 `switchedAway` 回调调用 `MSIMEResetSystemInputModeState`，共享模式记录 `current` 变成 nil；切回来后的第一次 `setValue:forTag:client:` 报告因此一定被 `MSIMEAdoptReportedInputMode` 采纳。系统设置里「中」「双」两个入口都加了时，macOS 切回时可能落在「中」上。`MSIMESchemeForReportedInputMode` 原来的规则是：当前中文方案有自己的、系统可用的模式（双、五、粤、注、笔），而报告的是「中」，说明用户越过那个模式选了「中」，改用本版本默认方案（full 是全拼）。`systemDidReportInputMode:client:` 随后把全拼写进偏好。切回时的「中」并不是用户在「中」和「双」之间挑过，这条规则在这里误判了意图。

## Decision

维护者决定：从别的输入源切回水杉、系统报告「中」、而「双」也已安装时，保留当前的双拼方案，并把菜单栏重新选回「双」。只有水杉已经是当前输入源时从「双」切到「中」，才像现在一样改用全拼。

实现上，`MSIMESchemeForReportedInputMode` 多一个显式参数 `switchedIn`。它为 YES 时，「中」不再触发「越过自有模式即改用默认方案」这条规则，其余规则不变：从日、韩、越、藏回来仍回到进入前的中文方案，报告的「双」「五」「日」等仍选各自的方案。`systemDidReportInputMode:client:` 在调用 `MSIMEAdoptReportedInputMode` 之前读取共享记录，`current == nil` 即 `switchedIn`。记录为空只有两种来源：进程刚启动，或者刚离开过水杉；激活后的校准 `MSIMERecordRestoredSystemInputMode` 和控制器自己的 `MSIMESelectSystemInputMode` 都会写入记录，所以已经在用水杉时的模式切换不会被当成切回。

方案不动之后，报告处理末尾原有的 `syncSystemInputModeForClient:` 按双拼算出「双」，经 `MSIMESelectSystemInputMode` 请求系统选中它，菜单栏回到「双」，不需要新的同步路径。激活校准先于报告到达、已经把记录写成「中」时，随后的「中」报告是重复报告，本来就不会被采纳，校准后的同步同样选回「双」。

## Historical audit

- [macOS 恢复输入源时避免重复选择模式](2026-10-21-macos-input-source-restore.md)：部分重叠。那篇决定激活时延后一轮主队列、按 TIS 实际输入源校准记录；本篇依赖它说明记录为空的含义，不改变它的时序。
- [macOS 菜单栏模式状态项](../feature/2026-10-12-macos-input-mode-status-item.md)：无关。状态项显示控制器算出的模式，本篇不改。

## Alternatives considered

- 切回时一律忽略「中」报告（不采纳）：最简单，也能保住双拼；但「中」报告同时决定中英文状态，用户从 ABC 切回并在「英」和「中」之间选择时，忽略它会让控制器停在英文。只放宽方案规则，中英文状态照常跟随报告。
- 把判断放进 `MSIMEAdoptReportedInputMode`，让它返回「是否切回后的第一次报告」：少一个局部变量；但它的契约是「是否采纳」，混进第二个含义会让控制器和测试里的每个调用方都要理解它。控制器在采纳前读一次 `current` 更直接，单测按同样顺序复现。
- 维持原规则，只在 issue 里建议用户删掉「双」入口：不改代码；但两个入口都加是合理的配置，只要加了就会反复出现。维护者已决定修。

## Testing

`platforms/macos/tests/input/InputModeIdentifiersTest.mm` 覆盖两条路径：`switchedIn` 为 YES 时，「中」在双拼、五笔、全拼上都不改方案，从日文回来仍回到进入前的双拼，「双」「日」仍选各自方案；随后按控制器的顺序模拟「离开清空记录 → 切回报告中 → 方案不动 → 选回双 → 在水杉里再报告中 → 改全拼」，并断言激活校准后的重复「中」不被采纳。原有 `switchedIn` 为 NO 的断言保持不变。去掉修复时新断言失败。

Mac Studio 上用与 CMake 目标相同的编译选项（`-fobjc-arc -UNDEBUG -Wall -Wextra -Werror`，C++17）编译运行该测试通过；`InputController.mm` 和 `input_method_main.mm` 用 `-fsyntax-only -Werror` 检查通过。没有跑完整 macOS CMake 构建和 `ShortcutTest`：`ShortcutTest` 里系统模式报告的用例在清空记录后的第一次报告都是「英」「粤」「한」「日」，不经过改动的分支。

## Consequences

收益：从 ABC 等输入源切回不会再把双拼、五笔、粤拼、注音、笔画用户改成全拼，偏好不再被意外改写，菜单栏回到方案自己的模式。

代价：两个入口都加了的用户，想在切回水杉时直接从 ABC 选「中」改用全拼，需要切回后在水杉里再选一次「中」，或从方案菜单换。这是维护者明确接受的取舍。

没有真机验证：系统在切回时报告「中」、随后控制器选回「双」的实际顺序，以及之后系统是否还会补发一次滞后的「中」报告（那会被当成水杉内的切换），都只能在装机后用两个入口的配置验收。
