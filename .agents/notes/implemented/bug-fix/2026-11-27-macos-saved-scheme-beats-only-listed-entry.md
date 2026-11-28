# Agent Note: macOS 只加了一个水杉中文入口时以设置里的方案为准

Status: implemented

## Problem

Issue #11911：报告者在系统输入法列表里只加了「水杉双拼」，在设置里选了全拼。输入菜单里勾着的永远是「水杉双拼」，看起来像方案被强制改成了双拼，实际打的是全拼。设置页「输入 › 方案」的「菜单栏入口」本该说明缺的是「水杉全拼」，却什么都没指出来。

查下来是三处：

- 控制器在进程刚启动时会把方案改掉。[macOS 从别的输入源直接选「水杉全拼」时照它改方案](2026-11-12-macos-switch-in-honors-picked-mode.md) 加的 `MSIMEReportRepeatsLeftInputMode` 只在离开时的入口 `left` 已知时保护「只有双一个入口、方案是全拼」的人。重新登录、更新或输入法进程重启后 `left` 是 nil，系统切进来报告「双」，`systemDidReportInputMode:client:` 经 `MSIMESchemeForReportedInputMode` 把方案改成双拼并写回偏好。`ShortcutTest.mm` 的新用例在旧代码上就停在这里。
- 共享设置页把方案映射到入口时，全拼找不到入口。`packages/ui/src/settings/macos-input-modes.ts` 的表里 `Hans` 的 `scheme` 是 `null`，`useMacosInputModes` 用 `entry.scheme === scheme` 找当前方案的入口，全拼永远落空，只剩「还有 N 个入口没加进输入法菜单」。五笔版、日文版等把方案放在 `Hans` 上的版本同样落空。原生的 `MSIMEInputModeIDForSchemeIn`（`platforms/macos/src/input/InputModeIdentifiers.h`）一直是「方案自己的模式，本版本没声明时是 `Hans`」。
- 缺当前方案入口的那句话放在「?」里，不打开看不到。

## Decision

方案以设置里存的为准。`MSIMEReportIsOnlyChineseEntryIn(info, scheme, lastChineseScheme, reported, available)` 在下面几条同时成立时为真，控制器这时不改方案（当前在日、한、越、藏上时回到 `lastChineseScheme`）：

- 报告的是本 bundle 的中文入口（中、双、五、粤、注、笔），而且系统列表里有它；
- 设置里存的中文方案自己的入口（`MSIMEInputModeIDForSchemeIn`）不在列表里。当前方案是 japanese、korean、vietnamese 或 tibetan 时，存的中文方案是 `lastChineseScheme`，和 `MSIMESchemeForReportedInputMode` 取 `chinese` 的方式一致：存了全拼、列表里只有「双」和「日」的人切到日再选回「双」，要看「水杉全拼」在不在列表里；只看当前方案「日」自己的入口，它在列表里，判断就落空，方案被存成双拼。这条路径上控制器回到 `lastChineseScheme`；
- 列表里没有别的中文入口。

这时用户想用水杉就只能落在被报告的入口上，报告说明不了用户想换方案，不论是刚切进来、`left` 未知，还是从「英」切回来。其余情况照旧：方案自己的入口在列表里、还有别的中文入口可选（例如同时加了「五」），或者报告的是日、한、越、藏这类换语言的入口，报告仍按用户的选择改方案。读不到列表、或者列表里还没有被报告的入口时也照旧，不去猜。

菜单栏这时仍停在系统能显示的那个入口上，`MSIMESelectSystemInputMode` 不会去选列表里没有的「中」；从「英」切回中文时它退到唯一可选的中文入口，见 [方案的入口和「中」都不在列表里时选唯一的中文入口](2026-11-28-macos-fallback-to-only-listed-chinese-entry.md)。菜单栏状态项照旧显示真实方案。

共享设置页新增 `macosInputModeEntryForScheme(scheme, edition)`，规则与 `MSIMEInputModeIDForSchemeIn` 相同：本版本列出了方案自己的入口时是它，否则是 `Hans`。「菜单栏入口」用它找当前方案的入口；当前方案的入口没加时，在标题下常驻一行「输入法菜单里还没有当前方案的「水杉全拼」，请到「系统设置 › 键盘 › 文字输入 › 输入法」的「简体中文」下添加。」，按「设置项说明收进「?」」的规则，这是状态而不是解释。「?」里只说没加之前菜单栏会停在别的入口上、打字仍按这里选的方案。macOS 的设置还没有原生页面可以接新行（[全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)），这一处是对已有行的缺陷修复，没有新增设置；冻结的 AppKit 回退窗口本来就用原生映射，不动。

## Historical audit

- [macOS 从别的输入源直接选「水杉全拼」时照它改方案](2026-11-12-macos-switch-in-honors-picked-mode.md)：部分重叠，补上它留下的 `left` 未知那一段。那篇的 `left`、`MSIMEReportRepeatsLeftInputMode` 和「离开时在双、回来选中」的规则都不变，只在它的 Decision 里加了指向本篇的一句。
- 2026-11-26 的调查提案（#11920）要求先拿到报告者的构建号和日志再决定是否改代码。维护者随后在 issue 里直接决定了行为，提案被本篇取代并删除。它记下的事实（`preferences.scheme` 是权威、`MSIMEClientInputScheme` 只是输入法自己的本地回退、`TestFreshProcessActsOnTheSharedSchemeNotTheStaleLocalOne` 已覆盖本地回退的那条路径）仍然成立，本篇没有改那条路径。它否掉的两条路也仍然不走：把所有系统报告一律改成全拼，以及删掉 `MSIMEClientInputScheme`。

## Alternatives considered

- 进程刚启动、`left` 未知时，任何入口的报告都不改方案：规则最简单，覆盖得也最全。不选它，是因为它把同时加了「中」和「双」的人在登录后从 ABC 明确选「双」也吞掉了，而那时报告确实是选择；本篇只在报告不可能是选择（唯一可选的中文入口）时才不改。
- 把日、한、越、藏也算进「唯一入口」：保持「设置为准」更彻底。不选它，是因为从别的输入源选「水杉日语」是换语言，列表里方案自己的入口缺不缺都不影响这一点；把它吞掉，用户得先切走再选回来才能进日语。
- 把 `left` 持久化进 NSUserDefaults，让进程重启后也知道离开时的入口：11-12 那篇已经考虑过。它只能覆盖「离开时也在双」这一种，用户第一次装上、或上次离开时在「英」时仍然会被改掉；本篇的判断不依赖离开时的入口。
- 方案自己的入口不在列表里时，控制器改选列表里唯一的中文入口，让系统菜单至少不停在「英」上：能减少一种显示不一致。这次没做，它会让控制器主动去选一个和方案不符的入口，回执的处理还要再推一遍；先把方案保住、在设置里说清楚缺哪个入口。这样留下的显示问题（从「英」切回中文后菜单栏仍停在「英」）由 [方案的入口和「中」都不在列表里时选唯一的中文入口](2026-11-28-macos-fallback-to-only-listed-chinese-entry.md)（#11963）接着处理，那篇采用了这条路。

## Consequences

收益：只加了「水杉双拼」（或「五」「粤」「注」「笔」中任意一个）而在设置里选了别的方案的人，重新登录、更新、来回切换之后方案都不会被改掉，设置页常驻告诉他缺的是哪个入口、去哪里加。设置页和原生对「方案 → 入口」的理解一致，五笔版等单方案版本也能指出缺的入口。

代价：这类用户在输入菜单里看到的入口名仍与方案不符，直到他把方案自己的入口加进列表。真想改用双拼的人，只加「水杉双拼」不再够，要到设置或输入菜单的方案里改。判断要按中文入口逐个查一次 TIS，只发生在采纳系统报告时，次数很少。

没有在真机上验证：控制器的行为由单元测试覆盖，设置页由组件测试覆盖。

## Testing

`platforms/macos/tests/input/InputModeIdentifiersTest.mm` 覆盖 `MSIMEReportIsOnlyChineseEntryIn` 的各个分支：只有「双」时为真（全拼和五笔都是），方案自己的入口在列表里、还有「中」或「五」、报告的是「英」「日」或别的输入源、读不到可用性、列表里还没有被报告的入口时为假；五笔版的五笔按 `Hans` 找；当前方案是日或韩时按 `lastChineseScheme` 的入口判断。

`platforms/macos/tests/input/ShortcutTest.mm` 的 `TestOnlyListedShuangpinKeepsSavedScheme` 在真实控制器上模拟只加了「双」「英」的系统列表：进程刚启动报告「双」，方案留在全拼、本地偏好也没被改、不去选「中」；切到「英」再选回「双」仍是全拼；列表里只有「双」「日」时切到日再选回「双」回到全拼、本地偏好也是全拼；列表里有「五」或「中」时报告「双」照旧改成双拼。去掉控制器里的这道判断，第一条断言失败。

`apps/desktop/tests/settings/macos-input-mode-entries-section.test.tsx` 覆盖全拼只加了「双」时常驻显示「水杉全拼」（不在「?」的读屏副本里）、全拼入口已加时不提示、`macosInputModeEntryForScheme` 对全部方案和五笔版的映射，以及五笔版缺「水杉五笔」时的提示。把映射换回 `entry.scheme === scheme` 时其中两条失败。
