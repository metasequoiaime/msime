# Agent Note: 九键英文候选在数字变长时复用前缀行

Status: implemented

## Problem

iPhone XR 上九键每打一位的引擎耗时离一帧 16 ms 还差一截。在 macOS 上对 `ninekey_latency` 采样，`english_candidates` 占一次刷新的 20.9%（包含子调用），几乎全花在 `read_prefix_rows` 上。

字母前缀组只由前几位数字决定：九键每键三到四个字母，预算 64 个前缀，到第三位就封顶。此后每多打一位，仍要对同一组最多 64 个前缀各跑一条 `KEYPAD_PREFIX_SQL`。每一条都要扫描整个前缀区间、逐行做 GLOB，再排序取前五。可是数字变长只会让匹配的词变少，上一位已经筛过的行，这一位又从头筛了一遍。

## Decision

`NineKeySession` 增加 `english_rows: Option<EnglishKeypadCache>`，记下上一次英文查询时的数字串、前缀组深度（`letter_prefix_depth`），以及每个前缀下匹配整串数字的全部行。

- **何时建**：同一组前缀第二次出现时才取全部行，用的是 `EnglishDictionary::keypad_prefix_rows`。这条 SQL 与 `KEYPAD_PREFIX_SQL` 筛选相同、没有 `LIMIT 5`，按排序键中与 typed 无关的部分排好。九键前三位每打一位前缀组都会变，第一次出现就取全部行是白取。
- **何时复用**：键盘、深度相同，且新数字串以缓存的数字串开头时，用 `word_spells_from` 只检查新增的那几位，把不再匹配的行筛掉，不查 SQLite。
- **怎么取前五**：`append_keypad_ranked` 先取展示正好 `typed` 字节且权重为正的行，再取其余行。SQL 排序的首项就是这一个二分，其余键的顺序已经由取行时的 `ORDER BY` 定好，所以结果与逐条 SQL 逐行相同。`length(CAST(display AS BLOB))` 和 `weight>0` 都由 SQL 算出带回，不在 Rust 里重新解释。
- **何时退回 SQL**：任一前缀超过 `ENGLISH_CACHED_PREFIX_ROWS`（2048）行，或读取出错，就不建缓存，照旧逐条查询。词库里最大的三字母前缀 `con` 有 1034 个词。

## 失效契约

缓存是按数字内容复用的：数字串只要以缓存的数字串开头，正确性就不取决于它是怎么变成这样的，退格、在光标处插入、撤销都一样。它只在词库内容变了时会过时。组字过程中会改英文权重的只有选词学习，而选词之后一定经过 `consume` 或 `clear_composition`，这两处都会把 `english_rows` 置空。

以后新增任何会在组字中途写 `english_words` 的路径，都必须同样清掉缓存。

## 测得的效果

在 Mac Studio 上，对同一基点 `ac4a4c9cae` 运行 `ninekey_latency`，计 instructions retired（4 次平均）：从 398.8 亿降到 368.9 亿，减少 7.5%。

`convert_eval` 在九键和十四键上导出的全部候选，与基线逐行相同；比较前先剔除了随墙钟变化的时间日期候选。

## Alternatives considered

- **第一次出现前缀组就取全部行**：实现更简单。可是九键前三位每打一位前缀组都变，取了马上就扔；第二位时一组前缀就有几千行，例如 `26` 下匹配的有 4340 行。
- **从上一层前缀的行派生下一层**：例如用 `co` 的行筛出 `con`，这样前三位也能复用。但排序键里有一项 `word = 前缀`，前缀一变，预排好的顺序就不再成立，需要重新排序；而且第二层的行数约是第三层的两倍（`26` 下 4340 行，`266` 下 1958 行），派生省下的那几次查询，抵不过多存、多筛的这些行。
- **每次在内存里对全部行整体排序**：直接照搬 SQL 的七个排序键。每一位都要对几百行重新排序；而预排序之后只需要按首项做一次稳定二分，线性扫描就够。

## Consequences

收益：九键第四位起，英文候选不再访问 SQLite。用真实词库随机连打 3600 次刷新，有 1719 次走缓存。

代价：
- 建缓存那一位要多分配：全部匹配行各两个 `String`，第四位时通常是几百行。
- 会话里多存一份这些行，直到下一次数字不再是延长时被替换。

测试：
- `english_cache_tests` 在密集的伪随机词库上，用 DFS（覆盖改写后作废）和随机连打（覆盖复用）对照逐前缀查询，逐行比较结果和容量，九键和十四键各一组。
- 另有一个被 `#[ignore]` 的测试，用 `MSIME_ENGLISH_DICTIONARY_DIR` 指向真实词库做同样的对照。
- `english_page_append_tests` 量的是逐前缀查询路径的分配。测量前把缓存的深度改成对不上，让它仍然走那条路径。

相关：
- [九键英文前缀只物化最终层](../testing/2026-10-18-nine-key-english-prefix-generation.md)：那里的深度计算现在由 `letter_prefix_span` 承担，供缓存比对深度用。
- [九键英文候选借用已输入数字](../testing/2026-10-16-nine-key-english-digits-borrow.md)、[九键英文空结果延后缓冲](../testing/2026-10-10-nine-key-english-lazy-buffer.md)：只约束查询入口的输入借用和结果容量，缓存路径照样遵守。
