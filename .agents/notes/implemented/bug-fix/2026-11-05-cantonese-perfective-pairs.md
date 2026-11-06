# Agent Note: HKCanCor 把「咗」拼回前一个词计数，审校词按语料加权

Status: implemented

## Problem

#10909 报告粤语双拼里「动词加咗」排不上首位。msime-dictionary #132 把 借咗、做咗、去咗 收进 `custom/cantonese.txt`（`dict-v2.1.3` 起在库里），可是在粤语双拼里 借咗（`zezo`）、做咗（`zbzo`）仍是第二：审校词 essay 不收时权重固定取 `ESSAY_WORD_MIN_COUNT`（1000），而 `o` 键同时是 `o` 和 `oi` 两个韵母，同码上 essay 收的 這在（ze zoi，1105）和 早在（zou zoi，2121）更重。

这不只是 `o`/`oi` 同键的问题。用 `dict-v2.1.5` 的 `msime-cantonese.db` 逐码统计，两字的「X咗」共 76 条，有 36 条不是自己双拼码上的首选：食咗（20）在 `s;zo` 排第五，压在它上面的 星座 读 sing zo，韵母同样是 `o`；買咗（19）在普通粤拼 `maaizo` 里也排在 賣座（701）之后。根源在数据：essay 以书面语为主，几乎不收「动词加咗」（Rime 打字时由单字拼出来）；HKCanCor 是口语语料，但它把完成体助词「咗」单独标成一个词（`做/v/zou6/`、`咗/u/zo2/`），生成的 `hkcancor-word-counts.txt` 里只有 除咗、為咗 这种整体标注的词，做咗、買咗 一次也数不到。于是词表收的「X咗」只拿 `unlisted_word_weight` 的二十来分，审校表收的拿门槛值 1000，都低于同码上的书面语词。

## Decision

- `hkcancor-counts` 生成器在同一个 `<sent_tag>` 块里，把紧跟在一个全汉字标注后面的「咗」（`hkcancor::PERFECTIVE`）与它拼成一个词再计一次：`做` + `咗` 计 `做咗`，`唔記得` + `咗` 计 `唔記得咗`；拼出的词与语料本来整体标注的同形词合计（除咗 21 次加上拼出的 2 次）。块的边界、转写者的占位行都把「前一个标注」清掉，只拼真正相邻的两个标注。文件格式不变，仍是 `词<TAB>次数`，表头第 3 行写明这条规则和拼了多少个。固定的 HKCanCor 提交拼出 845 个，文件从 5102 行变成 5395 行。
- `languages` 给 `custom/cantonese.txt` 的审校词定权重时，essay 列了就用 essay 分数（不变，周杰倫 仍是 711）；essay 没列时用语料次数乘 `corpus_scale`，不到 `ESSAY_WORD_MIN_COUNT` 时取门槛值。语料次数少于 `CORPUS_MIN_COUNT` 也照样换算：那条下限挡的是异写和切分偶然，审校过的词不是。词表词本来就按语料加权，这次只是让审校词走同一条路。
- 只拼「咗」，不拼 HKCanCor 别的 `u` 类助词，理由见备选。

## 实测

构建器在 Mac Studio 上对 msime-dictionary `71dbf67`（`dict-v2.1.5`）跑 `languages`：

- 这个构建器配现有的计数文件，产出的 `msime-cantonese.db` 与 `dict-v2.1.5` 发布的逐字节相同（SHA-256 `f9103b08…`），审校词的新权重规则在计数文件换掉之前不改任何产物。
- 换上新生成的计数文件（同时删掉豁免表里过期的 講咗 一行，见 Consequences）：`corpus_scale` 仍是 845，香港字形的替换结果不变（只是 爲→為 的语料次数 505→506、蔘→參 15→16），改权重的条目 24 条，全是「X咗」：去咗 1000→40560、做咗 1000→17745、借咗 1000→4225、買咗 19→21125、死咗 19→16900、識咗 19→7605、食咗 20→3380、有咗 21→2535 等。
- 普通粤拼的 98456 个键里换了首选的 3 个：jau zo 有助→有咗、maai zo 賣座→買咗、sik zo 食咗→識咗（语料 識咗 9 次、食咗 4 次）。双拼的 91629 个码里换了首选的 7 个，都是「X咗」上位：`zbzo` 早在→做咗、`zezo` 這在→借咗、`mlzo` 埋在→買咗、`ykzo` 又在→有咗、`s;zo` 星座→識咗（食咗 第二）、`yxzo` 攘災→約咗、`ptzo` 劈咗→撇咗。用引擎直接打开这份库查独立双拼入口：`zbzo` 是 做咗(17745) 早在(2121)，`zezo` 是 借咗(4225) 這在(1105)，`hyzo` 是 去咗。
- 两字「X咗」不是自己双拼码首选的从 36 条降到 30 条。剩下的有三类：语料没数到或只数到一两次（試咗 0、神咗 0、耐咗 1）；同码上是常用词（實在 压 神咗、正在 压 直咗、轉載 压 轉咗）；同码上是另一个「X咗」（食咗 在 識咗 之后）。这些不强行抬到首位，粤语学习在用户选过一次后会把它提上来；母语者认为某个词就该在前面时，可以走审校表，审校词现在也吃语料次数。

## Alternatives considered

- **审校表加权重列（`词<TAB>粤拼<TAB>权重`）。** 最强的理由是精确：维护者一行一个数，借咗、做咗 立刻上首位，语料数不到的 試咗 也能救。不采用：数字没有数据来源，审校者只能对着同码的对手猜一个刚好压过它的值，msime-dictionary 的 AGENTS.md 也不许为了靠前填大权重；`custom/` 由 msime-web 和 msime-cloud 直接读写，改行格式要三个仓一起改，check-words 也要跟；而且只修写进去的那几个词，词表里的 買咗、死咗 照旧。
- **引擎里让模糊韵母键优先取「主」韵母（`o` 先于 `oi`），只在同长度整码行之间。** 最强的理由是不动数据、立刻生效。不采用：这套方案没有主次之分，`o` 键上的 `o`、`oi` 与 `a`/`aa`、`an`/`at`、`un`/`ung`/`eon` 一样是同等地放在一个键上；按 `dict-v2.1.5` 实测，含 `o` 韵母键的 13507 个码里 212 个换首选，從來 让给 巡邏（`cplo`）、打開 让给 打河（`daho`）、菜刀 让给 銼刀（`codb`）；它也修不了 食咗（压它的 星座 也是 `o` 韵母）和普通粤拼里的 買咗。
- **把 這在 写进粤拼否决表。** 否决表只收错字、错写这类根本不该出现的形式，這在 是正常的书面语片段；早在 更是常用词，否决不了，做咗 照样是第二。
- **HKCanCor 的 `u` 类助词全部拼合（嘅、到、下、啲、過、得、住、晒、返、埋……）。** 最强的理由是同一个切分问题一并解决：寫住、留返、做返、傾到、企得 这类豁免表里的「动词带补语」有了佐证，豁免表能删 12 行。不采用：实测改权重的条目 323 条，`corpus_scale` 从 845 变成 800，全部 332 个按语料加权的词跟着降约 5%；essay 补进来的碎片也拿到了佐证，返返 压过 犯法（`fjfj`）、睇過 压过 太過（`tlio`），黐住 与 癡住 换位。这些助词要逐个看它拼出来的是不是词，留作后续；「咗」几乎只作完成体助词，拼出来的都是「动词加咗」。
- **照 issue 原话，所有「X咗」一律排到自己键的首位。** 最强的理由是报告人要的就是这个。不采用：没有数据支持，会把 神咗 排到 實在 前面、直咗 排到 正在 前面、轉咗 排到 轉載 前面，这些都是更常打的词。
- **只抬高审校词的门槛（比如审校词至少 2500）。** 最强的理由是一行代码。不采用：2500 只是刚好压过 早在 的数，换一个对手就不够；对 essay 已列、分数不高的审校词（周杰倫 711）也说不清该不该一起抬。

## Consequences

- 生效要两步跨仓。msime-dictionary 那边用这个生成器在原路径整份重新生成 `sources/cantonese/hkcancor-word-counts.txt`、用 `scripts/check_upstream.py --update` 刷新 `upstream.lock.json`，并删掉 `curation/cantonese-essay-keep.txt` 的 講咗 一行：講咗 现在有 HKCanCor 佐证（15 次），不再是会被压低的 essay 补词，留着这一行构建会失败；删掉后它照旧保留 essay 的 8809 分。只换计数文件、仍用旧构建器时，词表里的「X咗」已经按语料加权，审校词（做咗、借咗、去咗）还停在 1000；要等 `builder.lock.json` 升到包含这次改动的 msime 提交才全部生效。新数据进应用还要 `resources/language-dictionaries.lock.json` 换到那次发布（现在固定的是 `dict-v2.1.2`，连 借咗、做咗、去咗 都还没有）。所以 msime 这边的 PR 只写 `Refs #10909`。
- 两步都已做完：msime-dictionary #135 把 `builder.lock.json` 升到本 PR 的合并提交 `dd32cafd71` 并用它重新生成计数文件（只有表头的生成器提交变了），合入后发布 `dict-v2.1.6`；`resources/language-dictionaries.lock.json` 随后换到 `dict-v2.1.6`（`scripts/resource_lock_from_sums.py` 从发布的 `msime-language-dictionaries-SHA256SUMS` 生成，`source_commit` 记构建器提交 `dd32cafd71`）。与 `dict-v2.1.2` 相比，`msime-cantonese.db` 新增 6678 条（#132 的审校词和 #133 的香港地名，`license` 变成 `CC-BY-4.0 AND LicenseRef-HK-CSDI`），改权重 21 条；按完整双拼码取最重的词，`zezo` 首选 借咗（4225，這在 1105），`zbzo` 首选 做咗（17745，早在 2121）。注音、笔画库逐表内容相同，只是文件字节不同。
- 计数文件多了「拼出来的词」，对 `languages` 里所有读它的地方都成立：词表词的语料权重、essay 补词的佐证、`corpus_scale` 的中位数、香港字形的判定。这次实测只有前两项有变化，以后换 HKCanCor 提交时要连这几项一起看。
- 普通粤拼 `sik zo` 的首选从 食咗 换成 識咗，是语料次数决定的（9 比 4），不是有意的取舍。
- 审校词的权重来源多了一个：essay 没列时，同一个词的语料次数会改变它的权重。以后往 `custom/cantonese.txt` 加 HKCanCor 数到过的词，权重可能远高于 1000，审核时要看 `check-curation.yml` 的 `msime-cantonese.db` 差异。
- 验证：`cargo test -p msime-dict-builder --locked`（新加 `hkcancor::tests::a_perfective_particle_is_counted_with_the_token_before_it`、`cantonese::tests::reviewed_words_the_essay_lacks_take_their_corpus_weight_above_the_threshold`）与 `cargo clippy -p msime-dict-builder --all-targets --locked -- -D warnings`，经 rbuild 在 Studio 上跑；上面的实测用真实的 msime-dictionary checkout 构建，Rust 生成器的输出与做测量时用的计数文件正文逐字节相同。没有在模拟器或真机上验收。
- 相关：[粤语人工审校词表](../feature/2026-10-31-cantonese-reviewed-word-list.md)定下了审校词的格式和原来的权重规则；[语言词库清洗](2026-11-04-language-dictionaries-cleanse.md)记了 HKCanCor 把助词切开、講咗 一类词得靠豁免表保留的来由。
