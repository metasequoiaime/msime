# Agent Note: 固定到非首位后取消固定作用于错误的候选

Status: implemented

## Problem

用户在本机图形会话（终端、浏览器、文本编辑器）报告：修前把候选固定到 3–5 实际落到第 2 位；修后可以正确固定到各位置行，固定候选词、取消固定、删除自造词、置顶候选词均正常。按机制追查，右键动作原样到达引擎（如 `CandidateFix3/1/1290/1`）且确实把候选放到目标槽位，问题不在扩展或 Linux 宿主；两个独立根因如下。

- 固定会重排候选，但运行时高亮没有跟过去。[异步刷新保留候选页](2026-10-20-async-refresh-keeps-candidate-page.md)有意规定「组字没变、高亮原本在第一行、候选变了仍留在第一行」。用户在第一行固定到第 N 位时，被操作的行移到第 N 行，高亮仍在第 0 行；Linux 的 `Ctrl+0` 和右键菜单都按高亮行生成动作，于是取消的是另一行。这是机制判断，不是用户目视报告的「删错行」。
- 固定记录的 `entry_key` 按当前行的来源现算：首次固定时取 `canonical_pinyin`（如 `aa`），重排后同一行的来源换成五笔、改取 `pinyin`（如 `aad`）。显示投影只按词面匹配，同一个字因此出现两条记录，取消只按当前行的键删一条。完整状态与显示投影分离的规则见[学习同步提案](../../proposed/feature/2026-10-27-learning-sync-e2e.md)。`aa` 里重复「式」的另一个根因是自学习把全码那行提前，另见另一条 PR 附带的笔记 `2026-11-25-wubi-same-word-different-code-learning.md`。

## Decision

共享 `Runtime` 只在 `FixCandidatePosition` / `ClearCandidatePosition` 两个动作上做定向高亮重映射。发出引擎调用前，从用户看到的快照记下目标行的文字、编码、来源和座位；刷新完成后，固定动作按「文字相同且坐在目标槽位」找新行，取消动作按「文字、编码、来源相同」找新行，找到就把 `highlighted` 挪过去并推进候选代次。找不到时保留刷新的结果，不越界。异步刷新、云词插队和静止重排的高亮规则一律不动。

固定槽位只有一个主人，所以固定后的查找不要求编码一致：同一行的来源变了仍能跟上。取消时目标槽位已经清空，只能靠动作前的身份找回那一行。

`position_state::set` 在同一 IMMEDIATE 事务里先删掉「同一上下文同词面」的旧记录，再删目标槽位的旧主人，然后插入新身份并重建显示投影。时间戳在删除之前按原身份读取，移动时仍沿用原来的 Lamport 时间。这样每个上下文里一个词面最多只有一条固定记录。但重排后显示行的编码也可能从简码 `aa` 换成全码 `aad`，`clear` 若按当前行的 `entry_key` 删就仍然落空；因此取消固定只按上下文和词面删记录，不再比对 `entry_key`。

## Alternatives considered

- **去掉 `refresh` 里 `previous_highlight > 0` 的守卫**，让所有候选变化都按文字找回高亮。最强的理由是改动最小，但它会破坏既有契约：云词插进第二格时空格仍要选新首选，静止重排也不能把已翻开的页拉回去；那三条行为测试会失败。因此只在维护动作之后做例外。
- **让引擎在固定结果里返回新座位，经 host-api 传给运行时**。定位最精确，但要跨 engine → host-api → runtime 扩协议，六端都要跟着改；共享运行时已经能从快照按槽位找回，不需要新字段。
- **固定后用文字加编码找高亮行**，与异步刷新共用谓词。实现更统一，但同一行的来源换掉时编码会变（`aa` 变成 `aad`），高亮仍留在原座位。
- **让固定记录始终用同一个派生的键**，例如强制取原始输入或 canonical 拼音。能避免双行，但五笔原生行没有 canonical 拼音，且会改动完整同步状态的身份规则；与槽位冲突时保留输家的设计相冲突。按词面迁移身份是更小的改动。
- **只让取消按词面删记录，不动固定时的身份收敛**。装机后的 ctypes 探针证明这不够：固定 `aa` 的「式」后，显示行的编码变成 `aad`，虽然取消能删掉记录，完整表和显示投影里仍会留下重排前那条 `aa` 身份。两处都按词面收敛才是完整修复。

## Verification

先写测试看红再实现：

- `position_state/tests.rs` 的 `moving_a_word_to_another_slot_leaves_one_fixed_record_and_cancel_removes_it`：红时为 `[("aa","甲",1),("aad","甲",3)]`，实现后只剩一条且取消清空。
- `input-runtime/src/tests.rs` 的 `fixing_a_candidate_to_a_later_slot_keeps_it_highlighted_and_cancel_acts_on_it`：用真实引擎和合成词库逐槽位 2..=5 固定、断言高亮跟到目标行、再对新高亮行取消；红时高亮断言失败，实现后四轮全过。
- `session/tests.rs` 的 `canceling_a_fixed_wubi_row_still_works_when_its_code_changes_after_the_move`：同一词面在简码和全码各有一行，固定后显示行的编码变成全码，再按显示行取消；红时 `fixed_candidate_positions` 残留 1 条，改为按词面删除后清零。这条回归的 fixture 随本 PR 一起加进会话测试，不依赖五笔去重 PR。

追加提交在 #11841 head 上跑 `MSIME_OPENCC_OFFLINE=1 cargo test --locked -p msime-engine -p msime-input-runtime`：`msime-engine` 3266 通过、59 忽略，`golden` 31、`scheme_data_sets` 3、`session_threads` 3，`msime-input-runtime` 487 通过；两个 crate 的 `--all-targets` Clippy `-D warnings` 与全仓 rustfmt 通过。重新编了 Release 宿主库和完整 `.deb`（`pkexec dpkg -i` 装好，包内与装机库 SHA-256 一致 `5e09d804e870facd07da804c82e634ec677a9f432282ce2468f962fc72814eea`），`ibus-engine-smoke <已验证词库目录>` 退出 0。`ctest --test-dir target/linux-full-pack` 122/124，两条失败仍是 `linux-candidate-native-canvas` 与 `linux-candidate-native-canvas-chrome` 的像素断言（`candidate_native_canvas.cpp:776`、`candidate_native_canvas_chrome.cpp:216`）；同一断言、同一行号在不含本次改动的 `linux-full-verify` 构建树上复现，仓库既有笔记也把它们记为本机字体环境债务，未改产品代码或断言去绕开。

本机图形会话真机验收（终端、浏览器、文本编辑器等）：修前固定到 3–5 实际落到第 2 位；修后可以正确固定到各位置行，固定候选词、取消固定、删除自造词、置顶候选词均正常。装机库的 ctypes 临时状态探针另行证明固定表与状态表在取消后清空；自动化测试逐槽位 2..=5 覆盖位置、高亮和取消，Linux 键盘快捷键路径由上述真机验收覆盖。对比图未采集。本修复作为追加提交推送到 #11841。

## Consequences

固定到 2..5 后高亮跟着被操作的行，`Ctrl+0` 与右键取消作用于同一条；同一个词在同一个上下文里只留一条固定记录，重排后显示行的编码不再决定能否取消。其他平台共用运行时，固定菜单按点击的 `CandidateId` 提交动作，不依赖 `highlighted`，因此除高亮位置更正确外没有行为或 ABI 变化。

代价：本机把同一个词移到新槽位时会删除同上下文同词面的其他身份，包括同步冲突留下的隐藏输家；它们不再因为当前胜者被取消而重新显示。这里选择用户可见的词面身份优先于那类隐藏冲突状态。异步刷新时高亮仍留在原座位的既有规则不变，只有显式的固定/取消例外。

## 历史笔记检索

按 `固定位置`、`fixed_candidate_position_state`、`highlighted`、`refresh` 检索活跃笔记。持有「异步改候选时高亮放哪」的是[异步刷新保留候选页](2026-10-20-async-refresh-keeps-candidate-page.md)，本改动只在维护动作上加例外，不改它的结论。持有完整候选状态规则的是[学习同步提案](../../proposed/feature/2026-10-27-learning-sync-e2e.md)，本改动收窄了同词面身份的保留范围，代价已写在上文。没有可就地更新的同题笔记。
