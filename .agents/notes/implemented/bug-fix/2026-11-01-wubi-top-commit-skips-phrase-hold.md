# Agent Note: 五笔顶字立即上屏，不进暂存词段

Status: implemented

## Problem

六个宿主都向 runtime 请求 `phrase_preedit`（安卓 `MSIMEInputService.java`、iOS `MetasequoiaInputSessionBridge.swift`、鸿蒙 `KeyboardSession.ets`、macOS `InputController.mm`、Linux `ClientEngine.cpp` / `FcitxEngine.cpp`、Windows `PhrasePreeditOptions.h`）。在这个能力下，26 键五笔完整四码后再敲字母（`wqiy` 接 `a`）时，`Runtime::dispatch` 把字符键算作选择动作，`hold_phrase_progress` 看到「有提交、组字还在」，把顶下来的「你」收进 `phrase_prefix`，不写入文档。

这不只是「晚一点上屏」。顶字判定 `wubi_top_commit_on_next_key` 与四码唯一自动上屏 `wubi_auto_commit_on_fourth_key` 都要求没有暂存词段，所以第一次顶字之后：

- 再打一个完整四码，第五个字母不再顶字，引擎在混输开着时也续不上拼音，码被截在四位，之后的字母全部被吞掉（实测 `wqiyaaaaggll` 后编辑串停在 `aaaa`，`ggll` 一个都没进）；
- 四码唯一自动上屏同样失效；
- 这时按 Esc，`hold_phrase_progress` 的 `discard` 分支连同已经顶下来的「你」一起丢掉。

14 键五笔的顶字走 `Action::GridKey`，它不在选择动作里，一直是立即上屏，两种键盘行为不一致。

## Decision

五笔顶字（`wubi_top_commit` 为真的那次字符键）不算造词中途的选词：`picked` 排除它，顶下来的首选和尾巴照常作为这次转换的提交送出，`phrase_prefix` 保持为空，第五码开始的新编码是一段全新的组字。之后的顶字、四码唯一自动上屏、Esc 只取消未上屏编码，都按没有暂存词段的规则走。

范围只限顶字。拼音、双拼、日文的部分选词，九宫格到上限时的 `GridLimit` 代选，以及五笔混输里真正只消耗部分拼音的选词，仍按原规则暂存；Esc 丢弃暂存词段（`escape_keeps_selected_word` 关时）的规则不变，那是造词中途取消整个词，与顶字无关。

宿主不用改：Windows 的 `ReplyComposer::edit` 本来就把五笔字符键的提交路由到 `AutoCommitAndContinue`，消费数 `wubi_continue_consumed` 早已按「顶字后的小写字母为 4、继续组字」设计，大写字母顶字一直走这条路；安卓和其他宿主收到的是一次「上屏 + 新组字」转换，与韩文音节延续、14 键顶字相同。

## Historical audit

- [五笔 z 通配与第五码顶字开关](../feature/2026-10-21-wubi-input-switches.md)：部分取代，那里记的「顶字和其他选词一样暂存」改为指向本篇，开关与真值表仍由它持有。
- [Windows 五笔最终提交文本](2026-10-23-windows-wubi-commit-owner.md)：部分重叠，写入归属不变，只把 `abcdx` 的边界事实改为立即上屏、`phrase_prefix` 为空。
- [五笔顶字保留后续按键](2026-10-10-wubi-top-commit-tail.md)：无冲突，尾巴合并规则照旧，现在尾巴随首选直接上屏。
- [五笔 14 键](../feature/2026-10-25-wubi-fourteen-key.md)：无冲突，26 键与它对齐。
- [五笔拼音续输](2026-10-18-wubi-mixed-pinyin-continuation.md)：无冲突，续输时 `wubi_top_commit` 为假，不受影响。

## Alternatives considered

- **保持暂存（现状）**：与 Windows 回归当时核对的边界一致，不动跨平台行为。不采用：它不是一个可以接受的产品取舍，而是一个吞键的缺陷——第一次顶字之后再也不能顶字，续打的码丢失，Esc 丢掉用户已经定下的字。Rime 五笔（`speller/auto_select` + `max_code_length: 4`）、极点、微软、搜狗五笔的顶字都是首选立即上屏，水杉 14 键也是。
- **继续暂存，但 Esc 保留顶下来的字**：能堵住 Esc 丢字。不采用：只修了三个症状里的一个，暂存挡住后续顶字和四码唯一自动上屏的问题仍在；还要在 `hold_phrase_progress` 里区分「顶字来的词段」与「造词选的词段」，同一个 `phrase_prefix` 里混着两种语义，回退栈和 Esc 都要各算一遍。
- **放开「无暂存词段」这两条守卫**：让暂存时也能顶字和自动上屏。不采用：守卫保护的是造词中途的真实暂存（混输里部分选中的拼音词），在那时顶字会提前结束用户还在拼的词。

## Consequences

收益：五笔连打在所有开着 `phrase_preedit` 的宿主上恢复成标准顶字体验，不再吞键；Esc 只取消正在打的一组编码；26 键与 14 键一致；Windows 的双会话写入路径和大写字母顶字是同一条。

代价：顶字的提交现在跨越宿主的组字边界（一次转换同时有上屏和新组字），各宿主依赖已有的 `commit{continues}` 处理（[桌面生命周期](../architecture/2026-10-11-desktop-lifecycle-core.md)）。安卓设备验收、Windows `windows-wubi-commit-owner` 的 72 个双会话场景按新边界改断言；真机上的连打与 Esc 未在本次验收。

## Testing

- `msime-input-runtime`：`a_fifth_letter_after_wqiy_commits_the_topped_word_even_with_phrase_preedit` 用真实会话打 `wqiy` `a`、`aaa` `g`、`gll` 再 Esc：两次顶字各自上屏「你」「工」，第二组码正常进入编辑串，Esc 后没有提交、暂存为空。去掉 `&& !wubi_top_commit` 后这条在第一次顶字处失败（`left: None, right: Some("你")`）；恢复后 `cargo test -p msime-input-runtime` 422 项通过。
- Windows `tests/input/wubi_commit_owner.cpp`：非唯一、小写第五码的场景改为期望文档写入一次「合成国」、余码 `x`、`phrase_prefix` 为空。本机无 MinGW，交给 PR 上的 Windows CI。
- 安卓 `FourteenKeyWubiDeviceChecks`：输入框仍是「你a」（`a` 按默认写进组字区），读音行不再画暂存的「你」，再选「工」得到「你工」。交给设备 CI。
