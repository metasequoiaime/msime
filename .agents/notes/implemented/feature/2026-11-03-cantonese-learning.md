# Agent Note: 粤语选词学习：调频与造词

Status: implemented

## Problem

#10544 报告粤语（含粤语双拼）没有「记忆」：常选的词不往前挪，分几次选出来的词下次还得再分几次选。第一期（[粤语个人词条](2026-11-02-cantonese-personal-words.md)）只让用户手动加粤语词，粤语上屏仍然不写任何东西，调频和造词留给维护者另行决定，因为它会改变现有粤语用户的候选顺序。

维护者 2026-11-03 拍板：粤语像拼音一样学习，调频和自动造词都做；不新增开关，沿用拼音已有的学习开关和它们现有的值，开着这些开关的现有用户的粤语候选顺序会开始随使用变化，这一点已授权。云同步、社区词库和快照上传不在这次范围里，粤语学到的东西不能流进这几条路。

## Decision

开关与拼音完全相同，没有新增偏好键，也没有改任何默认值：

- 偏好 `learning`（各端的「学习选词习惯」「学习常用词」，引擎 `SessionOptions::learning`）关着时粤语什么也不写。隐私会话不需要单独处理：Android（隐私模式和不许个性化学习的输入框）、鸿蒙和 iOS 的隐私模式都是以 `learning=false` 建会话的，与拼音同一条路。
- 偏好 `frequency.mode`（「调频方式」，引擎 `FrequencyAdjustmentOptions`）为「关闭」时不调频，造词照常，与拼音相同；触发频次、线性步长按拼音的规则读。

调频（`session/cantonese_learning.rs` 的 `learn_cantonese_frequency`、`cantonese/learning.rs` 的 `adjust_frequency`）：

- 选中的不是首选、是粤语词库里的行时才调。比较的那组是与它覆盖同一段输入（同一个 `WordItem::pinyin`）的粤语行：整段的词一组，只覆盖前几个音节的词各一组，与粤语候选「先按覆盖、再按权重」的排法对应。组内按权重比名次，与拼音调频按权重不按显示顺序比的理由相同。
- 计数记在 `candidate_selection_state`，上下文是 `cantonese:` 加粤拼键（`SELECTION_CONTEXT_PREFIX`），够了触发频次后按 `ranking_target` 算目标名次，把选中词的权重写成目标名次那一行的权重加一（上限 `MANAGED_WEIGHT_CEILING`），写成用户日志里 `dictionary='cantonese'` 的 upsert。新行 `user_inserted=0`，不进「我的词条」；已经是用户词的行调频后仍是 `user_inserted=1`。写法是 `cantonese/learning.rs` 自己的 `LEARNED_WEIGHT_SQL`，不用拼音的 `UPSERT_JOURNAL_SQL`：用户删掉个人词时墓碑带着 `user_inserted=1`（`personal.rs` 的删除语句），拼音那条语句冲突时不动这一列，墓碑翻回 upsert 就又是用户词。拼音删词时回放会连同名的发布行一起删，删掉的词不再是候选，碰不到这种情况；粤语没有工作表，删掉的个人词若 `msime-cantonese.db` 里也有，候选里照样有它，再选它调频时就会让删掉的词回到「我的词条」。所以冲突的那一行是墓碑时把 `user_inserted` 清成 0，学到的权重照常生效，词不回来（`a_deleted_cantonese_word_stays_deleted_when_it_is_picked_again`）。
- 发布的 `msime-cantonese.db` 只读，学到的权重只在日志里。`cantonese::UserWords` 改为读日志里全部粤语 upsert 行（以前只读 `user_inserted=1`），`CantoneseLexicon` 合并同键同字时本来就取较大的权重，所以调频写下的权重直接生效，活着的会话下一次查询就看得到。

造词（`note_cantonese_piece`、`create_word`）：

- 粤语选一个候选就立刻上屏、剩下的接着组字，没有拼音的 `phrase_progress`，会话另记一份 `CantonesePhraseProgress`：组字没结束时记下已选的每一段，`reset_composition` 清掉它，所以取消、原样上屏、清空都不造词。
- 组字因为选完候选而结束时：选了两段以上，或一次选中的是组出来的整句行，就把整段（粤拼键以空格连接，文字相接）在粤语词库里还没有时记成一个词，`user_inserted=1`、权重 `LEARNED_WORD_WEIGHT`（等于拼音造词的 `INSERTED_WEIGHT`，10000）。整段要过个人词库的粤语校验（音节数等于字数），2 到 7 个音节（7 同拼音记整句的上限）；用户删过的词（墓碑）不再造回来，同拼音的选词对。
- 整句行靠 `CantoneseCandidate::composed`（`compose_sentence` 置真）识别，会话经 `ImeSession::cantonese_row_is_composed` 在拿走这一段之前问最近一次查询的行。整句行不调频。

不进同步、社区和快照：

- 云同步（`WordKind`）、WebDAV 同步（`dictionary_webdav::word_kind`）、社区词库和快照导出按词库种类挑行，本来就不收 `cantonese`，造的词和调频行都过不去。
- 粤语的选词计数是新多出来的一类记录：本地备份快照（`include_learning`）和云词库「应用到本机」带本机学习时都经 `learning_records` 读 `candidate_selection_state`，这里按 `cantonese:` 前缀跳过它们，计入 `learning_skipped`。
- 整份换上快照时 `carry_cantonese_words` 把旧一代里没删的粤语行全部抄进新的一代，`user_inserted` 照原样（`add_cantonese_words` 不再只收用户词）；粤语的选词计数不抄，从头再计。

宿主不用改：各端的学习开关本来就是会话的 `learning` 和 `frequency`，所有设置页文案都没有写「粤语不学习」。iOS `InputSchemePreference.swift` 里说粤语 nothing learned 的源码注释顺手改正。

设置页的文字这次不改：调频组的标题仍是「拼音方案调频」（共享设置 `frequency-section.tsx`、Windows WinUI 与 Win7），iOS「学习常用词」的说明仍是「学习支持的拼音组词」，它们现在少说了粤语。这个组名照参考产品的分组来（`docs/windows-parity.md` 的对照记录、桌面设置测试都按它核对），改名要六端设置页、对照文档和测试一起动，还要附改前改后截图；维护者这次拍板的是「沿用现有开关、不加新开关」，没有涉及改名。是否改成「拼音与粤语调频」一类的说法留给维护者决定，PR 里单独提出。

## Alternatives considered

- **新增「粤语记忆」开关、默认关**：不动任何现有用户的候选顺序。维护者明确选了沿用拼音开关并授权顺序变化，新开关还要六端设置页、云同步的偏好白名单一起加。
- **调频时把整张候选表当一组比**：与拼音按整个列表比更像。但粤语候选先按覆盖的音节数分组、组内才按权重排，只覆盖第一个音节的「妳」再重也排不到整段的「你好」前面；拿整段的权重来算会把单字的权重抬到几十万，下次在别的读音里乱跳。
- **造词用手动加词的默认权重 100000**：造出来的词马上排第一。但它是从几次选择里推出来的，不是用户明说要的；用拼音造词的 10000 落在同一读音的常用词后面、首页之内，再选它由调频往前挪，与拼音的节奏一致。
- **选中时用 SQL 查词库来分辨整句行**：不用改 `CantoneseCandidate`。但实测选中时查一次词库就多一次分配（rusqlite 的 `prepare_cached` 每次都给语句重设一个 `Arc<str>` 缓存键），选首选整段上屏这条最常见的路就会超出 `selecting_a_cantonese_candidate_does_not_clone_the_full_row` 守着的分配上限；整句标记由出候选的地方顺手给出，选中时只扫已有的行。
- **粤语计数不进 `candidate_selection_state`，只在内存里计**：不用碰快照导出。但触发频次是跨会话的（选三次才挪），移动端键盘进程随时被杀，内存计数等于把「触发频次」静默改成了「一次会话内的次数」。

## Consequences

- 收益：六个平台共用引擎，粤拼和粤语双拼都按用户已有的学习设置调频、造词；不新增偏好、不改默认值；首选整段上屏这条路仍然不分配、不碰日志。
- 代价：开着「学习」且调频方式不是「关闭」的现有粤语用户，候选顺序开始随使用变化（已授权）。日志里粤语行变多，`UserWords` 在日志变过之后整份重读的开销跟着涨，造词有 7 音节上限、调频只写选中的那一行，增长与拼音学习同量级；iOS 键盘扩展的内存上限下还没量化过。认得 `cantonese` 之前的引擎（降级安装）回放日志会失败，这一点第一期已经记下，学习只是让有粤语行的日志更常见。
- 仍不做：候选窗对粤语行开放置顶和删除（`is_editable_source` 不变）；粤语的个人上下文（n-gram）和选词对；云同步、社区词库、快照收粤语，等服务端协议一起改时另开 issue。
- 关联笔记：[粤语个人词条](2026-11-02-cantonese-personal-words.md) 的「粤语上屏仍然不写任何东西」由本篇取代；[粤语提交只复制候选词文本](../testing/2026-10-10-cantonese-commit-borrow.md) 的分配上限仍然成立，本篇为守住它把整句识别放在候选标记上。

## Verification

- `cargo test -p msime-engine --lib cantonese`：`a_cantonese_candidate_picked_again_and_again_rises`（逐次前移、新会话保持、分段只和同段比）、`a_deleted_cantonese_word_stays_deleted_when_it_is_picked_again`（删掉的个人词再被调频不回到「我的词条」）、`cantonese_pieces_picked_in_one_composition_become_a_word`（调频关着也造词、取消不造词、不重复造）、`a_composed_cantonese_sentence_is_learned_as_a_word`（学习关着不记）、`cantonese_rows_are_not_edited_and_learn_nothing_with_learning_off`、`selecting_a_cantonese_candidate_does_not_clone_the_full_row`（分配上限不变），以及 `cantonese::learning` 的单元测试。
- `cargo test -p msime-engine --lib user_dictionary::state`（`cantonese_words_are_added_to_a_generation_and_existing_rows_are_kept` 带上学到的行）。
- `cargo test -p msime-host-api --lib dictionary_snapshot`：`a_learning_export_leaves_out_cantonese_learning`、`a_snapshot_activation_keeps_the_personal_cantonese_words`（学到的行带过去、粤语计数不带）。
- `cargo test -p msime-engine --test golden`：`yue_partial_commit_no_learning` 改成学习关着的场景，期望文件不变。
- 各平台键盘没有在真机上验收。
