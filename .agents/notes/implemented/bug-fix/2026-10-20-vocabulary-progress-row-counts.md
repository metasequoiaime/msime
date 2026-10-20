# Agent Note: 背单词进度行的「今日待复习」与「未开始」

Status: implemented

## Problem

六端的背单词页都直接显示 `ReviewStatus` 的三个数：`due` 标为「今日待复习」，`answeredToday` 标为「已完成」，`remaining` 标为「未开始」。宿主没有自己的算法，所以两个数的含义都由 `crates/client-services` 决定。

在 Pixel 3 XL（Android 12）上装 0.5.0 验收时，选 CET-4（3802 词）后，进度行显示 0 / 0 / 3782，屏幕上却已经有一张 abandon。对 abandon 答「认识」、对 ability 答「不认识」之后，进度行变成 1 / 2 / 3783，「未开始」反而多了一个。

原因有两处：

- `build_queue` 的 `remaining` 定义是「既不到期、今天也不新出的词」，把排到以后某天的已学卡也算了进去。abandon 答完被排到以后，就从今天的新词里挪到了「未开始」。
- `due` 定义成「不含新卡额度的积压」，今天要出的 `introducing` 张新卡三个数里都不出现。刚选完词书时，正好是「今日待复习 0」配着一张卡。

## Decision

- `ReviewQueue.remaining` 只数从没出过、今天也不会出的词，即 `fresh.len() - introducing`。排到以后某天的卡已经学过，不再算进任何一个数。
- `ReviewStatus.due`（今日待复习）等于 `ReviewQueue.due + introducing`，也就是今天还剩的卡，包括今天会话要出的新卡。超出会话上限的积压照旧全数计入，所以这个数可以大于 `queue.len()`。
- `ReviewQueue.due` 保持只算积压。排序和「积压全量上报」都靠它，含义不变。
- `introducing` 仍在 `ReviewStatus` 里，表示 `due` 中的新卡数，iOS、Android 和桌面的类型都还在读它。

六端宿主不用改代码：Android、iOS、Windows、桌面共享页（macOS、Linux、HarmonyOS 走同一个会话）都只显示 JSON 里的数。改的只是 Rust 端和桌面类型上的字段注释。

改后同一个场景：选完 CET-4 是 20 / 0 / 3782；答完上面两张是 19 / 2 / 3782。「未开始」在当天的新词出完之前不变，第二天再减去新的一批。

## Alternatives considered

- 只改 `remaining`，`due` 保留「不含新卡」：这最贴近原设计，Anki 也把新卡、学习中、到期分开显示。没采用，是因为这一行的标题是「今日」，界面又只有三个格子，没有地方单独显示新卡；「今日待复习 0」配一张正在显示的卡，在真机上就是读不通的。
- 在 `ReviewStatus` 里加一个新字段，让各端界面去加 `due + introducing`：这样旧字段的含义一个都不变。没采用，因为要改六端的解析和显示，Windows 现在连 `introducing` 都不解析。同一个加法写六遍，正是这次两个数各端一起错的那种结构。
- 把「未开始」改成别的标签（比如「其余」），沿用旧的 `remaining`：这是改字最少的办法。没采用，因为旧值把「学过、还没到期」和「从没学过」混在一起，换什么标签都说不清，而且答一题会让它变大。

## Consequences

- 收益：三个数前后一致。今日待复习 + 已学未到期 + 未开始 = 词书总数，答题只会让「今日待复习」变小、「已完成」变大。
- 代价：`ReviewStatus.due` 与 `ReviewQueue.due` 同名不同义，前者含新卡，两处的文档注释都写明了。宿主若曾把 `status.due == 0` 当作「今天没有积压」，现在新卡也会让它非零；目前六端都只拿它显示。
- 学过、还没到期的词现在三个数里都看不到。要显示「已学」得另加一格，这次不加。

## Verification

`progress.rs` 新增「答题不会让未开始变多」的用例，`session.rs` 新增进度行三个数随导入、答「认识」、答「不认识」变化的用例；`cards_not_yet_due_stay_out_of_the_queue` 和次日用例改为新含义。修复前这 4 个用例失败，修复后 `cargo test -p msime-client-services vocabulary` 106 个全部通过，`cargo fmt --check`、`cargo clippy --all-targets -D warnings` 通过（经 rbuild 在 Mac Studio 上运行）。宿主端的测试只用手写 JSON 验解析，不受影响。改后的包还没有装到真机上看过。

同一轮验收顺手把 `scripts/fetch_wordbooks.py` 写出的 NOTICE 开头从 "The wordbooks in this directory" 改成 "The bundled wordbooks"：这份文本也在 App 的关于页里显示，那里没有「这个目录」。打包方式见 [内置词书打包](2026-10-20-bundled-wordbooks-packaging.md)。
