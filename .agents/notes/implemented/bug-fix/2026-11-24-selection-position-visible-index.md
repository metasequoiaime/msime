# Agent Note: 选词位置统计改取派发事实里用户看到的编号

Status: implemented

## Problem

#11776：Windows 打字统计页「候选命中位置」里「第 1 条」长期为 0、首选命中率被算成 0%，云候选开启时后面的桶整体后移一位。

两个独立的原因都在 `crates/host-api/src/lib.rs` 的 `dispatch_value` 里：

- 位置只由 `selected_position()` 从动作里算出，而它只认 `Action::Select`（点选、数字键、二三候选）和 `Action::SelectAnyCandidate`。空格、回车、标点、五笔顶字和结束组字都经命令路由（空格是命令 1 → `Action::SelectHighlighted`），动作里根本没有候选身份，于是这些选词一条都不计。空格恰恰是首选命中最高频的路由。
- 计的是引擎座位加一。云候选开启时引擎在下标 1 放一个 `CandidateSource::CloudPending`（文字是「☁」）占位席位，宿主不画它、也不给它编号，选它后面一行时用户看到的是第 2 条，统计记成了第 3 条。

## Decision

- `SelectionFact`（`crates/input-runtime/src/types.rs`）新增 `visible_position: Option<u32>`：用户看到的 1 基编号。`seat` 保持原样，它是脚本 ABI 的输入（`pack-tool` 的 `CandidateSelection` 夹具），改语义会牵动插件。
- `Runtime::selection_fact` 里按派发前的快照（`self.cached.candidate_sources`）算出它：数 `seat` 之前有几行不是占位席位，再加一；`seat` 自己就是占位席位时为 `None`（占位不是候选，宿主也没有给它的编号）。
- 占位席位的判定放在引擎里（`CandidateSource::is_placeholder_seat`，来源 12 的 AI 占位加来源 15 的云占位）。来源 12 已退役、引擎不再产生，留着是为了旧版本留下的行仍按占位对待，与 Windows 的 `candidate_source_is_visible`、macOS 的 `MSIMEVisibleCandidates` 是同一条规则。
- `dispatch_value` 不再从动作算位置，改成上屏之后读 `facts.selection.and_then(|selection| selection.visible_position)`。`facts()` 提前到统计之前取，`lifecycle_after` 用的仍是同一份，中间没有改运行时状态的操作。
- 效率（`efficiency_candidate`）只保留原来的 `Select`/`SelectAnyCandidate` 两条路：它要读被选中那一行的码、文本和来源，命令路由在派发之前拿不到那条候选。这条缺口与位置独立，本次不动。

## Alternatives considered

- **让每个宿主自己把可见编号塞回 `Action`**：六个平台各改一处，漏一个就少一类选词，而且 Linux 与 macOS 的可见集合本来就不同（macOS 也藏 12），等于把同一份规则复制六遍。
- **只把 `selected_position` 扩成认 `SelectHighlighted`**：修得了空格这一例，回车、标点、五笔顶字和结束组字照样漏；而且仍按引擎座位算，云占位席位的偏移还在。
- **改 `seat` 的语义为可见编号**：`seat` 已经作为脚本输入的一部分发布（`pack-tool` 的夹具和 `CandidateSelected` 事件），改语义会让现成的插件按错行。

## Consequences

- 空格、回车、标点、顶字、结束组字的选词都进「候选命中位置」；第 1 条会开始有值，首选命中率按真实数据算。
- 云候选开启时不再因隐藏的「☁」整体偏一位。
- 移动端、macOS、Linux 同一条计数路径一起受益：它们也经 `dispatch_value`。
- `SelectionFact` 多一个字段，`pack-tool` 的夹具只按自己的字段表解析 `seat`，不受影响。
- 未改「Windows 上只在 TIP 会话完成、Server 不经手的选词不记位次」那条（见 `2026-10-29-typing-statistics-selection-directory.md` 的附注），是另一处偏少。

## Verification

- `cargo test -p msime-input-runtime --lib facts`：包含新增的 `placeholder_seats_do_not_shift_the_visible_position`，覆盖「云占位在第 2 格时选第 3 格记作第 2 条」和「占位席位本身不记位次」。
- `cargo test -p msime-host-api --lib`：包含新增的 `space_selection_counts_the_first_position`（三次空格选首选 → 桶 1 有 3 次）和 `a_hidden_cloud_placeholder_does_not_shift_the_counted_position`（真实引擎下开启云候选、`nihao` 第 2 格是「☁」，选第 3 格记入桶 2）。后者在按引擎座位计数的旧口径下失败（实测 `left: 0`），确认它守的是这次修的行为。
- 未验证：Windows 11 上打字统计页的实际分布（需要真机 Server + TIP 会话），以及云候选真实回包到达后的位次。
