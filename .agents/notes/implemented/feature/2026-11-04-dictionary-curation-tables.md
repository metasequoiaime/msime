# Agent Note: 词库清洗走 msime-dictionary 的 curation/ 表，构建器在读入之后施加

Status: implemented

## Problem

msime-dictionary 的大清洗（碎片词压过常用词、上游权重被改坏、占位权重、含粗俗词的长词条、英文蔑称）要删词、改权重，可是能改的地方都不对：`sources/` 下的上游文件由 `upstream.lock.json` 逐字节锁定，`custom/words.txt` 只追加而且只升不降，`custom/` 还被 msime-web、msime-cloud 直接按 `main` 读写。引擎内嵌的粗俗词表（[2026-10-29 的笔记](../bug-fix/2026-10-29-offensive-words-out-of-sentences.md)）换一次要发一版引擎。英文那边，`english_supplement::OFFENSIVE` 只过滤 SCOWL 补充表，rime-ice 自己词表里的 nigger、tranny、beyatch 照样进 `english_words`。清洗还需要先有候选清单，由维护者逐条审定，不能自动删。

## Decision

维护者审定的表放在 msime-dictionary 新的顶层目录 `curation/`，构建器从 `--dictionary` 检出读（`sources::DICTIONARY_DIRECTORIES` 加上 `curation/`，`source.dirty` 也看它），在读入之后施加（`crates/dict-builder/src/curation.rs`）：

- `curation/pinyin-vetoes.txt`：`词` 整词否决（至少两个字）或 `词<TAB>全拼` 只否决一个读音。新阶段 `word-vetoes` 在 `reading-corrections` 之后、`wubi` 之前从全拼表删行；整词否决同时经 `write_code_table` 让 `wubi86`、`wubi98`、`wubixsj` 不写入这个词（原表的行和生成的补充行都算）。
- `curation/pinyin-weight-overrides.txt`：`词<TAB>全拼<TAB>权重`。新阶段 `weight-overrides` 紧接其后，把权重设成这个值，可降可升。这一行是 `custom/words.txt` 从已有的行提了权的（custom-words 之前并入的单字表、词组输入和 `places.txt` 已有这个词和读音，权重低于 custom 写的；`curation::custom_promotions` 按同样的输入重新算）时构建失败，人工词条「只升不降」的提权不被覆盖表推翻。custom 新插入的词可以覆盖：计划 H11 里已有的 13 条越位（挽昼 压过 万州、张雯 压过 掌纹 这类）正要走这里降权。
- 两张拼音表经 `PinyinCuration::rows` 交给 `wubi_supplement::frequencies`、`pinyin_words` 和 `places_supplement` 读对照集合的每一处：五笔三个生成器的 100 门槛、同码排序和区划生成器的读音确认、已有权重、上限，看到的都是施加之后的拼音行；整词否决的词不做五笔词组候选。表存在时，生成文件的表头把它们和 SHA-256 列进输入。
- `curation/offensive-words.txt`：格式与引擎内嵌的 `offensive_words.txt` 相同。新阶段 `sentence-excluded` 用引擎导出的同一个解析器（`msime_engine::parse_offensive_words`）读，写进 `msime-pinyin.db` 的 `sentence_excluded(value, exempt)`（`format::SENTENCE_EXCLUDED_SCHEMA`，`docs/data-formats.md` §4.5）。引擎打开词库时读这张表（`OffensiveWords::from_database`），经 `SentenceNgramTables` 随 `LatticeOptions` 交给整句、纠错整句，联想在打开词库时读；没有这张表、读不出来、一个词都没有时用内嵌表。只追加新表，不改 `FORMAT_VERSION`。
- `curation/english-vetoes.txt`：一行一个小写词。`english` 阶段对全部英文来源（rime-ice 两张词表、SCOWL 补充表、OALDPE 词头、`custom/english.txt`）统一否决，任何大小写都不写入；`custom-translations` 阶段也不写英文一侧（英译中的源词、中译英的译文）含被否决词的人工译文。内置的一组（`english_supplement::OFFENSIVE` 加 `curation::BUILT_IN_VETOES` 的 nigger、tranny、beyatch）总是生效，没有 `curation/` 也一样。
- `check-words` 拒绝构建会删掉的新词：`--pinyin-vetoes` 给的表否决的词或读音，英文否决集合里的词（内置的总是算），以及英文一侧含这些词的新译文。
- `msime-dict-build cleanse-report` 只写候选清单（上游权重相差过大、占位权重、压过同键常用词的碎片词、含粗俗词的长词条），每类一个 TSV，表头记 msime 提交、msime-dictionary 提交和对照文件的 SHA-256，已被 `curation/` 处理过的行不再列出。

### 每张表的两条硬规矩

- **文件不在就是空表。** 没有 `curation/` 的检出构建出的 `msime-pinyin.db`、`msime-wubi.db` 和各生成文件与之前逐字节相同（三个新阶段在表为空时不打开数据库，`sentence-excluded` 只在 `--out` 里有上一次留下的表时删掉它；空的 `PinyinCuration` 交给生成器的行与不经它时相同），所以这部分可以先于数据合入，不会让下一次发布失败。`msime-english.db` 不在此列：内置的英文蔑称不靠 `curation/` 生效，按 `dict-v2.0.41` 的内容，`english_words` 少 nigger、tranny、beyatch 3 行，`en_zh_glosses` 少 2 行，`zh_en_glosses` 里 深棕色 不再译成 nigger。这属于计划 §5 第 1.2 行「会改变产物」的那一半，任务要求随 1.1 一起做；词库的发布 workflow 还没有固定 `msime_ref`（R-1），这个提交合入 develop 后的下一次发布就会带上它。
- **每一条都必须匹配到东西。** 否决删不到任何行（整词否决还要在五笔原表里也找不到）、覆盖找不到行、粗俗词不是全拼表里的词或豁免写法不在任何词里、英文否决在所有来源里都没有，构建都失败、表不动，照 `apply_reading_corrections` 的做法，表不会在输入变了之后悄悄过期。

## Alternatives considered

- **放进 `custom/`（例如 `custom/vetoes.txt`）。** 最强的理由是不用新建目录，check-words 和发布 workflow 的路径过滤现成就覆盖 `custom/**`。不采用：`custom/` 是投稿目录，msime-web、msime-cloud 不经锁文件直接按 `main` 读写它，否决和权重覆盖是维护者审定的数据，不能跟投稿走同一个入口；`custom/` 的「只追加」规则也与否决表要能删条目（过期就删）冲突。代价是发布 workflow 的触发路径要加上 `curation/**`，由词库仓库那边改。
- **直接改上游文件或重新生成补充表。** 最强的理由是数据一处到位，构建器不用多几个阶段。不采用：上游文件逐字节锁定，改了就和上游提交对不上；补充表重新生成时会把手工改动冲掉。
- **粗俗词表照旧只在引擎里，靠发引擎换表。** 最强的理由是不碰词库格式，也不用测新旧两个方向。不采用：换表要发六个平台的客户端，而词库 24 小时内自动更新；内容审核也该和词条在同一个仓库。保留内嵌表作为退路，新旧引擎和新旧词库四种组合都能用。
- **英文否决只扩充 `english_supplement::OFFENSIVE`。** 最强的理由是改动最小。不采用：那张表只在生成 SCOWL 补充表时用，而且要求每个词都是 SCOWL 的词形（防止过期），rime-ice 词表里的 beyatch 不满足；在 `english` 阶段统一否决才能管住全部来源。英文生成器也不读 `curation/english-vetoes.txt`，否则补充表里不再有那些词，否决就匹配不到、构建失败。
- **覆盖表不许碰 custom 列过的任何一行（包括 custom 新插入的词）。** 最强的理由是规则简单，只看 `custom/words.txt` 本身，不用重读 custom 之前的输入；人工词条的权重也就只有投稿流程能改。不采用：计划把已有的 13 条越位交给 H2 降权，custom 新插入的词的权重就是投稿时定的，没有「被提权」可言，一概拒绝会让那批覆盖建不出来。只拒绝 custom 从已有的行提了权的读音，守住的是「只升不降」本来的意思：投稿把一个已有的词提上去，覆盖表不把它压回来。
- **覆盖表对 custom 的行也一律放行。** 最强的理由是维护者审定的覆盖本来就比投稿更可信。不采用：custom 提权的行被覆盖压回去，等于另开一个绕过投稿流程的入口，而计划 H2 明确不许作用于「已提权的行」。

## Consequences

- 收益：删词、改权重、换粗俗词表都只改 msime-dictionary 的一个文本文件，随一次词库发布生效，不发引擎；每一处改动都有行号、能审、能 revert；候选报告让清洗从「查数据库」变成「审清单」。
- 代价：构建器多了三个阶段和一个子命令，五笔和区划生成器多了一个输入；`curation/` 的条目过期会让发布失败（这是有意的），维护者要在输入变化时同步删条目。
- 粗俗词表迁过去时要先删掉内嵌表里匹配不到任何词条的两条豁免写法 `!内射模`、`!性奴役`（在 dict-v2.0.41 的全拼表里不出现），否则 `sentence-excluded` 阶段会失败。
- 兼容性：旧引擎配带 `sentence_excluded` 的新词库只是多一张不读的表，格式版本不变；新引擎配旧词库退回内嵌表。`msime-dict-build web` 原样保留这张表，但这次改动之前的 `web` 遇到不认识的表会失败，所以 `resources/desktop-dictionary.lock.json` 换到带这张表的发布，要在已经有这次改动的分支上做（`docs/data-formats.md` §4.5）。
- 许可：`curation/` 对 rime-ice（GPL-3.0）数据的删行、改权重，和内置英文蔑称对 rime-ice 英文词表的删词，都是对 GPL 数据的修改，要在 msime-dictionary 的 `NOTICE.md` 写明（照「拼音权重与万象对齐」和读音更正那两段的做法）。msime 这边的 `resources/licenses/msime-engine-dictionary-NOTICE.md` 先在「已登记、尚未随当前词库发布的来源」一节记下，词库锁换到带这些改动的发布、副本重新同步之后删掉。
- 词库仓库那边要跟上：发布 workflow 的触发路径加 `curation/**`；`check-words.yml` 把 `MSIME_COMMIT` 升到带这些改动的提交，并传 `--pinyin-vetoes`、`--english-vetoes`，这样投稿就加不回被否决的词。两张表要从 PR 的 base 提交取（照 custom/ 的做法 `git show "$BASE_SHA:curation/pinyin-vetoes.txt" > "$RUNNER_TEMP/base-pinyin-vetoes.txt"`，base 没有这个文件时不传或传一个不存在的路径，按空表算），不能用 head 检出里的：否则同一个 PR 删掉一条否决、再加回那个词就能通过。同时改了 `curation/**` 和 `custom/**` 的 PR 直接拒绝或标出来交维护者看；`NOTICE.md` 补上上一条的修改说明。

## Verification

- `crates/dict-builder`：`curation::tests`（两张拼音表的解析与冲突、`rows` 对否决和覆盖的施加、`apply_word_vetoes` 的整词和读音删除与过期失败、`apply_weight_overrides` 的升降、放行 custom 新插入的词、拒绝 custom 提过权的行（`custom_promotions` 认单字白名单）、`write_sentence_excluded` 的写入与过期失败、英文否决和译文英文一侧的否决），`main::tests::curation_stages_are_no_ops_without_tables_and_apply_them_when_present`（没有表时 `msime-pinyin.db` 逐字节不变、有表时三个阶段生效、整词否决不进 `wubi86`），`wubi_supplement`、`wubi86_supplement`、`wubi98_supplement`、`wubixsj_supplement`（连同 `build_wubixsj` 施加整词否决）、`places_supplement` 的 curation 用例，`cleanse_report` 同词同音只列一条，`msime::tests::wubi_tables_leave_out_whole_word_vetoes`，`english::tests::english_vetoes_apply_to_every_source`，`check_words::tests::vetoed_additions_are_rejected`（含译文），`cleanse_report::tests`，`tests/web.rs::the_sentence_excluded_table_is_kept`。
- 旧引擎配新词库实跑过一次：在基线 `a0dfe261eb` 的引擎上临时加两个测试（不提交），用新构建器的 `SENTENCE_EXCLUDED_SCHEMA` 建表，`rbuild cargo test -p msime-engine --lib -- session::tests::old_engine_new_db` 两个都过：旧引擎照内嵌表拦 巨乳、不理会表里的 巨人，带表的发布照常准备代次和查询。
- `crates/engine`：`lattice::offensive::tests`（运行时解析与编译期表一致、读进来的表取代内嵌表、空表退回内嵌表、`from_database` 的四种情形），`lattice::decode::offensive_tests::the_options_table_replaces_the_embedded_one`，`session::tests::sentence_excluded`（新引擎配旧词库、新引擎配新词库、带表的发布准备代次后照常查询并用表）。
