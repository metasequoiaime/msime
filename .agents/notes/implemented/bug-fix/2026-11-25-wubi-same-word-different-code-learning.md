# Agent Note: 五笔同词不同码只抑制自学习加权，不去重原始词库

Status: implemented

## Problem

用户在本机图形会话（终端、浏览器、文本编辑器）报告：修前输入 `aa` 出现两个「式」，`aad` 那行被自学习提前；修后输入 `aa` 和 `aad` 都只显示一个「式」（码 `aad`）。工作库里 `aa`→式(20)、`aad`→式(11) 都是打包词库原有的行；日志里另有 `('wubi','aad','式','upsert',530,user_inserted=0)`，把长码那行抬到简码同一屏。固定排位修复前后的复现一致，这条重复不是另一条 PR 附带的笔记 `2026-11-25-fixed-position-cancel-seat-and-identity.md` 里记录的固定位置与身份收窄引入的。

golden 场景 `wubi_prefix_codes` 明确记录参考 C++ 本来做词面去重，产品 overlay 刻意去掉去重，`wq`/`wqi` 两个「你」是正常的。因此不能按词面隐藏候选，也不能把「五笔同词只显示一次」当成通用规则；引擎层合成词库里的两行原始候选照常保留，用户可见的单个「式」是本机词库与界面呈现的结果。

## Decision

原始词库自带的同字不同码照常显示；自学习遇到「同一个词已经在本工作词的另一个编码上存在」时，不写这条调权日志，也不自动调频。用户显式置顶不走这道抑制，因为它不是自动学习。自造词写入本来就有按词面的 `SELECT EXISTS` 去重，不会重复插入。

已有的历史重复学习记录在每次创建会话时回退：扫描日志中的五笔 `upsert && user_inserted=0` 行，只对打包词库表里存在原值、当前日志权重高于原值、且词在另一个编码上也存在的行，把日志和工作词库的权重都写回打包原值，并用 `learning_clock::ranking_timestamp` 推进逻辑时间。用户自己加入的 `user_inserted=1` 行不回退；打包词库里没有原值的行不回退。日志保留一行低权重 `upsert` 记录，学习同步到远端后仍收敛到相同权重，不写删除墓碑。

判断只落在五笔原生候选的自动调频入口 `session/learning.rs::adjust_candidate_frequency`，不改 `WubiProvider` 的前缀查询，也不改全拼/双拼的学习规则。历史回退放在 `user_dictionary/wubi_duplicate_learning.rs`，由 `session/input.rs::Session::new` 在启用五笔时调用。`aa` 与 `aad` 两码都不再自动调频，显式置顶仍生效；如果用户把另一码那行彻底删掉，抑制条件自然消失，该编码重新参与调频。

## Alternatives considered

- **在显示层按词面隐藏重复行，并清理被隐藏行的学习加权。** 最直接，能让 `aa` 只剩一个「式」，但会让 golden 的 `wq`/`wqi` 从期望的四个候选变成三个，等于翻转场景备注里记录的产品决策、影响所有共用引擎的平台。用户明确澄清「原始词库不需要去重、不隐藏，只取消提前」后不走这条路。
- **物理删除重复的学习日志行。** 看起来最干净，但[学习同步](../../proposed/feature/2026-10-27-learning-sync-e2e.md)会从远端把 `user_inserted=0` 的日志同步回来，删完还会复活。
- **写 `operation='delete'` 墓碑。** 同步语义是整行删除，打包词库原有的那条候选也会被一起删掉；用户要的是取消提前，不是删词。
- **自造词表按词面去重。** 查过 `user_dictionary/wubi_learning.rs::learn()`，自造词写入前本来就有 `SELECT EXISTS(... WHERE value=?1)` 检查，不会在另一个码上重复造同一个词；这里没有可修的缺陷。
- **一律不学习单字。** 能盖住「式」，但会改掉六端现行规则和 golden 回放，而且清不掉已经存在的历史加权；按用户确认的范围不采用。

## Consequences

收益：`aa` 两行原始候选在引擎层照常出现，但自学习不再把另一码上已有的词提前；历史加权已回退到打包原值，本机真机输入 `aa` 和 `aad` 都只显示一个「式」（码 `aad`）。词组按词面同样处理，不区分单字与多字词。

代价：每次启用五笔的会话创建都会只读扫描日志，并可能打开日志与工作词库各写一次事务；用户自己加入的重复词和打包词库原有的重复候选在引擎层都仍是两条。显式置顶可以为同一词制造第二个槽位，这是刻意保留的用户操作，不受自动学习抑制影响。

## Verification

先加红灯再落实现：四条会话回归在 `crates/engine/src/session/tests.rs`：

- `a_learned_long_wubi_code_is_reset_without_hiding_the_original_row`：历史加权 530 回退到 11，合成词库页面上仍保留两个「式」（原始候选不在显示层隐藏）。
- `selecting_a_wubi_word_present_at_another_code_records_no_learning`：打开 Promote 选择第二个「式」，日志零条。
- `selecting_a_wubi_phrase_present_at_another_code_records_no_learning`：词组「工工」同样不记。
- `a_user_inserted_duplicate_wubi_row_keeps_its_weight`：`user_inserted=1` 的 530 原样保留。

在最新 develop 上跑 `MSIME_OPENCC_OFFLINE=1 cargo test --locked -p msime-engine -p msime-input-runtime`：`msime-engine` 3304 通过、59 忽略，`golden` 31 通过，`scheme_data_sets` 3 通过，`session_threads` 3 通过，`msime-input-runtime` 488 通过；此前显示去重方案会让 `wubi_prefix_codes` 红，收窄后 golden 保持原期望全绿。全仓 `cargo fmt --all -- --check` 与两个 crate 的 Clippy `-D warnings` 通过后补记。

本机图形会话真机验收（终端、浏览器、文本编辑器等）：修前输入 `aa` 出现两个「式」、`aad` 那行被自学习提前；修后输入 `aa` 和 `aad` 都只显示一个「式」（码 `aad`）。新学习的自造词如有重复，不再写调权日志、不再自动调频（显式置顶仍生效）；自造词本身按词面去重，不重复插入；旧历史加权回退到打包词库原值。改动已编进本机完整 `.deb` 并安装为系统输入法后验收，对比图未采集。

## 历史笔记检索

按 `同字不同码`、`同词不同码`、`五笔.*去重`、`打包词库原值` 检索活跃笔记；命中的都是查码/回退的局部优化，没有同题决定。与另一条 PR 附带的笔记 `2026-11-25-fixed-position-cancel-seat-and-identity.md` 是同一份用户报告里的第二个根因，互不改写结论。
