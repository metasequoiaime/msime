# Agent Note: 表情行不占首位，在线席位也不许被提到首位

Status: implemented

## Problem

Windows 上开着云候选（Google）和 emoji 混输，打 `gi`、`wi`、`git`、`github` 这类拆不成合法音节的字母串时 Server 十次十次停机：候选窗消失，之后打不出中文，`nginx` 这类有本地词的输入正常。日志里是 `Invalid candidate highlight`，经 `InputQueue` 停机后以 `session controller failed (input queue)` 停掉整个 Server。

`gi` 的本地拼音没有候选，列表里只剩英文、emoji 混输行和云候选的固定占位席位（`CandidateSource::CloudPending`）。`crates/engine/src/ime/queries.rs` 的 `keep_expressive_rows_off_the_lead` 负责把这些表情行赶下首位：先找第一个「没有固定位置、也不坐在线座位」的词，找不到时退而取任意第一个非表情行。`CloudPending` 不是表情行，兜底那一步于是把它提到了首位。

席位没有被选中、也选不中的语义没变，问题出在默认高亮：新列表的高亮恒为 0，现在正好停在席位上。Windows 候选窗按 [隐藏云占位席位](2026-10-22-windows-hides-cloud-pending-seat.md) 不收这一行，`candidate_presentation` 又要求可见行里恰好一个高亮，于是这一页一个高亮行也没有，它在输入队列的送达回调里抛异常，`InputQueue` 停机，整个 Server 重启。同一份列表在 Android、iOS、HarmonyOS 上也同样把高亮放在隐藏行上，只是它们的宿主没有把「可见行高亮数」当成不变量。

## Decision

兜底那一步也排除在线行（`CandidateSource::takes_online_seat()`：`CloudPending`、`AiPending`、`CloudSuggestion`、`AiSuggestion`）。没有可挪的词时列表保持原样，席位留在引擎给它的位置，高亮停在它前面的表情行上。

第一条规则（「没有固定位置、也不坐在线座位」）不动：在线行和固定位置的行都原地不动，只把它前面的表情行往后挪。兜底放宽到固定位置的词，仍然不碰在线行。

在线席位的位置与可见性仍由宿主决定：Windows 继续隐藏 `CloudPending` 并按 [云占位席位那篇](2026-10-22-windows-hides-cloud-pending-seat.md) 的规则让导航跳过它，这里只保证引擎交付的顺序不把它顶到首位。

## Alternatives considered

- **在 Windows 候选呈现里把高亮让给第一个看得见的行** — 改动只在一处，任何原因把高亮放到隐藏行都不会再停机。否决：显示与选择会分叉（高亮画在第 1 格、引擎的第 0 格是另一个身份），而且只修 Windows；引擎交付的顺序本身仍然是错的，Android、iOS、HarmonyOS 上高亮照样停在隐藏行上。`leave_hidden_seat` 已经把「导航落到隐藏席位」这一种处理掉了，这里再多一条同样的让位规则，等于承认引擎可以随便把席位放首位。
- **没有云查询时不产生占位席位** — 直接消掉这类只有表情行的列表，最省事。否决：席位是为了让云回包到达时不挤动用户正在看的候选（[云候选固定席位](../feature/2026-10-17-cloud-candidate-seat-placeholder.md)），它和「这一页还有没有云查询」没有稳定关系；去掉要同时改动在线批处理、宿主可见性和分页，代价远大于一条兜底条件。
- **把 `CloudPending` 当作表情行一起挪** — 让一类来源在规则里看起来统一。否决：席位不是表情行，它跟着表情行挪到词后面会改掉云候选的固定位置，回包到达时替换的格子也跟着变。

## Consequences

- 收益：只剩表情行时，列表不再把云占位席位放到首位，高亮停在看得见的行上；报告者的 `gi`、`wi`、`git`、`github` 不再让 Windows 的 Server 停机。
- 代价：只剩表情行且云候选开着时，首位仍是表情行而不是席位。Windows 看不到这个差别（席位本来就不显示）；Android、iOS、HarmonyOS 和 Web 上首位从「☁」变成表情行，高亮也就不再落在隐藏/次要行上。
- 未覆盖：这次没有在 Windows 装机上复验报告者的键盘；回归靠引擎自测和报告者，Windows 侧的 `candidate_presentation` 仍会对任何来源的隐藏席位报错，宿主侧的防御性让位不在本篇范围。
- 重访信号：如果以后取消 `CloudPending`、或者让在线行也能被选中，这里的 `takes_online_seat()` 判定要跟着改。

## Verification

- `crates/engine/src/ime/queries.rs` 的 `online_seats_never_take_the_lead_from_expressive_rows`：覆盖四种在线来源，只有表情行时都不动；席位后面有词时那个词照常提到首位。修复前 `["☁", "🎁", "🧡"]`、修复后 `["🎁", "☁", "🧡"]`。
- `crates/engine/src/session/tests.rs` 的 `an_expressive_only_list_keeps_the_cloud_seat_off_the_lead`：合成词库加合成 emoji 目录，经真实 Session 打 `gi`、云候选开着，断言席位不在首位、首位是 emoji。修复前该测试以 `["☁", "🎁", "🦒", "👧"]` 失败，修复后通过。
- `cargo test -p msime-engine --lib` 全量通过（含九宫格与符号混排里调用同一个函数的用例）。
