# Agent Note: 拼音拆不开且英文完全匹配时，混输英文占首选

Status: implemented

## Problem

全拼、双拼开着中英混输时，英文第一行总是进 `priority_slot()`：首选之后，有云候选时在它后面，有 AI 候选或 AI 占位行时再往后一位。对拼音根本拼不出来的输入，首选位置上只是拼音凑出来的东西。用真实词库打 `hello`，列表是 [何咯（`Generated`）, hello, ...]，打 `world` 时英文同样排在拼音候选后面。空格上屏的是没人要的 何咯，想要的英文词每次都得多按一个数字键或点一下。这是「自动识别语种」的第一阶段：输入信号足够强时，引擎直接把英文放在首位。

「混输英文永不领先中文」这条规矩有来历，没有写进过任何笔记，这里补上。随包的 `msime-english.db` 按 Google unigram 计数给权重（dancing 14310606），拼音词库用的是自己的尺度（单词 225415），两者没法比较。按权重提英文的 overlay 先在 6877d2b13a（#1040）改成只在用户给过权重时提，又在 6b92c95ca8（#1063）整个撤掉，因为打 `danci` 时 dancing 排到了 单词 前面，空格上屏的是英文。那条理由今天仍然成立，所以本篇的规则从不比较英文和拼音的权重。

## Decision

下面几条全部满足时，英文行占首位（下标 0），否则照旧进 `priority_slot()`：

1. 方案是全拼或双拼，会话允许英文领先：没有在光标前缀解码（`prefix_active`），也没有激活辅助码（`has_active_helpcode()`）。由 `InputSession::mixed_from` 判定，作为 `english_may_lead` 传给 `CandidateQueries::mixed`。九宫格经 `insert_expressive_rows` 传「不领先」，九宫格和 14 键自己的英文规则不变。
2. 英文照常查得到：混输英文开着，输入长度不小于 `english.minimum_prefix`（产品默认 5，引擎自身默认 2），并且全是小写字母（带撇号不查）。门槛没有为完全匹配放宽。
3. 输入拆不成完整音节。全拼用 `crate::pinyin::segment::is_complete_pinyin_input`，只认 421 个严格的完整音节，不认纠错和别名；双拼用 `crate::shuangpin::query::is_complete_input(prefix, profile)`，要求每两键是一个音节。只有查到英文行时才计算这个判定。
4. `insert_mixed_rows` 去重之后，剩下的第一行英文的 `pinyin` 正好等于整段输入。完全匹配的那行因为文字和某个候选重复而被去掉时，不拿补全词顶上来。

只有这一行到 0 位，同一个词的其它写法（`ok` 的 `OK` 和 `ok`）和其余补全词照旧接在列表末尾。列表里有云候选、AI 候选或 AI 占位行时也是 0 位。

| 输入 | 全拼判定 | 结果 |
| --- | --- | --- |
| `hello` | he'l'lo，拆不开 | [hello, 何咯, ...] |
| `world` | wo'r'l'd，拆不开 | [world, ...] |
| `change` | chang'e，完整 | 嫦娥 仍是首选，英文在优先位 |
| `women`、`ni`、`nihao`、`danci` | 完整 | 不变 |
| `home` | ho'me，拆不开 | 四个字母，产品默认前缀 5 时不查英文，不变 |
| 小鹤双拼 `hello` | 奇数长度，拆不开 | [hello, ...] |
| 小鹤双拼 `home` | ho（huo）+ me，完整 | 不变 |

### 下游配套

- **在线座位**：`msime-input-runtime` 的 `normalize_online_slots` 已经把「本地候选还在、首位却是英文」当作领先的英文（`promoted_english`），云候选、AI 候选到达后它仍占首位。这一层不需要另加规则，由 `promoted_english_candidate_keeps_the_first_seat_with_cloud_and_ai` 覆盖。
- **句子模型重排**：`crates/engine/src/ordering.rs` 的 `rerank_pick` 在首行来源是 `EnglishDictionary`（4）时返回 None。若不挡，英文行算作回答了按键（`answers_key`），又不是可信的词典命中，`chinese-ime-lm` 的 `comparable()` 会让模型拿它比较，把它偏爱的中文行转到英文前面；`normalize_online_slots` 在重排之后才跑，也不会把它放回去。这只在开了 `neural_keyboard` 或 `neural_desktop` 时发生，两项默认都关。网页引擎（`msime-engine-wasm`）调用同一个函数，但它把混输英文写死为关，行为不变。
- **学习**：`learn_candidate` 把下标 0 当作默认选择，不调词频。英文领先时，下标 1 的中文才是拼音给的首选；选它时首行是 `EnglishDictionary` 就同样直接返回，不调词频，也不写日志。否则 `adjust_candidate_ranking` 会把默认选择误当成一次有意的重排，至少多写一次日志。

### 每道门槛的理由

- **去重之后再判完全匹配**：英文显示文字和中文候选重复是真实情况，golden `english_mixed_candidates` 的夹具就有 `('ni','倪')`。若在去重之前看 `english_rows.first()`，完全匹配的行一被去掉，补全词（`Ninja`）就会被提到首位。
- **保留 `minimum_prefix`**：随包英文库里有 298 个 2 到 4 个字母、不含 a/e/i/o/u/v 的词（拼音里 `v` 代 ü，只按 a/e/i/o/u 算是 339 个），比如 `dj`（DJ）、`cd`、`cs`、`gdp`、`pk`，它们正好是常用简拼（大家、成都、测试），也全都拆不成完整音节；`bus`（不是）同理。放宽门槛会让这些简拼丢掉中文首选。
- **光标前缀不领先**：英文行上屏走 `commit_selection` 的整段结束分支（`reset_composition`），在 `ni|hao` 这种光标前缀里选英文会把光标后的输入一起扔掉。
- **辅助码激活不领先**：大写的辅助码被转成小写拼进了查英文的输入，`niC` 查的是 `nic`。那串字母是拼音加辅助码，不是用户要的英文词。

### 契约现在的说法

有中文候选时，英文只在两种情况下领先：输入拆不成完整音节且英文完全匹配整段输入，或者用户把这个词固定在第一位（`apply_candidate_positions`）。这句话写在 `queries.rs` 里 `mixed`、`insert_mixed_rows`、`priority_slot` 的文档注释，`crates/input-runtime/src/runtime.rs` 里 `normalize_online_slots` 的注释，input-runtime 测试 `promoted_english_candidate_keeps_the_first_seat_with_cloud_and_ai` 的注释，以及 `tools/engine-golden/README.md`。引擎测试改名为 `mixed_english_never_outranks_chinese_on_complete_pinyin`，它打的 `ni` 是完整音节，钉住的仍是 #1063 的结论：不凭随包权重、不因上屏过、不因置顶过让英文领先。

## Alternatives considered

- **保持现状，英文永远在优先位置** — 最强的理由是只有一条规则：空格上屏的永远是中文，#1063 的不变量原样不动，中文用户不会遇到意外。没有采用，是因为对拼音拼不出来的输入，首选只是凑出来的猜测（何咯），空格上屏的是废字，打英文的人每个词都要多按一次键。
- **比较英文和拼音的权重** — 最强的理由是通用：`women`、`change` 这类能拼成拼音的英文词也有机会领先，不需要音节判定。没有采用，是因为两边的权重不在一个尺度上（dancing 14310606 对 单词 225415），#1040 和 #1063 已经这样做过两次并撤回。
- **完全匹配时绕过 `minimum_prefix`** — 最强的理由是 `home`、`ok`、`hi` 这类短词也能领先，而且有先例：九宫格在拼音一行读法都没有时不看混输开关和最短前缀（见 [九宫格无拼音读法时给英文词](../bug-fix/2026-10-08-nine-key-path-ranking.md)）。没有采用，是因为那个先例只在列表本来为空时生效，这里列表里有中文；绕过门槛后 `dj`、`cd`、`gdp`、`bus` 这些常用简拼都会被英文抢走首位。
- **拼音拆不开时就提英文第一行，不要求完全匹配** — 最强的理由是信号来得更早：打到 `hel` 时 hello 就能领先。没有采用，是因为补全词不是用户打出来的东西。拆不开的输入多半是没打完的拼音或简拼，空格会上屏一个用户还没拼完、也未必想要的词。
- **在 `Session::snapshot` 里把英文行的 `answers_key` 置为 false，代替在 `rerank_pick` 里挡** — 最强的理由是只改引擎，不碰 `ordering.rs` 这份网页引擎也在用的共享代码。没有采用，是因为 `answers_key` 是行本身的事实（选这行确实结束组字），会写进快照和 golden 期望（`english_mixed_candidates` 里 `Ni` 记的是 `"answers_key": true`）；改它会改动所有带英文的 golden，并且连带改变英文在第二位时的重排比较，而要挡的只是「英文在首位」这一种。

## Consequences

- **收益**：
  - 拼音拼不出来、又正好是英文词的输入（`hello`、`world`），空格直接上屏英文，不必再选第二个。
  - 规则只看「拆不开」和「完全匹配」两个事实，不比较权重，#1063 的理由仍然成立。
  - 能拼成完整音节的输入一条都不变。脚本化 golden 没有一个快照同时满足「拆不开、完全匹配、列表里有中文」，期望文件一个都没改；真实词库集（`quanpin-words-v1` subset3000、`sentences-*`）的 4477 条输入全是完整音节。
- **代价与已知上限**：
  - 结束组字的标点或宿主的完成键走 `finish_composition`，它上屏首选，所以 `hello,` 现在上屏整个英文词（默认中文标点下是 `hello，`），原来上屏的是中文首选。
  - 双拼不对称：双拼的输入是双拼按键，奇数长度的词总是拆不开，偶数长度且每两键都是音节的词（小鹤 `home`）不会领先。同一个词在全拼和双拼下表现可以不同。奇数长度的按键状态也包括打到一半的音节：打字过程中每个奇数长度、不短于 `minimum_prefix` 的中间状态，只要恰好拼成一个英文词，英文就会短暂占首位，这时按下结束组字的标点会把这个英文词上屏。
  - 输入长度不小于 `minimum_prefix`、拆不成音节、又恰好是英文词的简拼会丢掉中文首选。宿主层回车原样上屏时会把拆不开的字母串学成权重 10 的英文词（`learn_entered_english_word`），所以一个回车上屏过的错字串，下次完全匹配后会领先；要纠正得删掉那个英文词，或者固定中文的位置。
  - 混输里选英文行不学习（`learn_candidate` 对混输英文直接返回），想要中文的用户每次都要选第二个，没法从选择里学回来。
  - 用户把英文固定在第一位时，选紧跟着的中文现在也算默认选择，不再调词频、写日志。
  - 首行是英文的列表（本规则领先的、用户固定在第一位的、只有英文行的）不再经过句子模型重排。
  - 真实词库 golden 只有设置了 `MSIME_EVAL_RESOURCES` 才跑，CI 不覆盖它们；评测集里出现拆不开又完全匹配的输入时，首选会变。
  - 重访信号：用户反馈英文抢走了简拼的首选，就考虑加元音或长度条件；双拼用户反馈打字途中英文闪到首位、或标点误上屏了英文，就重新评估双拼是否适用本规则；要让选择改变排序，属于后续阶段的学习，不在本规则里做。
- 已检索的相关笔记：
  - [混入的 emoji、颜文字紧跟它描绘的候选词](2026-10-08-expressive-rows-follow-their-word.md)：部分重叠，它的 Decision 写的「英文不变：第一行进优先位置」作为当时的决定保留，事实处已就地补上指向本篇的链接。
  - [九宫格无拼音读法时给英文词](../bug-fix/2026-10-08-nine-key-path-ranking.md)：部分重叠，九宫格「列表为空时给英文」是本篇最近的先例，两篇互链。
  - [AI 候选先占座位，回答到达时不挪动其他候选](../../archived/feature/2026-10-11-ai-seat-placeholder.md)：无关；`rerank_pick` 不比较来源 3 和 12 的事实不变，`priority_slot` 后来不再看 AI 行（[AI 候选坐第一页最后一格](2026-10-16-ai-candidate-last-slot.md)）。
  - [拼音组字里直接出日期、时间、星期和农历](2026-10-10-inline-date-time-candidates.md)：无关，它否掉固定优先位的理由（简拼 `sj` 被误伤）和本篇保留 `minimum_prefix` 的理由同源。
  - [双拼的纠错整句](2026-10-09-shuangpin-typo-sentence.md)：无关，`rerank_pick` 里另一处不让某类行被模型转到首位的先例（双拼纠错行）。
  - [全拼、双拼里混进一行日文整段转换](2026-10-12-mixed-japanese-candidates.md)：部分重叠，第二阶段沿用本篇的音节判定和优先位置；日文和英文都领先时英文第一、日文第二，「领先行后面是默认选择」推广到两类行。

## Verification

- `cargo test -p msime-engine --lib ime::queries`：`an_exact_english_word_leads_input_pinyin_cannot_spell`（完全匹配领先，`None` 或只是补全时不领先）、`only_the_first_exact_english_row_leads`、`a_leading_english_word_keeps_the_first_seat_ahead_of_cloud_and_ai_rows`（AI 候选和占位行都算）、`a_deduplicated_exact_word_lets_no_completion_lead`、`quanpin_english_leads_only_input_that_is_not_whole_syllables`（`hello` 领先，`change`、`women` 不变，`english_may_lead` 为假时不领先，`home` 在前缀 5 时不查英文、前缀 4 时领先）、`shuangpin_english_leads_only_keys_that_are_not_whole_syllables`（小鹤 `hello` 领先，`home` 不领先）。
- `cargo test -p msime-engine --lib session::tests`：`an_exact_english_word_leads_input_pinyin_cannot_spell`（会话里 `hello` 领先、标点整段上屏 `hello，`、`he` 仍是中文首选）、`helpcode_and_caret_prefix_keep_an_exact_english_word_behind_the_first_seat`（对照 `nic` 领先，`niC` 和光标前缀 `nic|nihao` 不领先；去掉任一道门槛它都会失败）、`picking_the_chinese_row_behind_a_leading_english_word_is_the_default_pick`（零分配、不建日志；去掉学习的改动它会失败）。
- `cargo test -p msime-engine --lib host::tests::mixed_english_never_outranks_chinese_on_complete_pinyin`。
- `cargo test -p msime-input-runtime`：`the_reranker_leaves_a_leading_english_word_in_place`（同一张表首行是整句时模型提 和，首行是英文时不动；去掉 `rerank_pick` 的判断它会失败）、`promoted_english_candidate_keeps_the_first_seat_with_cloud_and_ai`。
- `cargo test -p msime-engine`（含全部脚本化 golden，期望文件未改）、`cargo clippy -p msime-engine -p msime-input-runtime --all-targets -- -D warnings`、`cargo build -p msime-engine-wasm`。
