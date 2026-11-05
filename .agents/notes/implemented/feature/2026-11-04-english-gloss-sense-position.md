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

### 英译中：行内按位置，行与行之间按首项常用度，冷僻义项后移

ECDICT 一行之内的义项大体按常用程度排，行与行之间是按词性分的，而且常把名词行放在最前（give「n. 弹性」、keep「n. 生计」、natural「n. 白痴」），所以位置只能在行内说话。`order_terms`：

- 每行去掉行首和词性后面的领域标签，带标签的行是领域义项；只有含「计」字的标签还能进中译英，`[网络]` 不能。同一个词在不同的行里各算一次（keep 的「保持」在名词行和动词行都有）。
- 行按（领域与否，首项权重，行号）排：首项权重取这一行第一个义项在拼音表里的权重，第一条通用行乘 `FIRST_LINE_BIAS`（10），别的行首项要常用十倍以上才排到前面。
- 冷僻义项按行的先后排在所有常用义项后面。冷僻的判据是权重低于 `RARE_TERM_WEIGHT`（1000），或不到这个词最常用的通用义项的 1/`RARE_TERM_RATIO`（50）：post 的「柱」「杆」「准星」不到「岗位」的 1/50，school 的「鱼群」「门派」，may 的「五月」「愿能」。「X的」形容词大多不在拼音表里，按它和 X 中较高的权重算（`rarity_weight`：natural 的「自然的」看「自然」），否则它们会全被当成冷僻义项。
- 取前两项；第一项是通用义项时只留通用义项（TV 不再配「转移向量」）。上面的排序分两步：先排通用行与领域行，再全局后移冷僻义项；因此所有通用义项都冷僻而领域义项常用时，领域义项可以领先。审查中用合成输入复核了这个边界，本次只澄清注释，不改排序行为。
- 没有通用义项、只有屈折说明的词（is 只有「be的现在式第三人称」和一行 `[计]`，are、was 原来没有释义）用说明里的原形的义项。

### 中译英：位置衰减的常用度，主行之外再打折，位置太后不给

每个（中文义项，英文词）的分数是英文词的常用度（`vocabulary_quality`）乘 `0.7^步数`，步数是行内序号，不在这个英文词的主行（`order_terms` 排出的第一项所在的行）上再加 1。取最高分，第二个英文词要有它的 60%（原来是 45%）。最高分的步数超过 `MAX_REVERSE_STEPS`（3）时这个中文词不给译文：排在第五位以后的义项多是成语或引申义的碎片，抽样里这样留下来的释义约一半是错的。其余规则：

- 屈折形式的领域义项不记到原形名下（is 的 `[计]` 是缩写 IS，不该成为 be 的「信息系统」）；大写词头（States）不按 `exchange` 换成小写原形（state）。
- 显示写法只从 ECDICT 词头和 SCOWL 拼写词典来（`display_for`），不借 `english_words` 的写法：词头是小写就显示小写；词头有大写时，SCOWL 只收小写词形的显示小写（ECDICT 把 core、fax、maxim 的词头写成 CORE、FAX、Maxim），只收词头写法的显示词头（Africa、Monday，`english_words` 里可能只有 africa），两种都收时只有词头的第一个义项和以它开头的义项用大写（China 的「中国」「中国的」，china 的「瓷器」）；SCOWL 两种都不收时（缩写、品牌）看 `english_words` 有没有小写写法。SCOWL 包已经固定在构建输入里（`english` 阶段用它定领头的写法），这里读同一份。

### 两张维护者表和一个删除标记

- `curation/gloss-vetoes.txt`（`curation::GlossVetoes`）：`中文<TAB>英文`，ECDICT 给这个英文词（小写；屈折形式代表原形时按原形）列的这个中文义项两个方向都不用。一对匹配不到任何候选词的义项时构建失败，与其余 `curation/` 表一样（[curation 表的笔记](2026-11-04-dictionary-curation-tables.md)）。第一批 56 对是冒犯配对；后加的 9 对是高频中文词里 ECDICT 本身列错、打分改不动的配对（建议 instance、可能 like、知道 wise、利用 impose、非常 rare、时间 when、认为 account、导致 account、行业 calling），每一对都核对过两个方向。
- `curation/english-vetoes.txt` 沿用 #10858 的机制，第一批数据由本次调查给出（词库仓库那边）。
- `custom/translations.txt` 的译文写成单独一个 `-` 时，`apply_custom_translations` 删掉这个源词的释义；源词本来没有释义不算错，因为 `custom/` 只能追加，ECDICT 换版本后删除行不能跟着撤。词库仓库固定的构建器（`builder.lock.json`、`check-words.yml` 的 `MSIME_COMMIT`）和 msime-windows 的 `english_db.py` 都支持之前，词库里不能加这种行：旧构建器会把 `-` 当成一条译文发布。

### rime-ice 英文补充表

`rime_ice_supplement::english_compared` 用 `english` 阶段的同一套解析器取对照集合（多词短语不算成首词，否则 generative、quantumult 会被 `Generative Pre-trained Transformer GPT` 这类行挡掉），并把 `rime-ice-en.txt` 注释掉的 `# 词 编码` 条目算进去（复用 `english_supplement::commented_base_dict_words`）。表头第 1 行改成「…由 msime 仓库提交 X 的 … 以 `…` 生成；不要手工编辑」，与其他生成表一致，词库仓库的 `check_upstream.py` 才能认出 `-dirty`。

整份重新生成后补充表按上游写大小写（AdGuard、PuTTY、Playwright），而 SCOWL 补充表按小写词形对照补充表，不收 putty、playwright、musk 这些本身是普通英文词的小写词形，于是这 18 个词在 `english_words` 里只剩大写写法。`english` 阶段用 `english::add_attested_lowercase` 给补充表里只有非小写写法、SCOWL 收了小写词形的词补上小写写法，`leading_display` 让小写领头。

## Alternatives considered

- **纯按位置（行号、项号）选英译中。** 最强的理由是最符合「位置为主」、不依赖拼音表。不采用：ECDICT 的行是按词性分的，给前 300 个常用词跑一遍，give 成了「弹性；适应性」、want「缺乏；贫困」、still「寂静；剧照」、look「一看；神色」，比原来还糟。
- **英译中第一条通用行的优势取 3 倍或 5 倍。** 3 倍时 but、very、many、here、through 对了，但在前 1500 个常用词里两者不同的 69 个词中，3 倍好的约 15 个、10 倍好的约 33 个（eye「眼睛」、mother「母亲」、dog「狗」、class「班级」）。取 10 倍，but「除了；但是」、very「真正的」这几个功能词是已知的残留错误。
- **英译中行的权重改用行内前两三项的最大值或总和。** 最强的理由是 state 的名词行首项「州」不常用，按首项排会让「说明；陈述」那一行领先。不采用：前 600 个常用词里 want 成了「缺乏」、open「展开」、out「在外」、good「善行；好处」，坏的比好的多。
- **英译中按 ECDICT 的 WordNet 释义（`definition` 列）统计词性，给最常见词性的那一行加优势。** 不采用：那一列截断在约 4 条释义，而且按名词、动词、形容词、副词的顺序排，good、well 都数出 n 最多。
- **冷僻义项用绝对门槛（2 万、5 万）。** 最强的理由是简单，常用词上效果和相对门槛接近。不采用：冷僻英文词的中文义项大多不常用，5 万的门槛改了 15129 个词的英译中，随机抽查里 generous 成了「大量的」、brass「支付」、israel「犹太人」、platform「论坛」；相对门槛（1/50）只改 6938 个，常用词上更好（right「正确的」、game「游戏」、march「进行」、america「美国」）。1/20、1/100 都比较过，1/50 改得最少、错得也最少。
- **冷僻义项后移时保护主行的首项不动。** 最强的理由是 ghost 的「鬼」只有 2.8 万、不到「一点」的 1/50，被挪到后面，ghost 成了「灵魂；一丝」。不采用：前 3000 个常用词里首义因此不同的 242 个，保护首项时 may 回到「五月」、post「柱」、right「权利」、main「主要部分」、film「薄膜」，坏的远多于好的；ghost 是已知的残留错误。
- **中译英不在主行的义项不再多打一折（只按行内序号）。** 这是审查给的建议。最强的理由是 非常 会译成 very、但是 译成 but、影响 译成 influence、状态 译成 state。不采用：拼音权重前 400 个中文词里同时坏了 开始 first、出现 offer、产生 mother、支持 side、找到 hit：ECDICT 给 mother 列「vt. 产生」、first 列「n. 开始」，次要词性行的首项一旦不打折就压过真正的译法。very、mother 的结构完全一样，没有数据能把它们分开。
- **中译英按英文词频的名次加权（常用度乘 (50000/名次)^α，α 取 0.1、0.2、0.3）。** 最强的理由是 use 比 impose 常用得多，利用 应该译成 use。不采用：前 400 个中文词里好坏大致相抵（非常、美国、建议、状态 好了，回答 respond、训练 coach、获得 get; buy、记录 record; page、收益 income; win 坏了），常用的英文词会压过义项并不对口的位置。剩下的高频错配交给成对否决表，有出处、可撤。
- **中译英硬性只收前四项（项号不超过 3）。** 最强的理由是实现最简单。不采用：它把靠后的义项从候选里拿掉，原本位列第一但位置靠后的常用词让位给排第一项的冷僻词，宝贝 成了 Sonny、本人 成了 McCoy、坦克 成了 panzer。改成「第一名位置太后就整条不给」。
- **中译英第二个英文词仍用 45%，或者要求第二个词的位置不超过 1 步。** 不采用：45% 时 内容「content; gut」、可以「could; okay」这类第二个词多是错的；限位置会把 后面 rear; back、它们 they; them、可是 though; however 这些对的第二译法也去掉。
- **中译英显示时要求词头的大写写法也在 `english_words` 里。** 审查提的可选做法，能让 Maxim 显示成 maxim。不采用：rime-ice 把很多地名、国名写成小写（africa、beijing、europe），要求它会让 非洲 显示成 africa。改用 SCOWL 判断，Maxim 照样是小写。
- **补充表的大小写问题改在生成器里解决：SCOWL 补充表按写法（区分大小写）对照补充表，重新生成 `scowl-words.txt`。** 最强的理由是不用在构建时补写法。不采用：两个生成器互为对照，按写法对照后谁先生成谁说了算；重新生成 `scowl-words.txt` 还会带进 `custom/english.txt` 这段时间的变化，putty 也会因此成为「只有 SCOWL 带来的词」而退出中译英。构建时按 SCOWL 补小写写法只动这 18 个词。把同一规则推广到 `rime-ice-en.txt` 会再给 bob、discord、gecko 等 122 个词加小写写法、并让小写领头，那是另一个决定，没有做。
- **把补充表恢复成旧的全小写写法，只删 rime-ice 注释掉的词。** 不采用：旧文件不是生成器写的，恢复它就又回到改不动、查不出来源的状态；上游的大小写（AdGuard、PyTorch、CUDA）也确实更对。
- **成对否决只管中译英一个方向，或者加一列写方向。** 最强的理由是有些对子的英译中是对的（whore 译「娼妓」、blowjob 译「口交」），双向否决会把它们删掉。不采用：多一列格式就多一种出错方式；改为审表时每一对都核对否决前后两个方向，英译中会因此丢掉正确释义的对子不写进表（口交 blowjob、娼妓 whore、小便 piss、公鸡 cock 留着）。
- **按引擎的粗俗英文词表自动拦：普通中文词不配粗俗英文词。** 最强的理由是不用逐对维护。不采用：要把引擎的英文表和词库的中文粗俗词表接进构建器，判断「普通中文词」又要另一份数据；方案要的是一张可审、可撤的成对表。
- **`[网络]` 行继续进中译英。** 不采用：在候选词范围里这种行 121 条，抽查多是音译或错译，还带进了 小猫 pussy。英译中仍可用它们（只有这种行的词没有别的释义）。

## Consequences

- 收益：按 dict-v2.0.56 比较（试跑 41844429132，构建器 `a34e91a42f`、词库 `579fb98`），`en_zh_glosses` 66819 → 67362 行，改了 21336 行（首义变了 16123 行），新增 592 行（多是 `n. [机械]硬度降低` 这种词性后带标签、原来整行被丢掉的技术词），删 49 行（否决的词）；`zh_en_glosses` 19682 → 18475 行，改了 4464 行（其中 794 行只是大小写，January、China、English），删 1210 行（绝大多数是位置太后不给的，其余是否决），新增 3 行。
- 人工审阅：拼音权重前 400 个中文词里中译英变了 241 个，新的更好 55、更差 16、相当 170；英文词频前 300 个词里英译中变了约 170 个，更好 54、更差 18、相当约 98。前 1000 的完整新旧对照在 `_gloss-work/review-zh_en-top1000.tsv`、`review-en_zh-top1000.tsv`（不进仓库），要维护者过目签字。较早的随机抽样（200 条，修正前的规则）：英译中好 32、差 6，中译英好 37、差 16。
- 代价：仍会错的有几类。功能词的行选择（but「除了；但是」、very「真正的；恰好的」、many「多数；多数人」、about「大约；有关」、state「说明；陈述」），冷僻义项后移误伤单字首项（ghost「灵魂；一丝」），中译英里功能词被主行打折（但是「only; but」、关于「regard; concern」、由于「now; after」），以及 美国「States; America」。1210 条中译英被删，按抽样估计其中约一半原来是对的（终点站 terminal、岗位 post、对应 correspond）；`custom/translations.txt` 可以补回。ECDICT 词头全大写、SCOWL 两种写法都收的词还会显示成大写（大桶 VAT、小精灵 ELF）。
- 补充表不再收 rime-ice 注释掉的 29 个词，其中有普通英文词和专名（elk、termed、supplemented、unveils、Oslo、UNESCO、Turin、MRI、CMOS、RFID、UML），它们在 `english_words` 里一种写法都不剩。这和 SCOWL 补充表尊重 rime-ice 注释的规则一致，要不要用 `custom/english.txt` 加回来由维护者定。
- 释义成对否决的条目跟 ECDICT 版本和候选词绑定：ECDICT 升级、英文词表增删、`english-vetoes.txt` 删掉某个词时，相关的对子可能匹配不到而让构建失败，要随之删掉。
- msime-windows 的 `scripts/dictionary_build/english_db.py` 自己实现了一份同样的释义推导和 `custom/translations.txt` 读法，不在本仓库，要另外同步（删除标记、两张 curation 表、打分、显示写法）。

## Verification

- `crates/dict-builder`：`english::gloss_regression_tests`（ECDICT 原样的 22 行加拼音表的真实权重：page「页；记录」、is「是；表示」、give「给；产生」、post「邮件；职位」、school「学校；学派」、影响「affect; effect」、信息系统 不记到 be、中国 / 中国的 China、瓷器 china、美国 两个词都大写、淡灰色 putty、公羊 ram、签证 visa、格言 maxim；成对否决两个方向生效、否决 damn 后 一点 不给译文、匹配不到时失败），`english::tests` 的标签、屈折说明、行选择、冷僻义项、「X的」看 X、大写、补小写写法和删除标记用例，`rime_ice_supplement::tests::english_compares_with_what_the_build_reads`。`cargo test -p msime-dict-builder` 与 `cargo clippy -p msime-dict-builder --all-targets -D warnings` 在 Studio 上通过。
- 同一套规则写了一份 Python 复刻（`_gloss-work/scripts3/`，先对旧规则复刻试跑 41837014331 的两张表逐条一致），对试跑 41844429132 的两张表逐条比较，0 条差异。
- 用改过的生成器从 rime-ice `3aea6d3` 重新生成补充表：词键少了 30 个（rime-ice 注释掉的 29 个加上已在 `custom/english.txt` 的 figma），没有新增，437 个词的显示写法恢复上游的大小写（AdGuard、PyTorch）；试跑里 `english` 阶段报告给其中 18 个词补了小写写法（putty、playwright、musk、sequoia、vandal 等），`english_words` 74160 → 74067 行。
