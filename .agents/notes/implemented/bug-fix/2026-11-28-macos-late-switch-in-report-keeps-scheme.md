# Agent Note: macOS 切进后立刻打字时，迟到的模式报告不改设置里的方案

Status: implemented

## Problem

#11391：macOS 0.59.4、微软双拼，打得快时字母之间全部分词、被当成英文候选，或者不显示候选窗。报告没有按键串和日志，维护者先按待复现处理（见下面的历史审计）。

按代码能确认一条和「打得快」直接相关的时序，在 `platforms/macos/src/input/` 里。切进水杉后，共享记录 `MSIMESystemInputModeState` 的 `switchedIn` 为真，表示还没见到用户在水杉里的选择；`handleEvent:client:` 收到第一次按下的键就把它清掉。切进时系统发来的那份模式报告（`setValue:forTag:client:` → `systemDidReportInputMode:client:`）并不保证先于按键送到：切进后立刻打字，尤其是进程刚被拉起（登录后、Sparkle 装完更新、崩溃重启，这时 `left` 未知），报告的「中」会排在头几个键后面。此时 `switchedIn` 已经是假，`MSIMESchemeForReportedInputMode` 把这份迟到的「中」当成用户越过「双」选了「中」，返回本版本的默认方案（full 是全拼），控制器先把正在组的字母按双拼提交，再把全拼写进偏好。之后的双拼按键按全拼切分，一个个字母各成一段，也就是报告里的现象；方案被保存，之后一直如此。

这和 #11911 那里维护者定下的原则冲突：以设置里保存的方案为准，系统输入源那边的状态不应把它改掉。

## Decision

第一次按键不再一下子结束「刚切进」。共享记录多一个字段 `switchInLateUntil`（systemUptime 秒）：

- `MSIMENoteSystemInputModeKey(state, now)`：`handleEvent:client:` 收到按下的键时调用。`switchedIn` 仍被清掉；若这是切进后的第一次按键，`switchInLateUntil` 设为 `now + MSIMESwitchInLateReportSeconds`（2 秒）。之后的按键不延长它。
- `MSIMESystemInputModeSwitchedIn(state, now)`：`switchedIn` 为真，或者还在 `switchInLateUntil` 之前。`systemDidReportInputMode:client:` 用它代替原始的 `switchedIn`，交给 `MSIMEReportReturnsToLeftInputMode`、`MSIMEReportRepeatsLeftInputMode` 和中英状态的起点规则，所以迟到的切进报告和准时的一样处理：报告的是离开时的入口（或入口未知）时方案不动，菜单栏选回方案自己的模式。
- `MSIMEFinishSystemInputModeSwitchIn(state)`：采纳一份报告后调用，`switchedIn` 和窗口一起结束。离开水杉时 `MSIMEResetSystemInputModeState` 把整个记录清空，窗口也随之归零。

英文切回中文的回执判断（`reportConfirmsEnglishToChineseSwitch`）仍看原始的 `switchedIn`：那是在水杉里按 Shift 之后的事，不是切进，换成新的判断会让窗口内的回执被当成切进报告。

窗口只放宽「这份报告算不算切进」，不改切进报告本身的规则：离开时在「双」、回来报告「中」仍按用户从别的输入源选了「水杉全拼」处理（[macOS 从别的输入源直接选「水杉全拼」时照它改方案](2026-11-12-macos-switch-in-honors-picked-mode.md)），窗口内外都一样。

## Historical audit

- [跟踪 macOS 快速双拼输入异常](../../rejected/bug-fix/2026-11-26-issue-11391-macos-fast-shuangpin-followup.md)：被取代。那篇等报告者补复现资料；本篇按代码找到了一条能解释现象的时序并直接修掉，那篇改为 rejected。
- [macOS 从别的输入源切回时报告「中」不改双拼](2026-10-29-macos-keep-shuangpin-on-return.md)：部分重叠，补上它没覆盖的顺序。那篇让「第一次按下的键」结束刚切进，只考虑了报告先到和校准先到两种顺序，没有考虑报告排在按键之后；本篇保留按键的作用，只给切进报告留一个短窗口。
- [macOS 从别的输入源直接选「水杉全拼」时照它改方案](2026-11-12-macos-switch-in-honors-picked-mode.md)：不改。`left` 的判断照旧，只是喂给它的「是否刚切进」改由新函数给出。
- [macOS 英文按 Shift 回中文时保留双拼](2026-11-10-macos-shift-keeps-shuangpin.md)：不改，回执判断继续看原始标记。
- [调查 macOS 方案选择后显示与实际方案不一致](../../proposed/bug-fix/2026-11-26-issue-11911-macos-scheme-selection.md)：同一原则。#11911 里维护者定下以设置里保存的方案为准，本篇是它在模式报告时序上的一处落实，不改那篇。

## Alternatives considered

- 按键完全不再清 `switchedIn`，只有采纳一份报告才结束刚切进：最简单，迟到多久都不会误改。不选，是因为系统不一定发切进报告（重复已校准入口的报告不采纳），这时 `switchedIn` 会一直为真，进程刚启动后打了一个小时字再在输入菜单里明确选「中」，也被当成切进而不改方案；10-29 那篇把这种代价限定在「还没按键」之内，这样做会把它扩大到整个会话。
- 迟到的报告一律不改方案，只改中英状态：最符合「设置为准」的字面意思。不选，是因为分不出迟到的切进报告和用户在输入菜单里选的入口，到达控制器的是同一份报告；一律不改会让在水杉里选「水杉全拼」永远不生效，回到 11-12 那篇修掉的问题。
- 窗口从激活（`activateServer:`）算起而不是从第一次按键算起：不依赖按键。不选，是因为焦点在客户端之间移动也会激活，分不出切进；从按键算起，窗口只在切进后的第一次按键时打开一次。

## Consequences

收益：切进后立刻打字，迟到的切进报告不再把双拼（以及五笔、粤拼、注音、笔画）改成全拼并写进偏好，正在打的双拼不再按全拼切分；默认英文的人切进后立刻打字，迟到的报告也不再把英文翻回中文。

代价：切进后的第一次按键之后 2 秒内，用户在输入菜单里明确选离开时的入口（或进程刚启动时选「中」），这一次不改方案。打了字、发现不对、再打开输入菜单选别的入口，通常超过这个时间。2 秒是按代码推的取值，没有设备上的报告延迟数据；诊断日志里被采纳的 `input_mode_report` 行带 `switched_in=`，下一份日志能看出迟到报告是否落在窗口内。

没有设备验证：报告者的现象是否就是这条时序，没有日志能确认；只按代码和单元测试确认。

## Testing

`platforms/macos/tests/input/InputModeIdentifiersTest.mm`：第一次按键后窗口内仍算刚切进、到点结束；之后的按键不延长；采纳报告结束窗口；切进已处理后再按键不开新窗口；离开水杉清掉窗口。

`platforms/macos/tests/input/ShortcutTest.mm` 的 `TestSwitchBackKeepsShuangpin`：原来「按过键之后报告『中』改全拼」那一段改为「按键后迟到的『中』保留双拼、菜单栏选回『双』，之后再报告『中』才改全拼」，另加一段把截止时间挪到此刻之前，模拟窗口过去，报告的「中」改成全拼。把控制器换回原始的 `switchedIn` 时第一段失败。
