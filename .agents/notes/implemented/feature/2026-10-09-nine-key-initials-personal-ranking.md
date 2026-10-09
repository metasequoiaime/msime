# Agent Note: 九宫格首字母分词输入把用户用过的词排到前面

Status: implemented

## Problem

#6185（#5640 的后续）：安卓九宫格用「分词」键按首字母出词时，用户自己打过的词既不靠前，有时还查不到。出货词库上：`4'4'5'5` 的 关关雎鸠 排第 27，要展开才看得到；`9'7` 的 隐私 前面有 284 行更重的，`9'9'2'9` 的 仔细查找 权重 100，前面有 246 行更重的、还有三百多行同样是 100，两者都被简拼查询「按权重取前 64 行」截掉，展开后也没有。

整条简拼路径没有任何用户使用信号：词库里已有的词再打一次不写任何东西；九键的调频只在选非首位时触发，上下文还绑在具体的数字串上（`nine-key:<digits>`），用全拼读音的数字或 26 键打过的词对简拼列表没有影响；个人上下文模型只参与整句词网格。

## Decision

- **信号**：26 键早已把选中的词记进个人上下文模型（`PersonalNgramStore`，显式选词每次记 `DICTIONARY_PICK_TIMES` = 2 次）。九键选词时也记：选中的多字汉字词库词（选首位也记），以及这次存进词库的词（分段连成的词组、选中的整句），前一个词当作句首，次数与 26 键相同（`PERSONAL_PICK_TIMES`，`session/tests.rs` 核对两者一致）。开关与 26 键相同：记录和读取都要 `learning` 和 `personal_context`（对应 26 键的 `personal_context_applies`；关掉个人上下文后，模型里早先记下的使用也不再读），`Session::set_personal_context_enabled` 同步给九键。新增的访问器：只读的 `PersonalNgram::word_count` 和 `QuanpinDictionary::personal_model`（一次简拼查询取一次读锁），以及只开放记录这一个写操作的 `QuanpinDictionary::record_personal_use`，不把整个 `PersonalNgramStore` 交出去。
- **查得到**：学习打开、个人模型有记录时，简拼查询改用 `query_jianpin_codes_per_table(codes, INITIALS_SCAN_LIMIT = 512)`：每张首字母表各取权重最高的 512 行，合起来不再截断；按个人模型给有计数的行做标记，稳定地把它们排到前面，再截到 `INITIALS_ROW_LIMIT`（64）。不合起来截断，是因为同权重的行太多，合起来截时排在后面的首字母表会被整批截掉（仔细查找 就在 z 表里）。
- **排位沿用调频设置**：调频关掉时只保证查得到，按权重排；打开时，每个用过的简拼行按计数算触发次数（每 `PERSONAL_PICK_TIMES * trigger_count` 个计数一次），从它在整段里的位置起，每次触发按调频模式算一次新位置（`ranking::ranking_target`，与 26 键调频同一个函数：置顶、减半、按步长、提到第五位以内再逐位上移）。默认的「提到前五、触发一次」下，用过一次就排第五，再用一次排第四。
- **只由一处挪位**：在简拼列表里选中一个会记进个人模型的简拼行时，不再走九键原有的调频（`adjust_frequency` 改的是这个词的全局权重）。两边都挪的话，一次选词按调频设置挪了两次：全局权重先把它抬到更前的位置，个人计数又从那里再挪；全局权重还会连带改掉 26 键 `ys` / `yinsi` 下它的位置。没开个人上下文时（不记进模型），选简拼行照旧走原有调频。
- **边界**：只在覆盖全部数字的那段词典行里挪（简拼行都在这一段）。没打切分时挪不过前 `SYLLABLE_ROWS_BEFORE_INITIALS` 个音节行，保留 #5640 承诺的「首字母词排在几个最常用单字后面」；打了切分时可以挪到最前。几个用过的词抢同一位置时，原来靠前（权重高）的先占。挪位在去重之后、截到 `CANDIDATE_LIMIT` 之前做，不改变保留哪些行，`push_ranked` 的跳过规则照样成立。只作用于九宫格，26 键简拼顺序不变。

## Alternatives considered

- **选简拼行时照旧调全局词频，个人计数只负责查得到**：改动最小。但全局权重是按九键简拼列表里那些重行算的，会把低权重的词在 26 键里也提上去，违背「不改 26 键简拼顺序」；而且关掉个人上下文后用过的词就又查不到了。由个人计数单独决定挪位，九键和 26 键各自的调频互不影响。
- **用过一次就置顶，不管调频设置**：最直接满足「打过的词排在前面」。但关掉调频的用户明确不要候选随使用变动，置顶也会让偶尔选错一次的词长期霸占首位；产品取舍是沿用调频的开关、触发次数和模式。
- **给 `RankKey` 加一个「用过」字段，用排序键表达提升**（分诊方案）：排序和 `push_ranked` 的跳过规则自然一致。但排序键只能表达「用过的排在没用过的前面」这种全序，表达不了调频模式要求的「按次数挪到第几位」；而且没打切分时还要另外保证常用单字在前。挪位放在去重之后做，跳过规则不受影响，规则也更容易对照 26 键的调频。
- **合起来按权重取前 512 行再找用过的词**（分诊方案）：实现最小。实测出货词库里 仔细查找 和三百多行同是权重 100 的词并列，合起来截断时 z 表排在最后，它照样被截掉；每张表各取 512 行读的行数没有变多（原来每张表的 SQL 上限就是 512），只是不再合起来截。
- **不设上限扫出全部简拼行**：查得到任何用过的词。但 `9'7` 在出货词库里有六千多行，每按一键都要读一遍；个人模型只存词的散列，没法反过来列出用户用过哪些词再按词去查。
- **只在造新词时记**：报告里的 隐私、关关雎鸠、仔细查找 都是出货词库原有的词，只在造词时记，修完仍然无效。

## Consequences

- **收益**：出货词库上（Mac Studio 上的临时探针，未入库），26 键各打一次 隐私、关关雎鸠、仔细查找 之后，九宫格 `9'7` / `97`、`4'4'5'5` / `4455`、`9'9'2'9` / `9929` 都把它们排在第五位；之前分别是查不到、第 27 位、查不到。
- **代价**：个人模型有记录时，两位以上数字的简拼每一键多读一些行（每张首字母表最多 512 行，原来是 64 行）并逐行查一次个人模型的散列（整次查询只取一次读锁）。探针测得的进程 CPU 时间（机器同时跑着 CI，只作量级参考）：`9'7` 三键合计 1.4 ms → 3.4 ms，`9'9'2'9` 七键合计 6.2 ms → 11.4 ms，大约每次简拼查询多 1.5–2 ms；移动设备上没有测过，如果低端机上按键变卡，先回来看这里。
- 每张首字母表里比用过的词更重（或同重而排在前面）的行超过 512 行时，它仍然查不到；出货词库里 `9'7` 的 y 表有两千行，权重 10000（新造词的初始权重）以上的就有六百行，新造的两字词落在这种组合里时还会被截掉。
- 个人模型有记录的用户，同权重、没用过的行在截断处留下哪几行由每张表 `LIMIT 512` 的 SQL 结果决定，和模型为空时 `LIMIT 64` 的取舍可能不同；SQL 本来就不保证同权重行的先后，两种情况下这些行在权重上都是并列的。
- 九键选中的多字词现在会进个人上下文模型，整句词网格（26 键和九键）读到的句首计数随之变化，与 26 键在句首选词的效果相同。「清除个人数据」和 `delete_personal_ngram_word` 走的是同一个模型和日志表，对这些记录同样生效。
- 已检索的相关笔记：[九宫格按首字母出词](2026-10-08-nine-key-initials.md) 是本篇扩展的对象，取行数量的事实已同步更新；[九宫格候选去重](../testing/2026-10-08-nine-key-candidate-dedup.md) 讲 `push_ranked` 的跳过规则，挪位在去重之后，结论不变。

## Verification

- `cargo test -p msime-engine`：`nine_key::tests::used_initials_words_survive_the_row_limit_and_move_up`（截在 64 行之外的词用全拼读音选一次后排第五、再用一次排第四，不打切分时同样）、`a_used_word_tied_with_a_crowded_table_is_kept`、`used_initials_words_follow_the_frequency_mode_and_trigger_count`（置顶、减半、按步长、触发次数 2）、`used_initials_words_only_move_with_frequency_adjustment`（调频关掉只保证查得到，学习关掉不读个人数据）、`picking_a_used_initials_word_moves_it_once`（在简拼列表里选中挪上来的词只挪一次：第五位选一次进第四位）、`earlier_uses_are_ignored_with_personal_context_off`、`used_initials_words_stay_behind_the_leading_syllable_rows`、`nine_key_picks_are_recorded_for_the_initials`（首位、分段词组记，单字和关掉个人上下文时不记），`session::tests::words_typed_on_the_full_keyboard_lead_the_nine_key_initials`（26 键打过的词在九宫格简拼里排第五，之后关掉个人上下文的会话不读早先的记录、查不到也不挪）和 `nine_key_personal_picks_count_like_the_full_keyboard`，`dictionary::pinyin::tests::jianpin_codes_merge_their_tables_by_weight`（每张表各自截断）。
- `crates/engine/tests/golden` 不受影响，基准未改（`cargo test -p msime-engine --test golden` 通过）：开着学习、又在九键里选词的场景（`nine_key_learning_and_management`、`personal_context_public_session`、`pick_pair_public_session`）选的都是单字（米、甲、你），不会记进个人模型；简拼查询在个人模型为空或学习关闭时与原来完全相同。
