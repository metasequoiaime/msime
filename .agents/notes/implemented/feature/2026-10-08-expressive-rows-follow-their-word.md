# Agent Note: 混入的 emoji、颜文字紧跟它描绘的候选词

Status: implemented

## Problem

#5667 的报告人补充了截图，要的是「词汇和图案联动」：每个 emoji 紧跟在它画的那个词后面。九键打 54（li、ji）是「里 李 鸡 🐔 几」，打 jing'gao 是「警告 ⚠️ 警…」，原帖也说输入「美国」先出美国、第二个是 🇺🇸。

原来的规则（参考实现 candidate_queries.cpp:101-195，26 键和 [九宫格](2026-10-08-nine-key-expressive-candidates.md) 共用）不看词：emoji、颜文字各取按编码前缀命中、按目录顺序的前三行，每组第一行放进首选之后的优先位置，其余接在末尾。于是 54 的第二个候选是 😁（`li` 开头的某个编码），而 🐔 根本取不到；`meiguo` 的第二个候选是编码更长、目录顺序更靠前的 🇺🇲（美国本土外小岛屿），🇺🇸 落到列表末尾（#5907）。

## Decision

- 混排查询（26 键的 `query_mixed_emoji` / `query_mixed_kaomoji`，九宫格的 `query_emoji_readings` / `query_kaomoji_readings`，都在 `crates/engine/src/local/emoji.rs`）连同目录关键词一起取：`emoji_pinyin` LEFT JOIN `emoji` 取 `emoji.keywords`，`kaomoji` LEFT JOIN `kaomoji_catalog` 取 `kaomoji_catalog.keywords`。两张目录表随包 `msime-others.db` 早就有（表情面板读的就是它们），不改词库格式、不改 `dict-builder`、不需要发新词库。资源里没有目录表时退回不带关键词的同一查询，行照样给，只是接不上词。
- 每种读法内编码与读法完全相同的行排在前面，再按目录顺序；每种读法最多取 `MIXED_FETCH_PER_READING`（16）行，每组合计最多 `MIXED_FETCH_LIMIT`（48）行。`ji` 开头的编码有两百多行，🐔 按目录顺序排在九十几位，但它的编码正好是 `ji`，排第一；🇺🇸 的编码就是 `meiguo`，排在 🇺🇲 前面。
- 插入规则在 `crates/engine/src/ime/queries.rs` 的 `insert_mixed_rows` / `anchor_expressive_rows`，26 键和九宫格（`insert_expressive_rows`）共用：
  - 去重不变：每组先按文字与列表、与前面各组去重。
  - 英文不变：第一行进优先位置（首选之后，有云候选、AI 候选时再往后），其余接在末尾；九宫格的英文仍按它自己的规则先插好。
  - 一行 emoji 或颜文字的某个关键词（空格分隔）与某个候选词完全相等时，它紧跟在第一个这样的候选后面。同一个词后面的几行 emoji 在颜文字前，各自按查询的先后。它总跟在一个词后面，所以不会占首选。候选词可以是云候选、AI 候选或英文词；只对插入前的行找词，混入的行不互相接。
  - 接不上任何词的行每组只留前 `MIXED_RESULT_LIMIT`（3）行，emoji 在前，排在列表末尾，不再占优先位置。
  - 找词用候选词连同位置排好序的索引二分查找（同一个词取最前面的位置）：26 键的列表常有几百行（`ji` 七百多行），逐个关键词从头扫列表在 `ji`、`shi` 上每次按键要多花 1 ms 以上。
- 两个开关都关时不查、不插，列表不变。`mixed_expressive_*` golden 的夹具没有目录表，所有混入行都排到末尾，期望文件按此重排（权重等其余字段不变），`tools/engine-golden/README.md` 记了这次改动。
- 各端设置页的说明从「排在英文候选之后；云候选与 AI 联想会使其相应顺移」改成「紧跟在它描绘的那个词后面，对不上候选词的排在末尾」：共享设置页 `packages/ui/src/settings/mixed-input-section.tsx`、Windows `platforms/windows/settings/main.cpp` 与 `installer/config.default.toml` 的注释、iOS `CandidateOptionsSettingsView`、Android `ExpressionPage`。

## Alternatives considered

- **保留固定的优先位置，只把编码完全相同的行排到前面**：改动最小，`meiguo` 的第二个候选就成了 🇺🇸，#5907 的验收标准也满足。但优先位置与词无关：54 的首选是「里」，第二位放的是 ji 的 🐔 还是 li 的 🍐，都和它旁边的词对不上；报告人截图里要的是 🐔 跟着「鸡」、出现在第三四位，固定位置做不到。
- **调整目录顺序**（在 `crates/dict-builder` 构建 emoji 表时把常用项、编码短的项排前，#5907 原本提的修法之一）：查询代码不用动。但要发新词库，已装的旧词库不受益；而且只解决「哪一个排第一」，解决不了「排在哪个词旁边」；目录顺序还被表情面板用作分类内的展示顺序，为候选改它会牵动面板。
- **按编码与候选的全拼相等来接**（🐔 的编码 `ji` 等于「鸡」的全拼）：不用目录表。但同音字太多，`ji` 的 🐔 会接到「级」「几」「机」后面，编码说不出画的是哪个字；关键词说的是词本身，「鸡」才是它画的东西。
- **按候选词去目录里反查**（`keywords LIKE '%鸡%'`）：不受编码前缀和取回行数的限制，候选里有哪个词就找哪个词的 emoji。但 `keywords` 没有索引，每个候选一次全表扫描，两百多个候选就是几百次；查询也不再以输入码为准，会带出与输入无关的 emoji。
- **只在前若干个候选里找词**：每次插入的比较次数有固定上限。但排好序的索引已经把找词的代价降到可以忽略（见下），截断窗口反而让窗口外的词明明在列表里却接不上，规则多一个说不清的例外。
- **用哈希表做候选词索引**：查找是常数时间。但每次刷新都要新建一张表，几百个候选时排序加二分已经足够快，有序数组只分配一次、结果确定。

## Consequences

- **收益**：开关打开后 emoji 出现在它画的词旁边：九键 634486 是「美国 🇺🇸」，5464426 是「警告 ⚠️」，54 的 🐔 紧跟「鸡」；26 键 `meiguo`、`jinggao`、`ji`、`li`、`xiangjiao`、`nihao` 也一样。🇺🇲 这类只是编码以输入开头的行排到末尾，#5907 随之解决。
- **代价**：每次刷新多取一些行（每种读法 16 行，每组合计 48 行），每行多一次目录表查找，有关键词时多一次候选词索引的分配和排序。在出货词库上用 release 构建测量（Mac Studio，它同时是 CI runner，数字有噪声；同一进程里开、关两个会话交替各打 40 遍，取每串的中位数）。下表「改前」是最初每组第一行进优先位置的实现（`fa4a4a64e`），「改后」是本篇：

  | 输入 | 关 | 开（改前） | 开（改后） |
  | --- | --- | --- | --- |
  | 九键 634486 | 2.2–2.6 ms | 2.56 ms | 3.90 ms |
  | 九键 54 | 0.84–0.89 ms | 0.90 ms | 1.47 ms |
  | 九键 94264 | 14.3–14.5 ms | 14.9 ms | 16.8 ms |
  | 九键 74642648 | 43.6–43.9 ms | 47.1 ms | 47.3 ms |
  | 26 键 meiguo | 0.20 ms | 0.39 ms | 0.49 ms |
  | 26 键 jinggao | 0.41 ms | 0.69 ms | 1.02 ms |
  | 26 键 ji | 0.12 ms | 0.19 ms | 0.41 ms |
  | 26 键 shi | 0.42–0.43 ms | 0.60 ms | 1.19 ms |
  | 26 键 xiangjiao | 0.52 ms | 0.89 ms | 1.37 ms |
  | 26 键 zhongguoren | 1.00 ms | 1.46 ms | 1.89 ms |

  按每串总耗时除以长度算的单键中位数：关 0.20–0.24 ms，开（改前）0.27 ms，开（改后）0.46 ms。改用排序索引之前，逐个关键词扫列表的版本在 26 键 `shi` 上是 5.5 ms、`zhongguoren` 8.6 ms。
- **已知上限**：
  - 接不上任何词的行排在末尾，26 键的长列表（几百行）里基本看不到它们。
  - 颜文字目录的关键词多是拼音（`kai xin`），很少与候选词相等，大多数颜文字排在末尾。
  - 只有编码以输入为前缀、并且在每种读法取回的前 16 行里的行才可能接上词；编码与输入完全相同的行总在前面，但更长的词（26 键打 `ji` 时的「鸡头」）未必取得到。
  - 目录关键词原样使用：🐔 的关键词里有粗俗词「鸡巴」，所以 `jiba` 也会给出 🐔，这次不改数据。
  - 资源缺目录表时全部排在末尾。
  - 真机上没有验证过。
- 已检索的相关笔记：[2026-10-08-nine-key-expressive-candidates.md](2026-10-08-nine-key-expressive-candidates.md) 的插入规则由本篇取代，那里的事实已就地更新并链到这里；[2026-10-07-mixed-candidate-dedup-state.md](../testing/2026-10-07-mixed-candidate-dedup-state.md) 的栈上去重表容量随 `MIXED_FETCH_LIMIT` 变大，事实已就地更新；[2026-10-08-emoji-query-prefixes.md](../testing/2026-10-08-emoji-query-prefixes.md) 讲的是 E、M 模式用的 `query`，本篇没有改它。

## Verification

- `cargo test -p msime-engine --lib ime::queries`：`expressive_rows_follow_the_word_they_depict`（54 的 🐔 接在「鸡」后、🇺🇸 接在「美国」后、🇺🇲 排末尾，关键词要整个相等）、`unanchored_emoji_and_kaomoji_go_to_the_end`（接不上的排末尾、每组最多三行）、`several_rows_on_one_word_keep_emoji_before_kaomoji_in_query_order`（同一个词后 emoji 在颜文字前、各按查询先后，颜文字也能接，同一个词出现两次接在第一次后面）、`anchored_rows_follow_their_word_behind_cloud_ai_and_english_rows`（有云候选、AI 候选时英文仍在它们后面，emoji 可以接在云候选、英文词后面）、`nine_key_expressive_rows_use_the_same_rules`、`mixed_merge_keeps_bounded_dedup_state_off_the_heap`（每组取满 48 行时最多分配一次）。
- `cargo test -p msime-engine --lib local::emoji`：`mixed_rows_put_exact_keys_first_and_carry_their_keywords`、`mixed_rows_stop_at_the_fetch_limit`、`mixed_rows_without_catalog_tables_are_checked_against_the_matched_key`。
- `cargo test -p msime-engine --lib nine_key`：`digits_put_emoji_and_kaomoji_after_the_words_they_depict`（九键 634486、5464426、54）、`expressive_rows_follow_a_leading_english_word`。
- `cargo test -p msime-engine --lib session::tests::nine_key_mixes_emoji_and_kaomoji_like_the_full_keyboard`：同一份词库下 26 键 `meiguo`、`jinggao`、`ji`、`li` 和九键 634486、5464426、54 的 emoji 都紧跟它画的词，🇺🇲 排在末尾。
- `cargo test -p msime-engine`（含重排后的 `mixed_expressive_*` golden）、`cargo clippy -p msime-engine --all-targets -- -D warnings`。
