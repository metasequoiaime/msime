# Agent Note: 九宫格候选混入 emoji 和颜文字

Status: implemented

## Problem

#5848（源自 #5667）：「候选带 emoji」「候选带颜文字」两个开关（共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`）只对 26 键的全拼、双拼生效，用九键打 634486（meiguo）不会出现 🇺🇸。#5667 的报告人用的就是 Android 九键。

原因在 Engine：`Session` 在九键模式下把数字交给 `NineKeySession`（`crates/engine/src/nine_key.rs`），它的 `refresh` 只组装词库行、简拼行和英文行，从不经过 26 键的 `CandidateQueries::mixed`（`crates/engine/src/ime/queries.rs`）。`mixed` 本身也只接受字母组成的前缀，`query_emoji` / `query_kaomoji` 按拼音字符串前缀查 `msime-others.db`，一串数字没有唯一的拼音，不能直接拿来查。Android、iOS、HarmonyOS 的九键都走这个会话，三端都受影响。

## Decision

- `NineKeySession` 多一个 `MixedExpressiveOptions`，由 `Session::new` 从 `SessionOptions::expressive` 经 `set_mixed_expressive` 交给它，和英文选项一样只在建会话时设置。两个开关都关时（Android 默认）不查任何东西，列表与原来逐项相同。
- 每次 `refresh` 在拼音候选排好序之后、插入英文行之前，按数字串可能的读法查 emoji 和颜文字（`expressive_candidates`）：
  - 读法是已锁定的拼写加上一条拼满其余数字的切分路径，只取字母。路径就是查词用的 `alternatives`，已经按用户打的切分（「分词」键）和左列选的首字母筛过；只拼了开头一个音节、供部分选择用的那几条不算，它们对应的不是整串数字。
  - 读法按「支持它的拼音候选在列表里的位置」排先后：候选覆盖全部数字、不是模糊音行、全拼的字母以这个读法开头，就算支持；没有候选支持的按路径原来的先后排在后面。只查前 `EXPRESSIVE_READING_LIMIT`（8）种。
  - 查询走 `local/emoji.rs` 的 `query_emoji_readings` / `query_kaomoji_readings`：读法逐个查，跨读法按文字去重；每种读法取几行、读法内的先后和合计上限见 [2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md)。
  - 命中的编码还要过 `splits_into_syllables_at`：编码要能在每个已确定的音节边界（锁定拼写的结尾、用户打的切分）处切开，各段都能切成完整音节。没有边界时不检查，与 26 键的前缀匹配相同。
  - 门槛和排除与 26 键相同：数字不足 `MIXED_EXPRESSIVE_MINIMUM_INPUT`（2，26 键原来写死的 `prefix.len() >= 2` 改成这个常量，两边共用）个时不查；九键纯英文模式和会话不允许全拼时 `refresh` 只给英文行，不混入；按单字、笔画筛选时也不混入，那两个筛选针对的是汉字。
- 插入用 `queries.rs` 的 `insert_expressive_rows`，它就是不带英文行的 `insert_mixed_rows`，与 26 键同一套去重和定位规则；九宫格的英文行仍按它自己的规则先插好（有权重的首个英文词占第二位，零权重的放到末尾），emoji、颜文字在英文之后插入。定位规则本身（紧跟描绘的词、接不上的排末尾）见 [2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md)，它取代了这里最初「每组第一行进优先位置」的做法。
- 混入的行 `pinyin` 是整串数字，选中后吃掉全部数字、结束组字；它们没有 `canonical_pinyin`，也不是词库行，所以不调词频、不能置顶删除固定位置，也不会被学成用户词（`learn_selection` 只收词库行和整句行）。
- Android「设置 › 表达 › 智能」两个开关的说明改为「全拼（26 键或 9 键）、双拼输入时……」。iOS（`CandidateOptionsSettingsView` 的「emoji 混输」「颜文字混输」）和 HarmonyOS（共享设置页 `packages/ui/src/settings/mixed-input-section.tsx`）的说明本来就没有限定 26 键；各端说明里关于位置的那半句随定位规则一起改了，见 [2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md)。

## Alternatives considered

- **把九宫格的数字先换成一个拼音再调 `CandidateQueries::mixed`**：一条代码路径，`mixed` 里的规则一处都不用碰。但数字串没有唯一的拼音：只取首选的读音，首选是英文词、简拼行、整句行或模糊音行时就没有可用的读音，读出来的也只是众多读法之一；`mixed` 还会顺带按这个前缀查英文，而九宫格的英文是按数字展开字母查的另一套规则，两边会重复或冲突。
- **把数字展开成全部字母串去查**（像九宫格英文那样按 `letter_prefixes` 展开）：不依赖音节表，任何编码都查得到。但六个数字就有 3^6 到 4^6 种字母串，绝大多数不是拼音；`ENGLISH_PREFIX_BUDGET` 那样截断前缀又会让 `mei` 这种短前缀匹配上一大片无关的 emoji，并且绕开了切分和左列锁定，违背「只用符合已确定音节的拼音去匹配」。
- **读法按切分路径原来的先后取前几种**：最简单。路径的先后只看音节数、末段是否完整和音节频度，与用户眼前的候选无关；而每种读法、每组的行数都有上限，排在前面的冷门读法会把首选那个词的 emoji 挤掉。按支持它的拼音候选排先后，首选的读法总是第一个被查。
- **不查音节边界，编码以读法开头就算**：与 26 键完全一样，少一段代码。但锁定 xian 之后再打 4，读法字母是 xiang，香蕉的 🍌（`xiangjiao`）就会出现，而 xian 之后的 gjiao 怎么也切不成音节，用户已经排除了这个读音。反过来，`msime-others.db` 的编码不记音节在哪里分开，`xiangjiao` 也能切成 xi'ang'jiao，所以锁定 xi 或切成 `94'26` 时 🍌 仍会出现；能做到的只是排除「怎么切都对不上」的编码，见 Consequences。
- **按编码的最少音节切法判断边界**（`xiangjiao` 只认 xiang'jiao）：能把上面 xi'an 的情形也排除。但同一串字母本来就可能有两种读法（`xian` 是先也是西安，`fangan` 是方案也是反感），最少音节切法会把用户切出来的那种读法的 emoji 判错。宁可多出现一个也不漏掉用户要的那个。
- **把英文行也交给 `insert_mixed_rows` 一起插**：九宫格和 26 键就完全是同一个插入函数。但九宫格的英文有自己的规则（零权重的词放到末尾、拼音一行都没有时英文就是全部答案），改走 `insert_mixed_rows` 会让英文行多一次与拼音行的去重、零权重词换位置，开关全关时的列表就不再与原来逐项相同。

## Consequences

- **收益**：三端九键在开关打开后都有 emoji、颜文字候选，排位、去重、最短输入与 26 键是同一套代码；Android 默认关，现有用户的九键候选不变。
- **代价**：开关打开时每次刷新最多多查 8 种读法 × 2 张表。下面是最初「每组第一行进优先位置、取够三行即停」时的数字，改成紧跟描绘的词之后的数字见 [2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md)。在出货词库上用 release 构建交替测量（同一进程里开、关两个会话各打 40 遍，Mac Studio，它同时是 CI runner，数字有噪声）：634486 每串 2.20 → 2.62 ms、5464426 14.5 → 15.2 ms、94264 14.4 → 14.8 ms、9426494264 74.4 → 74.3 ms、94264542648 85.9 → 91.6 ms；全部按键的单键中位数 3.15 → 3.25 ms，p95 12.9 → 14.5 ms。
- **已知上限**：编码不记音节边界，锁定或切分落在一个编码的另一种合法切法上时（锁定 xi 后的 `xiangjiao` 可以读成 xi'ang'jiao），这个 emoji 照样出现。超过 8 种读法时排在后面、又没有候选支持的读法不查。混入的行在九宫格里与 26 键一样，没有可以学习或管理的读音。真机上没有验证过，只有 Engine 回归测试和 Android 的 Java 编译。
- 已检索的相关笔记：[2026-10-08-android-symbol-catalog-and-emoji-candidates.md](2026-10-08-android-symbol-catalog-and-emoji-candidates.md) 的「九键不混入 emoji」缺口由本篇补上，那里的事实已就地更新并链到这里；[2026-10-07-mixed-candidate-dedup-state.md](../testing/2026-10-07-mixed-candidate-dedup-state.md) 的栈上去重表仍在 `insert_mixed_rows` 里，容量随取回的行数改了，见 [2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md)；[2026-10-08-nine-key-initials.md](2026-10-08-nine-key-initials.md) 与 [2026-10-08-emoji-query-prefixes.md](../testing/2026-10-08-emoji-query-prefixes.md) 讲的是同一个刷新函数和同一个查询模块的其他部分，无冲突。

## Verification

- `cargo test -p msime-engine --lib nine_key`：`digits_put_emoji_and_kaomoji_after_the_words_they_depict`（634486 出 🇺🇸、5464426 出 ⚠️、54 出 🐔，与拼音候选和跨读法重复的只出现一次，只开 emoji 时没有颜文字）、`expressive_rows_follow_a_leading_english_word`、`both_switches_off_leave_the_list_unchanged`（开关打开的列表去掉混入行后与关闭时逐项相同）、`expressive_rows_need_two_digits_and_pinyin`（一个数字、纯英文、筛选、不允许全拼）、`expressive_rows_follow_chosen_syllables_and_splits`（锁定 nei 后没有 🇺🇸，锁定 xian 或切成 `9426'` 再打 4 时没有 🍌）、`choosing_an_expressive_row_commits_it_and_ends_the_composition`、`expressive_readings_are_capped_and_led_by_the_candidates`、`expressive_keys_must_split_at_the_fixed_boundaries`。
- `cargo test -p msime-engine --lib`：`ime::queries` 的 `nine_key_expressive_rows_use_the_same_rules`，`local::emoji` 的 `mixed_rows_without_catalog_tables_are_checked_against_the_matched_key`，`session::tests::nine_key_mixes_emoji_and_kaomoji_like_the_full_keyboard`（同一份词库下 26 键与九键打同一个词，emoji 都紧跟这个词）。
- `cargo test -p msime-engine`（含 golden）、`cargo clippy -p msime-engine --all-targets -- -D warnings`、Android `compileFullReleaseJavaWithJavac`。
