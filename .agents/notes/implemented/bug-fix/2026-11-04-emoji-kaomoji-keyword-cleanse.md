# Agent Note: 清洗表情与颜文字关键词，加拒绝表防回潮，因为、所以 暂不作符号名称

Status: implemented

## Problem

词库大清洗方案 §5 第 2.2 项和 §5 末尾的两件零碎事。这一区原来没有经过独立复核，这次在 dict-v2.0.55 的 `msime-others.db` 和同版 `msime-pinyin.db` 上重新核对，问题都复现了：

- **粗口关键词**：`resources/dictionary-sources/emoji/emoji.txt` 里 鸡巴 → 🐔、操你妈 → 🤬、草泥马 → 🦙 等 11 行，`kaomoji/kaomoji.txt` 里 剪jj、jian jj 两行（对应的颜文字画的是阉割）。在 E 模式或混排里打这些码会出表情。
- **颜色写反**：Unicode 名称里 black 指实心、white 指空心，表里 ◼ ◾ ▪ 写成了「白色」，◻ ◽ ▫ 写成了「黑色」；◾ ◽ 是中小方块，却写成了 大方块。
- **多音字**：关键词的拼音由 `pinyin` crate 逐字给出，26 个关键词拼出的键在拼音主库里没有这个词的这个读音（长裙 zhangqun、男大夫 nandafu、佛得角 fudejiao、尼泊尔 nipoer、调高 diaogao），再加 八爪鱼 的 bazhaoyu 只有 bazhuayu 权重的 0.5%。
- **`#` 解析**：`others.rs` 把 `#` 开头的行一律当注释。`emoji_catalog.txt` 第 1560 行 `#️⃣\tSymbols\t1558` 因此被丢掉，#️⃣ 只靠关键词表补进来，按「目录外的 Symbols」排在第 1935 位（最后一个）；`emoji_en.txt` 的英文关键词 `#` 也丢了，而对应的 *️⃣ 有 `*`。符号表（词库仓 `sources/symbols/piliapp-symbols.txt`）的 `parse_piliapp` 也是 `#` 开头就跳过，项目清单分类第 785 行的 #️⃣ 因此不在 `symbol_catalog` 里，同一分类的 *️⃣ 却在。
- **符号名称**：词库仓 `sources/symbols/symbol-names.tsv` 在 83186b6 照 MSIME-Windows#928 收了 因为 → ∵、所以 → ∴。msime 的 `resources/symbol-tables.lock.json` 还停在更早的 1af243c，引擎副本里没有这两行；一旦升锁，打 `yinwei`、`suoyi` 时符号紧跟在词后面排第 2，打 `yw`、`sy` 时也会挤进第一页。
- **birds 包**：`packs/birds/quanpin.txt` 里 佛 读 fu 的 12 条全错。

## Decision

- **直接删改三份关键词表**：删 11 行 emoji 关键词（你妈死了、NMSL、肏你妈、操你妈、傻逼 → 🤬，干你 → 👊，开淫趴、开银趴 → 🥳，草泥马 → 🦙，鸡巴 → 🐔，小鸡鸡 → 🐥）和 2 行颜文字关键词；方块按 CLDR 中文名改成 ◼ 黑色中方块、◻ 白色中方块、◾ 黑色中小方块、◽ 白色中小方块、▪ 黑色小方块、▫ 白色小方块，大方块、大正方形 改挂到 ⬛ ⬜ 上，⬛ ⬜ 另加 黑色大方块、白色大方块。粗口的收词标准与引擎的 `crates/engine/src/lattice/offensive_words.txt` 相同；不涉及性的轻度流行粗口（牛逼 → 👍、颜文字的 cao、kao）和字面义正常的词（屎、放屁、去死）不删，与维护者「流行粗口进轻度表、不删」的决定一致。
- **拒绝表 `resources/dictionary-sources/keyword-vetoes.txt`**：`关键词` 拒绝它对应的任何表情或颜文字，`关键词<TAB>表情` 只拒绝这一对。Emoji、Kaomoji 两个阶段都调用 `others::check_keyword_vetoes`，一起查 `KEYWORD_FILES` 里的三份表：任何一行命中就让构建失败，并列出文件和行号。关键词比较时 ASCII 不分大小写，表情比较时忽略 U+FE0F。
- **拒绝表的「过期」检查只管成对的条目**：成对条目里的表情在三份表里一行都没有了就失败。只写关键词的条目没有可以对照的东西，不检查。
- **读音改正写进 `pinyin-overrides.txt` 末尾一节**，26 行，每行的读音都取拼音主库里这个词权重最高的读音。长胡子的女人（🧔‍♀️）保留 zhang：长胡子 是动宾结构，主库里也只有 zhang'hu'zi。
- **注释行改为「`#` 开头且不含制表符」**（`others::is_comment`），`keyword_lines` 和 `read_emoji_catalog` 共用这条规则。数据行至少有一个制表符，现有文件里的注释行一个制表符也没有。符号表一行一个符号、没有制表符，另用 `is_piliapp_comment`：只有单独的 `#` 和 `# ` 加说明是注释（分类标题行在这之前已经按正则匹配掉）。
- **词库仓删掉 因为、所以 两行**。它们是全拼主库多字词里排第 6、第 8 的词，也是这张表文件头写明「不收」的常用虚词。∵ ∴ 照样能用 因为号、所以号 打出来。引擎的表检查新增 `FUNCTION_WORD_NAMES`（10 个常用连词），`conjunctions_offer_no_symbols` 断言这几个码不出 ∵ ∴。这样以后升锁时，如果词库仓又把它们加回来，引擎测试会失败。
- **同类检查**：其余多音节名称里权重最高的依次是 存在（第 132）、左右（第 205）、属于（第 243）、角度（第 571），比 因为、所以 低一个数量级以上，而且是实词，也是符号本来的读法，保留。
- **birds 包**：12 条 佛 改读 fo；同一类的 尼泊尔鹪鹛 泊 po → bo 一起改。佛得角圆尾鹱 改完后与 `sources/pinyin/rime-ice.txt` 同词同音，按该包的去重规则去掉这一行，中文候选从 9819 条变为 9818 条。单改 `packs/` 不触发发版。

## Alternatives considered

- **源表不动，构建时按拒绝表丢行，表里每条都要命中一行（照 `curation/` 的做法）。** 最强的理由是与 `curation.rs` 的约定一致（上游文件不改，删改写成独立的表，过期就失败），重新整理时也不用管拒绝表。不采用的原因：这三份表不是逐字节锁定的上游文件，是本仓自己整理过的文本（文件头写着「整理为不重复的」），方案 §3.3 也写明可以直接删改；把粗口留在源表里、只在产物里删掉，读源表的人（比如另一个读 `emoji.txt` 的工具）看到的仍是脏数据。拒绝表在这里的作用是守门，而不是施加改动。所以干净的状态本来就该一条也不命中，「每条都要命中」这条约束对它不成立；能检查的只有成对条目里的表情是否还在。
- **命中时静默丢弃，不让构建失败。** 最强的理由是重新整理的人什么都不用做。不采用的原因：静默丢弃之后，源表里又有了脏行，而且没人知道；构建失败时报错会给出文件和行号，删掉那一行就行，代价很小。
- **用引擎的粗俗词表（或词库仓的 `curation/offensive-words.txt`）按子串自动拦关键词。** 最强的理由是不用另维护一张表，以后的新粗口也能拦住。不采用的原因：Emoji、Kaomoji 阶段不读 msime-dictionary（`--only emoji` 就是只跑不读词库仓的阶段），读引擎目录下的文件又会跨 crate 耦合；而且那张表是给拼整句用的，单字不收（鸡巴 收了，鸡 当然不能拦），NMSL、肏你妈、开银趴 这类不在主库里的写法本来就不在表里。
- **因为、所以 留在表里，改引擎，把高频锚定词后面的符号挪到第一页末尾（MSIME-Windows#928 原本就要求「固定于最后一位」）。** 最强的理由是保留请求者要的读法。这次不做：这是引擎的排位规则，影响所有名称，需要另外评测，不属于数据清洗；先删掉，等那条规则做好再考虑收回这两个名称。
- **按权重阈值一并删掉 存在、左右、属于。** 最强的理由是它们的首字母 `cz`、`zy`、`sy` 也是常用简拼。不采用的原因：首字母命中本来就按 `ambiguous` 最早排第 4 位、只跟在锚定词后面；这几个词是实词，也是 ∃ ↔ ∈ 的标准读法，没有像 因为号 那样更长的替代名称（左右箭头 除外），删掉会让请求者要的读法失效。

## Consequences

- `msime-others.db` 会变：emoji 少 11 个关键词，方块关键词改了，26 个关键词的键改了，#️⃣ 的 `sort_order` 从 1935 变为 1558，并多了 `#` 关键词，它原来就排在最后，其他表情的 `sort_order` 不变（dict-v2.0.56 对照试跑产物核实）。`symbol_catalog` 的项目清单多了 #️⃣，它后面的符号 `sort_order` 各加 1。`kaomoji_catalog` 少了 ( *・ω・)✄╰ひ╯ 的 2 行（两个关键词各一行），排在它们后面的行 `sort_order` 减 1 或减 2；颜文字的 长鼻子 从 zhangbizi 改为 changbizi。其他附件不变。
- 以后照上游重新整理 emoji 或颜文字表时，被删的行一回来构建就失败，要逐行删掉；新出现的粗口不在表里的拦不住，仍要人工看 diff。
- `pinyin-overrides.txt` 末尾一节不再是 pypinyin 的输出，`docs/third-party.md` 里对它的说明同步改了。
- 引擎这次只多了测试，不改行为：msime 的锁还在 1af243c，副本里本来就没有这两行。MSIME-Windows 的本地构建器不经锁直接读 `symbol-names.tsv`，词库仓合入后那边打 `yinwei` 也不再出 ∵。
- 相关笔记：打拼音出符号的位置规则见 [symbol-candidates](../feature/2026-10-13-symbol-candidates.md)，符号表迁到词库仓见 [math-greek-symbol-names](../feature/2026-10-25-math-greek-symbol-names.md)，`curation/` 表的约定见 [dictionary-curation-tables](../feature/2026-11-04-dictionary-curation-tables.md)。
