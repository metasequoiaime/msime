# Agent Note: ECDICT 释义以义项位置为主，配上释义成对否决和删除标记

Status: implemented

## Problem

`msime-english.db` 的两张释义表由 `english-glosses` 阶段从 ECDICT 推出（`crates/dict-builder/src/english.rs` 的 `derive_glosses`）。两个方向犯的是同一个错：常用度压过了义项位置。

- 英译中按「通用义项在前，再按中文词在拼音表里的权重」排，于是 page 选了更常用的「事件；记录」而不是排第一的「页」，is 选了 `[计]` 行的「信息系统」，give 选了名词行的「产生」。
- 中译英的分数是英文词的常用度（0 到约 73 万）加一个最多 3 万的位置分，位置几乎不起作用：影响 译成 win; reach（win 第二行第三项、reach 第二行第五项），一点 译成 ghost; damn（ghost 的第五项，damn 的 not a damn），根据 译成 from; grind。两次独立抽样估计 zh_en 误译率约 15–17%。
- 领域标签只认行首的，`n. [俚] 黑鬼` 这种写在词性后面的标签被当成通用义项；`[网络]` 行能进中译英，那些行多是网上抓来的音译或错译（xxxvii 是「炒杂碎」，pussy 是「小猫」）。中译英的英文词一律小写（china、english、monday）。
- 冒犯释义：普通词配上冒犯的英文或中文（深棕色 nigger、黑人 negro、杀价 jew、natural「白痴」），没有办法按对删掉，`custom/translations.txt` 也只能改、不能删。
- `sources/english/rime-ice-en-supplement.txt` 不是现在的生成器写的：生成器的对照集合按每行第一个词取、不算注释行，rime-ice 在 `rime-ice-en.txt` 里注释掉的 na、ci、js、qa 等 29 个词被补充表加了回来。

## Decision

### 英译中：行内按位置，行与行之间按首项常用度

ECDICT 一行之内的义项按常用程度排，行与行之间是按词性分的，而且常把名词行放在最前（give「n. 弹性」、keep「n. 生计」、natural「n. 白痴」），所以位置只能在行内说话。`order_terms`：

- 每行去掉行首和词性后面的领域标签，带标签的行是领域义项；只有含「计」字的标签还能进中译英，`[网络]` 不能。同一个词在不同的行里各算一次（keep 的「保持」在名词行和动词行都有）。
- 行按（领域与否，首项权重，行号）排：首项权重取这一行第一个义项在拼音表里的权重，第一条通用行乘 `FIRST_LINE_BIAS`（10），别的行首项要常用十倍以上才排到前面。行内按位置，权重低于 `RARE_TERM_WEIGHT`（1000）的词排到本行后面（may 的「愿能」810）。
- 取前两项；第一项是通用义项时只留通用义项（TV 不再配「转移向量」）。
- 没有通用义项、只有屈折说明的词（is 只有「be的现在式第三人称」和一行 `[计]`，are、was 原来没有释义）用说明里的原形的义项。

### 中译英：位置衰减的常用度，主行之外再打折，位置太后不给

每个（中文义项，英文词）的分数是英文词的常用度（`vocabulary_quality`）乘 `0.7^步数`，步数是行内序号，不在这个英文词的主行（`order_terms` 排出的第一项所在的行）上再加 1。取最高分，第二个英文词要有它的 60%（原来是 45%）。最高分的步数超过 `MAX_REVERSE_STEPS`（3）时这个中文词不给译文：排在第五位以后的义项多是成语或引申义的碎片，抽样里这样留下来的释义约一半是错的。其余规则：

- 屈折形式的领域义项不记到原形名下（is 的 `[计]` 是缩写 IS，不该成为 be 的「信息系统」）；大写词头（States）不按 `exchange` 换成小写原形（state）。
- 显示写法：ECDICT 词头不是小写时保留它的大写；`english_words` 也有小写写法时，只有词头的第一个义项和以它开头的义项用大写（China 的「中国」「中国的」，china 的「瓷器」）。释义只用来显示，所以大写写法不必在 `english_words` 里。

### 两张维护者表和一个删除标记

- `curation/gloss-vetoes.txt`（`curation::GlossVetoes`）：`中文<TAB>英文`，ECDICT 给这个英文词（小写；屈折形式代表原形时按原形）列的这个中文义项两个方向都不用。一对匹配不到任何候选词的义项时构建失败，与其余 `curation/` 表一样（[curation 表的笔记](2026-11-04-dictionary-curation-tables.md)）。
- `curation/english-vetoes.txt` 沿用 #10858 的机制，第一批数据由本次调查给出（词库仓库那边）。
- `custom/translations.txt` 的译文写成单独一个 `-` 时，`apply_custom_translations` 删掉这个源词的释义；源词本来没有释义不算错，因为 `custom/` 只能追加，ECDICT 换版本后删除行不能跟着撤。

### rime-ice 英文补充表

`rime_ice_supplement::english_compared` 用 `english` 阶段的同一套解析器取对照集合（多词短语不算成首词，否则 generative、quantumult 会被 `Generative Pre-trained Transformer GPT` 这类行挡掉），并把 `rime-ice-en.txt` 注释掉的 `# 词 编码` 条目算进去（复用 `english_supplement::commented_base_dict_words`）。表头第 1 行改成「…由 msime 仓库提交 X 的 … 以 `…` 生成；不要手工编辑」，与其他生成表一致，词库仓库的 `check_upstream.py` 才能认出 `-dirty`。

## Alternatives considered

- **纯按位置（行号、项号）选英译中。** 最强的理由是最符合「位置为主」、不依赖拼音表。不采用：ECDICT 的行是按词性分的，给前 300 个常用词跑一遍，give 成了「弹性；适应性」、want「缺乏；贫困」、still「寂静；剧照」、look「一看；神色」，比原来还糟。
- **英译中第一条通用行的优势取 3 倍或 5 倍。** 3 倍时 but、very、many、here、through 对了，但在前 1500 个常用词里两者不同的 69 个词中，3 倍好的约 15 个、10 倍好的约 33 个（eye「眼睛」、mother「母亲」、dog「狗」、class「班级」）。取 10 倍，but「除了；但是」、very「真正的」这几个功能词是已知的残留错误。
- **中译英硬性只收前四项（项号不超过 3）。** 最强的理由是实现最简单。不采用：它把靠后的义项从候选里拿掉，原本位列第一但位置靠后的常用词让位给排第一项的冷僻词，宝贝 成了 Sonny、本人 成了 McCoy、坦克 成了 panzer。改成「第一名位置太后就整条不给」。
- **中译英第二个英文词仍用 45%。** 不采用：内容「content; gut」、可以「could; okay」这类第二个词多是错的，60% 去掉它们，代价是一部分正确的第二译法也没了（可以 只剩 could）。
- **成对否决只管中译英一个方向，或者加一列写方向。** 最强的理由是有些对子的英译中是对的（whore 译「娼妓」、blowjob 译「口交」），双向否决会把它们删掉。不采用：多一列格式就多一种出错方式；改为审表时每一对都核对否决前后两个方向，英译中会因此丢掉正确释义的对子不写进表（口交 blowjob、娼妓 whore、小便 piss、公鸡 cock 留着）。
- **按引擎的粗俗英文词表自动拦：普通中文词不配粗俗英文词。** 最强的理由是不用逐对维护。不采用：要把引擎的英文表和词库的中文粗俗词表接进构建器，判断「普通中文词」又要另一份数据；方案要的是一张可审、可撤的成对表，第一批 56 对足够。
- **`[网络]` 行继续进中译英。** 不采用：在候选词范围里这种行 121 条，抽查多是音译或错译，还带进了 小猫 pussy。英译中仍可用它们（只有这种行的词没有别的释义）。

## Consequences

- 收益：按 dict-v2.0.56 比较（试跑 41837014331，构建器 `f557354b39`、词库 `7a0b98a`），`en_zh_glosses` 66819 → 67362 行，改了 19686 行（首义变了 14425 行），新增 592 行（多是 `n. [机械]硬度降低` 这种词性后带标签、原来整行被丢掉的技术词），删 49 行（否决的词）；`zh_en_glosses` 19682 → 18477 行，改了 4971 行（其中 1288 行只是大小写，January、China、English），删 1208 行（绝大多数是位置太后不给的，其余是否决），新增 3 行。抽样 200 条人工判断（前 50 个常用词和 50 条随机，每个方向各 100 条，判断表在 `_gloss-work/gloss-sample-200-judged.tsv`，不进仓库）：英译中好 32、差 6、相当 62；中译英好 37、差 16、相当 47。
- 代价：仍会错的有两类。功能词的行选择（but「除了；但是」、very「真正的；恰好的」、many「多数；多数人」、about「大约；有关」），以及中译英里功能词被主行打折（但是「only; but」、可能「like; possibly」、关于「regard; concern」、非常「highly; rare」）。1208 条中译英被删，按抽样估计其中约一半原来是对的（终点站 terminal、选段 extract、确实 exactly）；`custom/translations.txt` 可以补回。
- 释义成对否决的条目跟 ECDICT 版本和候选词绑定：ECDICT 升级、英文词表增删、`english-vetoes.txt` 删掉某个词时，相关的对子可能匹配不到而让构建失败，要随之删掉。
- msime-windows 的 `scripts/dictionary_build/english_db.py` 自己实现了一份同样的释义推导和 `custom/translations.txt` 读法，不在本仓库，要另外同步（删除标记、两张 curation 表、打分）。

## Verification

- `crates/dict-builder`：`english::gloss_regression_tests`（ECDICT 原样的 16 行加 dict-v2.0.55 的真实权重：page「页；记录」、is「是；表示」、give「给；授予」、影响「affect; effect」、一点 不译成 ghost、信息系统 不记到 be、中国 / 中国的 China、瓷器 china、美国「States; America」；成对否决两个方向生效、匹配不到时失败），`english::tests` 的标签、屈折说明、行选择、大写和删除标记用例，`rime_ice_supplement::tests::english_compares_with_what_the_build_reads`。`cargo test -p msime-dict-builder` 与 `cargo clippy -p msime-dict-builder --all-targets -D warnings` 在 Studio 上通过。
- 同一套规则写了一份 Python 复刻（`_gloss-work/scripts2/proto2.py`，先对旧规则复刻 dict-v2.0.55 两张表逐条一致），对试跑产物的两张表逐条比较，0 条差异。
- 用改过的生成器从 rime-ice `3aea6d3` 重新生成补充表：词键少了 30 个（rime-ice 注释掉的 29 个加上已在 `custom/english.txt` 的 figma），没有新增，437 个词的显示写法恢复上游的大小写（AdGuard、PyTorch）。
