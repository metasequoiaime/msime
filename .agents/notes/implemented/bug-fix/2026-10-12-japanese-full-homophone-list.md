# Agent Note: 日文完整读音列出全部同音词

Status: implemented

## Problem

#7353 反馈日文罗马字候选太少：`syou` 只有三页左右，大量书面语和 N1 级汉字词打不出来，用户以为是词库太小，请求换装 Mozc 之类的完整词库。

词库本身是完整的：`msime-japanese.dat` 就是 Mozc OSS 系统词库（`crates/dict-builder/src/japanese.rs`），共 1,285,198 条，しょう这个读音有 107 条、68 个不同词面（症 賞 省 証 … 鐘 梢 簫）。缺的是 provider 的列表：`query_into` 只列 12 条 N-best 句子（矩阵每个跨度又只取最便宜的 24 条精确词条）、16 条读音前缀预测，再加平假名和片假名两行。`syou` 因此只有 21 行，六十多个同音词里只有十几个能进列表，其余翻多少页都翻不到。日文也没有翻页展开的路径（`expand_initial_candidates` 对 `JapaneseRomaji` 答 false）。

## Decision

读音完整（没有待定字母）时，`JapaneseProvider::query_into` 在片假名行和云候选插入之后，把这个读音的全部精确词条按 `(cost, id)` 接在列表末尾，权重与预测行相同（`PREFIX_LEMMA_BASE - word_cost`，来源 `Database`），已列出的词面由 `Rows` 原有的去重跳过。单个假名（`ka`）也是完整读音，走同一段：假名行、句子之后是其余同音词。

同音词尾部必须排在云候选之后。云候选插入时会跳过已经列出的词，如果先接同音词，Google 返回的首选只要是这个读音的同音词（句子和预测没列到的那些，正是云候选最该补上的），就会被当成重复丢掉：它既占不到第二位（单个假名时第三位），也失去 `CloudSuggestion` 来源。先插云候选，同音词尾部再按去重跳过它。

接在片假名之后，前面各行一行不变：句子、预测、平假名提到第二位、片假名的位置都和原来相同，第一页的排序和日文评测不受影响。

上限 `EXACT_READING_LEMMAS = 512` 只防异常模型。Mozc 词库最大的读音组是こうじ的 286 条，现有词库里每个读音都能列全。

精确词条用 decoder 新增的 `for_each_exact_lemma_view_reusing` 访问：读音组的 `(cost, id)` 收进调用方传入的缓冲，超过限额时 `select_nth_unstable` 截断，再排序。provider 把这份缓冲作为 `exact_ranking` 字段跨查询复用，热查询不申请排名存储。原有 `for_each_exact_lemma_view` 按限额预留排名堆，限额 512 时每个按键都要申请 4 KB；矩阵仍用原入口，没有改。

## Alternatives considered

- **调大 `SENTENCE_LIMIT` 或矩阵每个跨度的 24 条**：同音词会进 N-best，不用另加一段。代价是每个跨度的精确词条都乘进格子，整句搜索变慢，句子顺序也会变，第一页跟着变，需要重跑日文评测；而且单个跨度的上限仍会截掉生僻词。
- **给日文接 `expand_initial_candidates`，翻到最后一页或打开全部候选时再补同音词**：每个按键不必多列。代价是行为取决于宿主有没有走 input-runtime 的展开路径，展开时 runtime 还会按已显示的行重新排序；而全部同音词只多 10～80 µs（见 Consequences），没有必要分两步。
- **完整读音的同音词排在较长的预测之前**：用户打完整读音时更可能要的是同音词。但这会改变现有第一页和评测结果，属于排序决策，不在这个缺陷修复里做。
- **直接用 `for_each_exact_lemma_view`，不加新入口**：少一个 decoder 方法和一个 provider 字段。代价是每个完整读音的按键都按 512 预留一次排名堆，与 provider 已有的零分配预算方向相反。[精确词条流式消费](../../implemented/testing/2026-10-09-japanese-exact-lemma-stream.md)当时否掉常驻缓冲，理由是要管理词库视图的生命周期；这里缓冲只存 `(cost, id)`，不借用词库，没有那个问题。

## 既有笔记审计

[罗马字拼法与平假名位置](../../implemented/bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md)部分重叠：平假名最晚在第二位、片假名位置不变，本篇保持这两条，并把新行放在片假名之后。[精确词条流式消费](../../implemented/testing/2026-10-09-japanese-exact-lemma-stream.md)与[排名堆成本键复用](../../implemented/testing/2026-10-10-japanese-ranked-heap-output.md)约束矩阵用的精确入口，本篇没有改动它，新入口沿用同一 `(cost, id)` 总序，测试逐条对照。[完整读音预测流式消费](../../implemented/testing/2026-10-09-japanese-prediction-stream.md)和[句子结果容器复用](../../implemented/testing/2026-10-09-japanese-sentence-output-buffer.md)约束前面两段的分配，相关测试的分配预算没有变化。其余日文笔记管罗马字扫描、待定前缀和矩阵存储，与本篇无关。

## Verification

修复前新增的合成测试失败：40 个同音词只列出 15 个，单假名的 30 个同音词只列出 12 个。云答案是尾部同音词的合成测试（`ka` 的云答案为第 21 个同音词，`syou` 为第 31 个）在同音词先于云候选接上时失败，云答案只作为 `Database` 行留在尾部；改为先插云候选后，它在第三位或第二位、来源为 `CloudSuggestion`，尾部不重复。修复后 `syou` 的合成列表逐行等于原列表接上其余 25 个同音词（按成本），`ka` 列出假名行和全部 30 个。decoder 的新入口对 300 条同读音的合成词库，逐字段对照 `exact_lemma_views` 的 `(cost, id)` 顺序，覆盖空读音、未命中、限额 0/1/24/299/300/512，缓冲容下整组后再查不申请。

真实词库（`MSIME_EVAL_RESOURCES` 指向 Mozc 词库所在目录）：`syou` 从 21 行变为 75 行，鐘、梢、簫都在列表里，平假名仍在第二位；`kouji` 从 26 行变为 274 行。release 下热查询每次耗时的最优批次：`syou` 132→146 µs，`kou` 227→248 µs，`kouji` 34→111 µs，长句 `watashihagakuseidesu` 不变（精确范围为空）。

`cargo test -p msime-engine` 全部通过（1841 项、golden 31 项），`cargo clippy -p msime-engine --all-targets -- -D warnings` 与 `cargo fmt --check` 通过。

## Consequences

日文完整读音能选到词库里的每个同音词，所有平台随引擎一起拿到修复，宿主代码不变。#7353 不需要外接词库：词已经在 Mozc 词库里，只是排不进列表。

代价是完整读音的列表变长，候选行从二三十行变成最多几百行（こうじ 274 行），引擎快照里交给宿主的候选也随之变多；这个长度与拼音展开后的列表同一量级。`Rows::contains_word` 的线性去重在这个长度下是平方级，こうじ多出约 75 µs，常见读音 10～20 µs；若以后整张列表还要更长，再换成按词面哈希去重。新行接在片假名之后，生僻同音词总在第三页以后；要不要让同音词排在较长的预测之前，是另一个排序决定。用户词典导入和按分段选词也是另外的功能，不在本篇范围。
