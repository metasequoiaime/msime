# Agent Note: 运行时移动高亮越过云占位行

Status: implemented

## Problem

#10665：Windows 0.4.1 开着云候选打全拼，按 ↓（横排时按 →）候选窗立刻消失，之后打不出中文，要在设置里重启输入法才恢复。竖排时 ←/→ 不移高亮，所以不出；横排时会出。

[云候选固定席位](../feature/2026-10-17-cloud-candidate-seat-placeholder.md)在第二格放一行 `CandidateSource::CloudPending`（来源 15，文字「☁」），云回包没有新词时它留到组字结束，常见词的第二格几乎总是它。[Windows 隐藏了这一行](2026-10-22-windows-hides-cloud-pending-seat.md)（#9273），[macOS 也隐藏了](2026-10-27-macos-cloud-placeholder.md)（#9951），移动三端从一开始就不画它。但共享运行时移动高亮时不跳过它：`Action::NextCandidate` 直接算 `highlighted + 1`，从首选按 ↓ 正好落到下标 1。

落在那里之后各宿主的表现：

- **Windows**：Server 的候选窗投影（`CandidatePresentation.h`）跳过不可见行之后要求可见行里正好一个高亮，没有就抛 `Invalid candidate highlight`。这发生在输入队列的任务里（`SessionPump` → `CandidateMailbox` → `candidate_presentation`），`InputQueue.cpp` 的 `catch(...)` 停掉整个 Server，日志是 `Server stopping: session controller failed (input queue)`。报告者的三份日志里，只有一份完整记到方向键和这条日志；另两份在方向键记录落盘之前 Server 就停了，时序吻合但没有直接看到。
- **macOS**：高亮落在不画的行上，看起来就是高亮消失了。
- **Linux**：照旧把「☁」画成次要色，高亮停在一个选不中的格子上。
- **移动端硬件键盘**：同 macOS。

隐藏这一行的两次修复都只改了宿主的显示和按位置选词，没有人改共享运行时的高亮移动。

## Decision

高亮**never** 停在云占位行上，这条规则放在共享运行时（`crates/input-runtime/src/runtime.rs`），所有宿主同时生效：

- `seat_takes_highlight` 判断一个座位能不能停高亮：只排除来源 15，与 Windows `candidate_source_is_visible` 同义。已退役的 AI 占位行（12）引擎不再产出，规则不管它。
- `highlightable_seat` 从一个座位起朝一个方向找第一个能停的座位。
- 每一处算新高亮的地方都经过它：↑/↓ 朝移动方向找，找不到就留在原地；Home 从头往后找，End 从尾往前找；`+`/`-` 先在目标页里往后找，`-` 在整页都是占位行时（每页一个）再往前找；横排左右键走的是同一个 `NextCandidate`/`PreviousCandidate`，不另外处理。
- 组字没变时的异步刷新（`keep_highlight_on_same_candidate`）找不回原来那一条、退到那一页开头时，那个位置也可能是占位行（每页一个、列表变短），同样挪开。
- `expand_for_next_candidate` 判断「到了末尾」时，后面只剩占位行也算末尾，于是 ↓ 照样先放出引擎扣下的候选。

Windows 候选窗投影的不变式**不放宽**，也不加防御性 `try/catch`：高亮在不可见行上就是运行时的错，抛出能让它暴露出来，而不是画出一页没有高亮的候选。

### 跟着改的调用方

有几个宿主不读回高亮，而是按发出的命令数推算它在哪。运行时越过占位行后，一次「下一个」可能走两格，这些推算就会差一格，所以一起改：

- **macOS 收起展开网格**（`closeExpandedCandidates`）：原来「下一个」就把位置加一，经过占位行会多走一格。改为每一步从回复的视图读回真实高亮，步数以身份距离为上限，和 `moveEngineHighlightToIndex` 一样。
- **日文空格转换**：Engine 对日文罗马字同样放云占位行（`online_query` 里罗马字 `cloud_eligible`）。状态机在 `shared/input/JapaneseConversion.h`，宿主数第几次空格、回车按这个下标选词。现在约定：数的是去掉云占位行之后的行。macOS 用 `MSIMEVisibleCandidates`，IBus 和 Fcitx5 用 `japanese_conversion_rows`，Windows Server 只数 `candidate_source_is_visible` 的行。macOS 和 Linux 的回车按下标选词，不改的话越过占位行之后会上屏高亮前面那一个。

## Alternatives considered

- **在 Windows 候选窗投影里容忍高亮落在隐藏行上**（例如把高亮挪到下一可见行，或者不抛出）——改动最小，只碰出事的那一端。否决：运行时和宿主对高亮在哪各执一词，之后空格、数字键和释义请求都按运行时的高亮走，显示的和选中的会分叉；macOS 和移动端的「高亮消失」也照样存在。
- **把云占位行从 `visible_len` 里去掉**，让运行时整体不认它——高亮、数字键、分页一次都对齐。否决：占位行仍占引擎下标和每页一格，移动三端和 Windows 都按引擎下标做显示序号映射（[Windows 那篇](2026-10-22-windows-hides-cloud-pending-seat.md)已经否决过在运行时改数字映射），`visible_len` 是可见前缀长度，占位行在第二格，不是一个前缀能表达的。
- **只改 ↑/↓/End 那几个匹配分支**——分诊最初给的范围，改动最小。否决：Home/End、每页一个时的翻页和异步刷新的回退位置都会把高亮放到同一行上，一个 helper 全部经过它，下次不会再漏一条路径。

## Consequences

- **收益**：Windows 不再因为方向键停掉 Server；macOS 和移动端硬件键盘的高亮不再消失；Linux 上方向键越过画出来的「☁」。
- **可见的行为变化**：Linux 照旧画「☁」，方向键现在越过它。它本来就选不中（`commit.rs` 对它「什么都不做但算处理过」），不是改出厂默认值。
- **未改的计数**：Android（`MSIMEInputService` 日文空格）和 HarmonyOS（`KeyboardSession.convertJapanese`）的日文空格仍按原始行数决定何时回到首条。它们的回车上屏高亮候选，不会选错词；页里有占位行时，走到本页末尾会多走一步（落到下一页第一个）才回到首条。修复前同一位置的那一步落在看不见的占位行上，高亮消失。
- **重访信号**：引擎再加一种选不中的占位来源，或者 AI 占位行（12）重新被产出，`seat_takes_highlight` 和宿主的可见性规则要一起改。

## Verification

- `crates/input-runtime/src/tests.rs`：`arrow_keys_skip_the_cloud_placeholder`、`horizontal_left_right_skips_the_cloud_placeholder`、`paging_one_by_one_skips_the_cloud_placeholder`、`a_late_refresh_does_not_leave_the_highlight_on_the_cloud_placeholder`、`only_the_cloud_placeholder_is_skipped`。前四个在修复前失败；异步刷新那条另外单独验证过：只去掉 `keep_highlight_on_same_candidate` 里的挪开，其余修复保留，它仍然失败（高亮停在「☁」）。
- `platforms/windows/tests/candidate/cloud_pending_presentation.cpp`（`windows-cloud-pending-presentation`）：高亮在看得见的行上时两条投影路径都照常生成，高亮在占位行上时投影仍然拒绝。
- `platforms/linux/tests/core/japanese_conversion.cpp`：去掉占位行之后，第二次空格停的行就是回车选的行。
- 没有在真机上复现报告的现象再确认消失。需要 Windows 11 实机开云候选、横排，按 ↓/→ 确认候选窗不消失、`server.log` 里没有 `session controller failed (input queue)`；macOS 上同样开云候选，从首选按 ↓ 确认高亮落在第二个看得见的候选上。
