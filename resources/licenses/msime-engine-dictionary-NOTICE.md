> 本文件是 msime 随词库分发的来源与授权说明。从下一行「来源与授权说明」起到「msime 构建时另加的输入」之前，逐字取自 [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 提交 `c239ba1a584511d15d33ff9c1c520897993f2013` 的 `NOTICE.md`，这个提交是 `resources/desktop-dictionary.lock.json` 与 `resources/language-dictionaries.lock.json` 所固定的 `dict-v2.1.2` 发布的源数据提交（发布说明与 `msime-dictionary-manifest.json` 都记着它）；其中的相对路径指那个仓库。「msime 构建时另加的输入」一节不在那份文件里，记的是由 msime 仓库一侧进入同一批词库的来源和构建器自己做的修改。词库锁换到新的 `dict-v*` 时，按那次发布的源数据提交重新同步本文件；构建器登记了新的上游、而锁定的发布还没有收录时，在末尾另起一节「已登记、尚未随当前词库发布的来源」写明，换到收录它的发布后删掉。

# 来源与授权说明

本仓库**不对外提供统一的开源许可**，因为其中绝大部分词库并非本项目的作品，而是从其他项目收集、合并、去重而来（见 [README](README.md)）。给整个仓库挂一份 LICENSE 等于替上游作者重新授权，因此这里改为逐项说明来源与上游条款。使用或再分发本仓库的数据时，请以对应上游的条款为准。

## 可选鸟类名称词库

`packs/birds/quanpin.txt`、`packs/birds/english.txt`、`packs/birds/translations.txt` 的鸟名来自 [alchaincyf/huashu-bird-sticker](https://github.com/alchaincyf/huashu-bird-sticker/tree/47bb6f366641a76a482e2fb2fd26b2ac515f1a13) 提交 `47bb6f366641a76a482e2fb2fd26b2ac515f1a13` 的 `data/species.json`（`names.zh-Hans` 与 `names.en`），对照同提交的 `data/species.csv`。该项目 `NOTICE.md` 声明这些名称来自 **IOC World Bird List v15.2** 多语种版，编者 **Frank Gill、David Donsker、Pamela Rasmussen**，按 [CC BY 3.0（署名 3.0 未本地化版本）](https://creativecommons.org/licenses/by/3.0/) 许可使用，原始来源为 [IOC World Bird List](https://worldbirdnames.org/)。

本包沿用 CC BY 3.0，不适用本仓自建专业词库的 GPL-3.0 声明。整理时保留原中英鸟名，筛选有简体中文名的 11,041 个鸟种，以 pypinyin 0.55.0 标注并按语境修正读音，对发布拼音输入去重，生成英文输入键、导入权重及中英双向对照。数据摘要、去重集合、多音字规则及审校状态见 [packs/birds/README.md](packs/birds/README.md)。未使用该项目的贴纸、参考照片或图鉴文字，也不将其 MIT 或 PolyForm Shield 许可套用到这些第三方鸟名。

## 中文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/pinyin/rime-ice.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，对照提交 `9e66b0729083b37d217312294f6d516c8d7234be`（见下）；与万象共有词条的权重取自 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0；共有词条的权重为 CC BY 4.0（rime-wanxiang） |
| `sources/pinyin/rime-ice-supplement.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的 `cn_dicts/base.dict.yaml`、`ext.dict.yaml`、`8105.dict.yaml`、`others.dict.yaml`，排除 `rime-ice.txt` 已有的同词同音行；与万象共有词条的权重取自 rime-wanxiang `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0；共有词条的权重为 CC BY 4.0（rime-wanxiang） |
| `sources/pinyin/places.txt` | [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China)，提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` 的 `dist/provinces.csv`、`dist/cities.csv`、`dist/areas.csv`（原始数据来自国家统计局的统计用区划代码）。由 msime `crates/dict-builder` 的 `msime-dict-build places-supplement` 生成省、地、县三级区划的全称与去后缀简称，读音须由 `rime-ice.txt`、`rime-ice-supplement.txt` 证实，只收这两份文件没有、或权重不超过 92 且低于分层下限的词，这两份文件已有排名（权重大于 92）的词不抬升（92 是万象的底值 10 按对齐系数 9.2 换算后的值：万象地名词库里八成多的行都是这个未排名的底值，对齐前的未排名尾部 1–10 也随之变成 28–92）；这两份文件里同一全拼的其他词权重超过 `rime-ice.txt` 全部行权重的第 90 百分位（常用线，由生成器从数据算出）时，地名权重降到比该词低 1，不抢常用词的首位，比较时跳过权重为占位值的词（`rime-ice.txt` 里权重超过常用线的行中被超过 1% 的行共用的单个权重值，由生成器从数据算出，当前为 9999）；万象（`sources/pinyin/wanxiang/`，提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）提供权重的 15 份词库收了同一个词和全拼时，再取 round(万象最高权重 × 9.2) 作为下限，只升不降：高于上面得出的权重、也高于这两份文件已有的权重时写这个值，不受常用线封顶限制，已排名的词也按它抬升（湖州 万象 1228，写 11298，排到 胡诌 前面）；规则、对照文件和两张 curation 拼音表的 SHA-256、万象提交写在文件头 | WTFPL（区划名称）；读音的证实与权重下限依据 rime-ice 的 `rime-ice.txt`、`rime-ice-supplement.txt`（GPL-3.0），其中与万象共有的行的权重改编自万象（CC BY 4.0，见「拼音权重与万象对齐」）；万象下限定的权重改编自万象（CC BY 4.0，改动是乘 9.2 后四舍五入） |
| `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt` | [wuhgit/CustomPinyinDictionary](https://github.com/wuhgit/CustomPinyinDictionary)（`2023-09-28(No.82)` 版）与 rime-ice 合并去重 | GPL-3.0 与**未声明**的混合 |
| `sources/pinyin/single-chars.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，读音以 [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) 校正；与万象共有读音的字频取自 rime-wanxiang `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0 + MIT；共有读音的字频为 CC BY 4.0（rime-wanxiang） |
| `sources/wubi/wubi86-jidian.txt` | [KyleBing/rime-wubi86-jidian](https://github.com/KyleBing/rime-wubi86-jidian)，提交 `28f19e3affdabb37d11d38f457c7a08a4694b226` 的 `wubi86_jidian.dict.yaml` 数据区，另有本仓的 6 处改动（见下） | Apache-2.0（取用时上游的条款，见下） |
| `sources/wubi/wubi86-rime-wubi.txt` | [rime/rime-wubi](https://github.com/rime/rime-wubi)，提交 `152a0d3f3efe40cae216d1e3b338242446848d07` 的 `wubi86.dict.yaml` 里极点表没有的行（见下） | LGPL-3.0 |
| `sources/wubi/wubi86-supplement.txt` | 本项目生成的补充表：由 msime `crates/dict-builder` 的 `wubi86-supplement` 子命令从本仓 `sources/wubi/wubi86-jidian.txt`、`sources/wubi/wubi86-rime-wubi.txt`、`sources/pinyin/single-chars.txt`（只用于读 86 表时比词频）、`sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt` 生成。词条取 86 表没有、而 98 表收录，或 rime-ice 主表与补充表权重不低于 100 的两字及以上词，或区划补充表与人工词条（不限权重；只收构建器支持的基本区汉字），编码按 86 版词组规则由 86 表的单字全码推出 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice 与人工词条、WTFPL 的区划名称、GPL-3.0-or-later 的 Fcitx 表与 Unlicense 的 98 五笔表；编码依据 Apache-2.0 的 jidian 单字码） |
| `sources/wubi/wubi98-supplement.txt` | 本项目生成的补充表：由 msime `crates/dict-builder` 的 `wubi98-supplement` 子命令从本仓 `sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt`、`sources/wubi/wubi86-jidian.txt`、`sources/wubi/wubi86-rime-wubi.txt`、`sources/pinyin/single-chars.txt`（只用于读 86 表时比词频）、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt` 生成。词条取两张 98 表都没有、而 86 表收录，或 rime-ice 主表与补充表权重不低于 100 的两字及以上词，或区划补充表与人工词条（不限权重；只收基本区汉字），编码按词组规则由 98 表的单字全码推出 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice 与人工词条、WTFPL 的区划名称及 Apache-2.0 的 jidian 表；编码依据 Unlicense 的 98 五笔表和 GPL-3.0-or-later 的 Fcitx 表） |
| `sources/wubi/wubi98.txt` | [yanhuacuo/98wubi-tables](https://github.com/yanhuacuo/98wubi-tables)（98五笔小组），提交 `6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697` 的 `98五笔含词表-【单义】.txt`（见下） | Unlicense |
| `sources/wubi/wubi98-fcitx.txt` | [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)，提交 `dbc7154a7f0b9fc04313160ae8066ed4d8cbc446` 的 `tables/wubi98.txt` | GPL-3.0-or-later（随仓库的 `LICENSES/GPL-3.0-or-later.txt`） |
| `sources/wubi/wubixsj-supplement.txt` | 本项目生成的新世纪五笔词组表：由 msime `crates/dict-builder` 的 `wubixsj-supplement` 子命令从本仓 `sources/wubi/wubixsj-byujiang.txt`、`sources/wubi/wubi86-jidian.txt`、`sources/wubi/wubi86-rime-wubi.txt`、`sources/pinyin/single-chars.txt`（只用于读 86 表时比词频）、`sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt` 生成。词条取 86、98 表里的两字及以上词，或 rime-ice 主表与补充表权重不低于 100 的两字及以上词，或区划补充表与人工词条（不限权重；只收基本区汉字），编码按词组规则由新世纪表的单字全码推出；86 极点表里有的词沿用它在极点表里的权重，其余的权重低于同码单字。不用 `wubixsj-byujiang.txt` 自己的词组 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice 与人工词条、WTFPL 的区划名称、Apache-2.0 的 jidian 表、Unlicense 与 GPL-3.0-or-later 的 98 表；编码依据仓库级 GPL-3.0 的新世纪单字码，见下；第一层权重取自 Apache-2.0 的 jidian 表） |
| `sources/wubi/wubixsj-byujiang.txt` | [byujiang/wubi06_rime](https://github.com/byujiang/wubi06_rime)，提交 `fc1cedbb21cde3ca160ce291888fa70697720ab3` 的 `dicts/wubi06.dict.yaml`（新世纪五笔，见下） | GPL-3.0（仓库级，由转载者给出；来源没有记录，见下） |
| `sources/unlicensed/single-char-whitelist.txt` | **待确认**，见下方「待解决」 | **待确认** |

`sources/pinyin/rime-ice.txt` 的上游提交是后来补记的：此前它是唯一没有被记录提交的中文词库来源，没有办法判断树里那份是从哪个版本导出的。按 rime-ice `9e66b0729083b37d217312294f6d516c8d7234be` 比对，它的前身 `BaseDictIce.txt` 有 28185 条上游不存在的词条，另有 8328 条权重与上游不同。多出来的条目里既有上游没收的真词（地名、品牌），也有语料切分留下的碎片（例如「米高」「钟的」「之事」）。已确认的碎片在 msime-engine 的 `67fd083` 中从当时的 `cn/BaseDictIceV1.txt` 与 `cn/BaseDictAllV1Part1.txt`（即现在的 `sources/pinyin/rime-ice.txt` 与 `sources/unlicensed/custom-pinyin-dictionary-part1.txt`）删除，本仓库的这两个文件与 msime-engine `e92a9c7c64e262f30218ca9aaad5f40d0b18cf89` 的 `dictionary/cn/` 逐字节一致。这份文件的真实来源仍然没有查清，所以不要把它描述成「原样搬运」。

`sources/wubi/wubi86-jidian.txt` 与 `sources/wubi/wubi86-rime-wubi.txt` 原是同一个文件 `wubi86-jidian.txt`：#36 把 rime-wubi 的词条接在了极点表末尾。两段后来按原样拆开，两个文件首尾相接仍是拆分前那个文件的全部字节（1992566 字节，SHA-256 `70afd8476e53b0f1b2184ebfafefb62212e8d3d554b12612cbae0336efa9a0e5`），现在分别由 `upstream.lock.json` 记在 `rime-wubi86-jidian` 和 `rime-wubi` 下。

- `sources/wubi/wubi86-jidian.txt`（89272 行，1319423 字节，SHA-256 `852e74d042b919a3a8f5f3d8591c0ea0002240a539aa0f9aa2cff1b6d1d09c35`）是 KyleBing/rime-wubi86-jidian 提交 `28f19e3affdabb37d11d38f457c7a08a4694b226`（2025-11-19）的 `wubi86_jidian.dict.yaml` 在 `...` 一行之后的数据区，但不与它逐字节一致，有本仓的 6 处改动：提交 `923ca17` 删掉了上游第 7884 个数据行 `<TAB>cokg<TAB>10<TAB>全角空格`（词那一列是空串，上游这一行没有全角空格字符本身，构建器的行解析本来也会把它当无效行跳过），给上游没有权重的 5 行（蹒跚 `khkh`、猫猫 `qaqa`、狗狗 `qqqq`、猫猫 `qtqt`、狗狗 `qtqt`）补了权重 10；#36 在上游没有换行的末行（文言 `yyyy`）后面补了 CRLF，所以这个文件只有最后一行是 CRLF，其余是 LF。按这些改动逆推可以逐字节还原上游的数据区。上游在 2026-10-29 的提交 `33a1f9430fa9bcb30b2f32b698e4e497f8864091` 把许可改成 MIT；这张表取自那之前的提交，那时的 `LICENSE` 是 Apache-2.0，与 `063377a0e3ae7d4084a45f03e4269f62f874f7dc` 的 `LICENSE` 逐字节相同。
- `sources/wubi/wubi86-rime-wubi.txt`（50497 行，673143 字节，SHA-256 `4b31d10564792118f50b591bdf37b3fd3218376ca4d9ad324d3a1d699b34b4af`，CRLF）是 rime/rime-wubi 提交 `152a0d3f3efe40cae216d1e3b338242446848d07` 的 `wubi86.dict.yaml` 在 `...` 一行之后的数据行，按文件顺序只取「词、编码」在上面的极点表里没有的行，同一组「词、编码」只取第一次出现的，只留词、编码、权重三列；权重不是整数的 1 行（`χ<TAB>zzxx<TAB>450.5`）不收。其中 49295 行是扩展区字，构建器不写入词库。rime-wubi 的权重是词频（从 0 到 444000000），极点表的权重是同码名次（10、20、30……），msime 构建器从拆分后的布局起这样读这个文件（见 msime `crates/dict-builder/src/wubi86_table.rs`），文件本身不改：极点表已经收了的词组不写入它给的另一个编码，实际是 2 行，`版权所有<TAB>bqsy`（上游的错码，拼音首字母，86 版应为 `tsrd`，由极点表提供）和 `T恤<TAB>ttnt`（极点表是 `tntl`）；上游权重为 0 的 12 个部首、部件（如 ⺻ `vhgd`）按 0 写入，排在同码的补充词之后；其余的行一般按权重 10 写入（同码极点行有更低的名次时取那个名次），上游的词频不进表，只用来排同码几行的先后；极点表没收的常用词不该落到同码冷僻行之后，所以两边换到拼音库上游四张表（`sources/pinyin/single-chars.txt`、`rime-ice.txt`、`rime-ice-supplement.txt`、`places.txt`）的词频上比（按 `curation/pinyin-vetoes.txt` 去掉否决的读音，`curation/pinyin-weight-overrides.txt` 第四列写了 `wubi` 的频率修正换成修正后的值），这里的词频不低于 100、又是同码最后几行极点行各自词频 10 倍以上的行，排到这几行之前。之后不含汉字的行（假名、符号等）降到同码下一行的权重，同码次序不变，这一条对极点表的行也适用。

`sources/wubi/wubi98-fcitx.txt` 保留 Fcitx5 table-extra 的 UTF-8 原始表格式（表头、规则和数据区），构建器只读取数据区的 `编码 空格 词` 行，并与 98 五笔主表按“编码、词语”去重。该表补足主表没有的合法开源候选，例如 `ukuy` 的“冲凉”，同时保留 Fcitx 表中的完整候选集合，不把单个词条作为特殊补丁。

`sources/wubi/wubi98.txt` 与上游文件逐字节一致（SHA-256 `1b5a4c22eddae08d8e0e6aa8f926f92d45d6cdaa591e5cee0e8aad29271da1cc`，UTF-16LE 带 BOM、CRLF），没有改编码或换行。权利声明来自数据作者本人：上游仓库在 2024-01-20 的提交 `4dbcaa65` 加入 Unlicense，此前同一作者在 2021 年把这份表贡献给 [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)（`tables/wubi98.txt`，GPL-3.0-or-later 包内分发，Debian main 收录）。两份表 112584 条里只有 4 条不同，差异来自上游 2024-06-25 的拆分校正（`72a36bae`）。上游没有记录 GB2312/GBK 部分的单字编码与简码是怎样生成的，只能说它们依照 98 版五笔的编码规范；这里如实记下，不把它描述成独立生成。界面上称“98 五笔”，不使用“王码”“五笔字型”等商标。

`sources/wubi/wubixsj-byujiang.txt` 与上游 `dicts/wubi06.dict.yaml` 逐字节一致（1421790 字节，SHA-256 `0c668f59b13b6f201d479678883925957989156b69eeaec163870ace2b011875`，UTF-8、LF、无 BOM），没有改编码或换行；文件名照 `wubi86-jidian.txt`、`wubi98-fcitx.txt` 的写法改成「方案-来源」，上游原文件名是 `wubi06.dict.yaml`。msime 构建器只取其中的单字行：只收基本区汉字，编码必须是 1 到 4 个 a–y 字母；上游的词组不用，上游的权重只用来定同一编码下几个单字的先后（例如 `g` 下「一」在「王」之前），数值本身不进表；新世纪五笔的词组由 msime 按这张表的单字全码和词组规则，从本仓已有的词表生成。许可与来源如实记在这里：GPL-3.0 是仓库维护者 byujiang 在仓库层面给出的，不是码表编制者本人的权利声明。上游没有记录码表从哪里来：文件头的「Original table author: Wozy」与 [rime/rime-wubi](https://github.com/rime/rime-wubi) 的 `wubi86.dict.yaml` 文件头相同，是沿用 86 极点码表的署名，不是这张表的出处；首次数据提交收了 27533 个字，与王码新世纪版公布的收字数相同，所以这张表很可能派生自那张商业码表。找到的其他公开新世纪码表也都能追溯到同一来源，许可同样只来自转载者，没有一份达到上面 98 五笔表那样「权利声明来自数据作者本人」的标准。维护者知道这些情况后决定接受这个风险，标准与同样由转载者给出许可的 jidian 86 表相同。仍在逐条校正新世纪码表的 CNMan/UnicodeCJK-WuBi06 明确写着权利归王码公司、请勿商用，所以不从那里取数据。专利与商标已于 2026-11-02 按公开来源核查（未经国家知识产权局官方系统复核，不构成法律意见）：新世纪版的专利 CN1117311C（ZL99109798.X，1999-07-19 申请）最晚 2019-07-19 届满，86、98 版专利也早已失效；没有查到“新世纪五笔”“五笔字型”在第 9、42 类的注册。本仓只在描述编码方案时使用“新世纪五笔”一词，与王码公司及其产品没有关联。界面上称“新世纪五笔”“五笔 新世纪”，不使用“王码”“五笔字型”等商标。

### 万象词库原始文件

`sources/pinyin/wanxiang/` 下的以下 17 份文件全部来自 **万象词库 / 万象拼音（rime-wanxiang），作者 amzxyz**，直接来源为 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 标签 `v18.0.16`（提交 [`7ab73ee0c7f80190004233e70f954833df64c80e`](https://github.com/amzxyz/rime-wanxiang/tree/7ab73ee0c7f80190004233e70f954833df64c80e/dicts)）的 `dicts/` 目录，是该目录下除 `abbrev.dict.yaml`、`t9_abbrev.dict.yaml`（简拼与九键简码）和 `en.dict.yaml`（英文）以外的全部词库。上游 README 将词库来源标为同一作者的 [万象词库（amzxyz/RIME-LMDG）](https://github.com/amzxyz/RIME-LMDG)，多份词库的文件头也保留了该链接。

| 本仓文件（均位于 `sources/pinyin/wanxiang/`） | 上游文件（均位于 `dicts/`） | 上游许可 |
| --- | --- | --- |
| `cuoyin.dict.yaml` | `cuoyin.dict.yaml` | CC BY 4.0 |
| `diming.dict.yaml` | `diming.dict.yaml` | CC BY 4.0 |
| `duoyin.dict.yaml` | `duoyin.dict.yaml` | CC BY 4.0 |
| `fangyan.dict.yaml` | `fangyan.dict.yaml` | CC BY 4.0 |
| `huaxue.dict.yaml` | `huaxue.dict.yaml` | CC BY 4.0 |
| `jichu.dict.yaml` | `jichu.dict.yaml` | CC BY 4.0 |
| `lianxiang.dict.yaml` | `lianxiang.dict.yaml` | CC BY 4.0 |
| `mingren.dict.yaml` | `mingren.dict.yaml` | CC BY 4.0 |
| `mixed.dict.yaml` | `mixed.dict.yaml` | CC BY 4.0 |
| `renming.dict.yaml` | `renming.dict.yaml` | CC BY 4.0 |
| `shici.dict.yaml` | `shici.dict.yaml` | CC BY 4.0 |
| `taifeng.dict.yaml` | `taifeng.dict.yaml` | CC BY 4.0 |
| `wuzhong.dict.yaml` | `wuzhong.dict.yaml` | CC BY 4.0 |
| `yaopin.dict.yaml` | `yaopin.dict.yaml` | CC BY 4.0 |
| `yiren.dict.yaml` | `yiren.dict.yaml` | CC BY 4.0 |
| `yixue.dict.yaml` | `yixue.dict.yaml` | CC BY 4.0 |
| `zi.dict.yaml` | `zi.dict.yaml` | CC BY 4.0 |

许可依据是该提交仓库根目录的 [LICENSE](https://github.com/amzxyz/rime-wanxiang/blob/7ab73ee0c7f80190004233e70f954833df64c80e/LICENSE) 与 README 的 CC BY 4.0 声明。许可名称为 Creative Commons Attribution 4.0 International（知识共享署名 4.0 国际），[许可链接](https://creativecommons.org/licenses/by/4.0/)，全文亦收在本仓 [sources/symbols/LICENSE](sources/symbols/LICENSE)。

本仓只将上述文件从上游 `dicts/` 复制到 `sources/pinyin/wanxiang/`，保留原文件名及全部字节，未修改文件头、词条、读音、权重、行序、编码或换行；`upstream.lock.json` 以上游名 `rime-wanxiang` 固定每份文件的大小和 SHA-256。这批文件最初收的是提交 `aa9947f3f513ff88a3f7ce4e342fcb519d517025` 的 15 份，构建器不读；下面「拼音权重与万象对齐」用的却是 `7ab73ee` 的 17 份，`aa9947f` 少了 `renming`、`lianxiang` 两份，`jichu` 等也与 `7ab73ee` 不同，拿它判断「万象收不收」会把约 2.5 万行误判为不收（李四、林克、马克 这类人名）。所以换成了 `7ab73ee` 的全套，与权重对齐用的是同一份数据，并登记进 `upstream.lock.json`，msime 构建器从此在构建时读它们（见下节）。再分发原文件或改编数据时，须保留作者 amzxyz 与万象词库的署名、来源链接、许可链接以及上游提供的版权与免责声明；改编时须注明改动并保留已有改动说明。上游按许可中的免责声明以原样提供数据。

### 拼音权重与万象对齐

`sources/pinyin/single-chars.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt` 中与万象共有的词条，权重改编自 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 标签 `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）`dicts/` 下的词库。上游仓库以 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) 发布（仓库根目录 `LICENSE`，全文也收在本仓 `sources/symbols/LICENSE`），词库文件头注明数据出自同一作者的 [amzxyz/RIME-LMDG](https://github.com/amzxyz/RIME-LMDG)。署名：万象拼音（rime-wanxiang），作者 amzxyz。CC BY 4.0 可以单向并入 GPL-3.0 作品，所以这三份文件仍按 rime-ice 的 GPL-3.0 分发，同时保留这里的署名与改动说明。

改动：只改权重列，词条、读音、行序、注释与换行都保持原样，没有从万象加入新词条。

- 对照范围是万象 `wanxiang.dict.yaml` 导入的词库：`zi`、`jichu`、`lianxiang`、`duoyin`、`shici`、`diming`、`yixue`、`huaxue`、`yaopin`、`mingren`、`yiren`、`wuzhong`、`renming`、`taifeng`、`fangyan`。错音错字表 `cuoyin` 收的是错误读音，不参与。
- 万象的带调拼音先去掉声调（NFD 分解后删去组合附加符号），ü 写作 v，音节之间用 `'` 连接，与本仓的全拼写法一致。去调后同一个「词＋全拼」在万象里出现多次（不同声调，或出现在多份词库里）时，取最高的权重，这也是万象候选去重后实际显示的排序。
- 本仓一行的「词＋全拼」在万象里有对应时，权重改为 `round(万象权重 × 系数)`，四舍五入取整；万象没有的行保持原权重。
- 两边的权重量级不同，所以每份文件各用一个系数，让改写后的权重与该文件未改动的行处在同一量级。系数 = 该文件与万象共有、且两边权重都大于 10 的行上，本仓权重之和 ÷ 万象权重之和，保留两位小数。本次 `single-chars.txt` 为 9.42（共有 45175 行），`rime-ice.txt` 为 9.2（共有 521669 行），`rime-ice-supplement.txt` 为 9.28（共有 30499 行）。三份文件的系数相差不到 3%，所以合并后共有词条之间的相对次序基本与万象一致；同一单字读音同时出现在 `single-chars.txt` 与 `rime-ice-supplement.txt` 时，构建取较高的一行。
- `sources/pinyin/places.txt` 是 msime 生成器产出的地名下限表，不手工改，这次没有动；构建时它按只升不降并入，只会把低于下限的地名调高。`custom/words.txt` 只追加，也没有动。

**构建期重算。** msime 构建器的 `reweight` 阶段（`crates/dict-builder/src/reweight.rs`，排在 `quanpin` 之后、`places-supplement` 之前）读 `sources/pinyin/wanxiang/` 里登记的 17 份文件，在每次构建时按上面同一套规则（同样 15 份词库、同样的读音换算和取最高权重、同样的三个系数、四舍五入）重算三份文件里与万象共有的行。三份文件现在的权重就是这样算出来的（共有的 597343 行逐行吻合），所以这一步不改变产物；它保证以后在原路径重新生成补充表、或者换新的上游提交时，对齐结果不会被冲掉。只认写了非负整数权重的万象行：没写权重（`fangyan` 的 闹热）或写 `-1`（`diming` 的 柏香镇）的行不换算，#84 当时也没有换算它们。

`curation/pinyin-reweight.txt` 存在时，`reweight` 阶段另做第二步（格式见 msime `crates/dict-builder/README.md`「Curation tables」）：`renming`（人名，正则抽取、未经筛选）和 `duoyin`（多音兼容读音）两份词库的权重先乘一个缩放比例再换算；`unattested-cap` 让万象全套 17 份词库在任何读音下都不收的多字词，不再排在同一全拼键上万象原始权重达到门槛（默认 100）的词前面（门的 不再压过 闷的）；`curation/pinyin-reweight-exempt.txt` 列出的行不做第二步。这一步改的仍然只是权重，用的仍然只是万象的权重和收录与否，不加入任何万象的词条。文件不在时不做第二步。

`sources/wubi/wubi86-supplement.txt` 的文件头记录了生成时对照的 `rime-ice.txt`、`rime-ice-supplement.txt` 的 SHA-256。那是生成当时的输入，作为历史陈述保留，与现在的文件不同；`upstream.lock.json` 已按改写后的字节刷新。`sources/pinyin/places.txt` 之后已按改写后的这两份文件重新生成，文件头记的就是现在的 SHA-256：权重下限、常用线（第 90 百分位）和已有排名都按对齐后的权重算，所以地名的权重随之降到与对齐后的同音词同一量级，不再按对齐前的量级压过同音常用词（阜阳 原为 31460，高于 抚养 对齐后的 25245）。地级简称的下限（当前 5115）低于常用线（当前 7204），常用线封顶对它们不起作用，所以仍有少数地级简称排在同全拼、权重不低于 1000 的词前面（丽水 > 溧水，晋中 > 金钟，辽源 > 燎原，迪庆 > 敌情，阜新 > 负心）；对齐时被压到万象底值 92 的地名（乌鲁木齐市 对齐前 45320）按分层下限重新抬起。dict-cleanse 之后又在含 `curation/` 的提交上重新生成：对照集合、常用线（当前 7158）和已有排名按施加否决和覆盖之后的行算，并加了万象下限，地名的权重不再低于万象词频按 9.2 换算后的值（本次 719 行由它定权）。所以 rime-ice 没收、万象词频高于同音词的地名排回同音词前面（湖州 > 胡诌），万象词频低于同音常用词的仍在后面（固原 < 雇员，阜阳 < 抚养）。

### 维护者清洗表

`curation/` 下拼音的三张表由 msime 构建器在读入上面这些文件之后施加，源文件本身不改：`curation/pinyin-vetoes.txt` 从全拼表删掉列出的词或读音（整词否决也从 `wubi86`、`wubi98`、`wubixsj` 三张五笔表删掉这个词），`curation/pinyin-weight-overrides.txt` 把列出的读音的权重改成给定的值，`curation/offensive-words.txt` 写成 `msime-pinyin.db` 的 `sentence_excluded` 表，并让三张五笔表里含这些粗俗词的行排到同码其余行之后。五笔补充词组只取覆盖表里第四列写了 `wubi` 的频率修正来排同码，覆盖表不会让词掉出五笔补充词组的门槛。所以发布的 `msime-pinyin.db`、`msime-wubi.db` 是对 rime-ice（GPL-3.0）以及五笔原表数据的修改版本，修改内容就是这三张表，每一行都可以在本仓的提交历史里查到加入的理由。

2026-11-04 的第一批（dict-cleanse 方案 2.1、3.1、3.2）：

- 否决：564 个含粗俗词的碎片短语（个鸡巴、是傻逼、去他妈的），整词删除。粗俗词本身不删，五笔里排到同码最后。
- 权重覆盖 2278 行，其中 779 行（本地损坏恢复上游值和占位权重）写了 `wubi`。其中 553 行（`rime-ice.txt` 里权重是上游 rime-ice `9e66b0729083b37d217312294f6d516c8d7234be` 的 `cn_dicts/base.dict.yaml`、`ext.dict.yaml` 同词同音权重 5 倍以上、万象对齐又没有改写过的行）恢复上游的权重，其中 7 行同时落进下面的规则、取了更低的值；其余的行降到同一全拼下被它压过的词的权重之下（取那个词在本仓的权重减 1），占位权重降到同一全拼下万象收录的词里本仓权重最高的那个之下。新的权重只来自 rime-ice 上游和本仓已有的权重，没有引入新的数据来源；但「降到某个词之下」取的那个词多是万象收录的词，它在本仓的权重是上面「拼音权重与万象对齐」改编出来的 round(万象权重 × 系数)，所以这些行（占位权重、碎片、非标准读音、custom 越位与 车件，共 1732 行）写的是由改编后的万象权重派生的值，随 `msime-pinyin.db` 按 CC BY 4.0 署名同上一节。
- 选出这些行时用 rime-wanxiang `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）`dicts/` 下除 `abbrev`、`en`、`t9_abbrev` 以外的 17 份词库作参照：万象不收的词才算碎片、万象不收的读音才算非标准读音、万象收录的词才算被压过的常用词。署名同上一节。
- `curation/offensive-words.txt` 是本项目自写的词表，从 msime 引擎内嵌的 `crates/engine/src/lattice/offensive_words.txt` 迁来（那一份以 MPL-2.0 随引擎分发），这里与 `curation/` 的其余两张表一样依据 GPL-3.0 提供。

## 英文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/english/rime-ice-en.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice) | GPL-3.0 |
| `sources/english/rime-ice-en-supplement.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的 `en_dicts/en.dict.yaml` 与 `en_ext.dict.yaml` 中 `rime-ice-en.txt`、`scowl-words.txt`、`custom/english.txt` 都没有的纯 ASCII 单词，大小写照上游；`rime-ice-en.txt` 里被 rime-ice 注释掉的条目（`# na na`）不收。由 msime `msime-dict-build rime-ice-supplement` 生成，生成器提交、去重规则和输入摘要见文件头 | GPL-3.0 |
| `sources/english/scowl-words.txt` | [en-wl/wordlist](https://github.com/en-wl/wordlist)（SCOWL）发布 `rel-2026.02.25`（提交 `7e99edab8e32f9f9ea2b15f249ca8d4d67237410`）的 Aspell 英文词典 `aspell6-en-2026.02.25-0.tar.bz2`（SCOWL 60 级官方拼写检查词典）中 `en-common.cwl`、`en_US-wo_accents-only.cwl`、`en_GB-ise-wo_accents-only.cwl` 的纯字母词形（至少 3 个字母），排除 `rime-ice-en.txt`、`rime-ice-en-supplement.txt`、`custom/english.txt` 已有或被 rime-ice 注释掉的词、蔑称、只有大写形式且拼出中文词全拼的专名，以及没有 Google 词频的词；由 msime `crates/dict-builder` 的 `english-supplement` 子命令生成 | SCOWL 许可：可自由使用、复制、修改、分发和出售，条件是在所有副本中保留版权声明，并在随附文档中同时保留版权声明与许可声明。文件头照录了版权声明与许可声明；`dict-v` 发布把许可全文作为 `msime-scowl_Copyright.txt` 与 `msime-english.db` 一起附上；词典包 `Copyright` 全文收在 msime 的 `resources/licenses/scowl-aspell6-en-Copyright.txt`。60 级官方词典不涉及 UKACD 条款，也不用澳大利亚英语数据 |
| `sources/english/google-word-counts.txt` | [Google 1/3 million 词频表](https://www.norvig.com/ngrams/count_1w.txt) | 以来源页面说明为准 |
| `sources/unlicensed/oaldpe-words.txt` | 自 oaldpe.mdx 提取的词形列表 | 权利归词典出版方 |

`sources/pinyin/rime-ice-supplement.txt` 第 1 行文件头里的 `BaseDictIceV1` 是生成时对照文件的旧名，指现在的 `sources/pinyin/rime-ice.txt`。这个文件按字节锁定，权重另经万象对齐，文件头作为历史陈述保留不改；下次在原路径重新生成时再改成新名。`sources/english/rime-ice-en-supplement.txt` 已用 msime 的生成器在原路径重新生成（2026-11-04，dict-cleanse 方案 H7），文件头写的是现在的文件名。

构建时还按 `curation/english-vetoes.txt` 从上面这些英文词表里删词（任何大小写都不收，连同释义），按 `curation/gloss-vetoes.txt` 不用 ECDICT（MIT，msime 构建时下载）里列出的英文词与中文义项的配对，源文件本身不改。另外，`rime-ice-en-supplement.txt` 里只有大写写法、而 SCOWL 拼写词典收了小写词形的词（PuTTY 和 putty、Playwright 和 playwright，2026-11-04 的试跑是 18 个），构建时补上小写写法（msime `english::add_attested_lowercase`）。所以发布的 `msime-english.db` 是对 rime-ice（GPL-3.0）英文词表和 ECDICT 释义的修改版本，修改内容就是这两张表和这条补写法的规则，表里每一行都可以在本仓的提交历史里查到加入的理由。2026-11-04 的第一批（dict-cleanse 方案 H7、H8）：英文否决 86 个词（网页模板残留 12、成人站点名和色情垃圾词 26、错拼 15、蔑称 7、垃圾邮件药名 26），释义成对否决 65 对（普通中文词配粗口或性含义英文 34、配蔑称 7、普通英文词配辱骂性中文 15、高频中文词的明显误配 9）。

## 粤拼、注音与笔画词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/cantonese/jyut6ping3.chars.dict.yaml`、`sources/cantonese/jyut6ping3.words.dict.yaml`、`sources/cantonese/essay-cantonese.txt` | [rime/rime-cantonese](https://github.com/rime/rime-cantonese)，默认分支 `main` 提交 `259f0e48bba840c3a2e0d117539e96937f3d89bc`（上游每周发布的 `latest` Release 所在提交）的同名文件 | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) |
| `sources/cantonese/hkcancor-word-counts.txt` | [fcbond/hkcancor](https://github.com/fcbond/hkcancor)（香港粤语语料库 HKCanCor），提交 `39aeadf920e0b5ca93d0ad7792c59e740e7bdd65` 的 `data/utf8/` 下 58 个转写文件，由 msime `crates/dict-builder/src/hkcancor.rs` 统计成两字及以上汉字词的词频，只含词与次数 | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)（上游 `data/LICENSE`） |
| `sources/zhuyin/tsi.csv`、`sources/zhuyin/word.csv` | [chewing/libchewing-data](https://github.com/chewing/libchewing-data)，提交 `c44e81aef24b06f1509f19e1be54c99812d0c43f` 的 `dict/chewing/tsi.csv`、`dict/chewing/word.csv` | [LGPL-2.1-or-later](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html) |
| `sources/zhuyin/mcbopomofo-supplement.txt` | [openvanilla/McBopomofo](https://github.com/openvanilla/McBopomofo)，提交 `be6564acad6c4d3265c34a2e1a872d80f9db6068` 的 `Source/Data/BPMFMappings.txt`，转换为本仓 CSV 格式并排除 `tsi.csv` 已有组合 | BSD（上游说明多字词表源自 BSD 授权的 libtabe，并含修改） |
| `sources/zhuyin/phrase.occ` | [openvanilla/McBopomofo](https://github.com/openvanilla/McBopomofo)，提交 `be6564acad6c4d3265c34a2e1a872d80f9db6068` 的 `Source/Data/phrase.occ` | [MIT](https://github.com/openvanilla/McBopomofo/blob/be6564acad6c4d3265c34a2e1a872d80f9db6068/LICENSE.txt) |
| `sources/stroke/stroke.dict.yaml` | [rime/rime-stroke](https://github.com/rime/rime-stroke)，提交 `1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48` 的 `stroke.dict.yaml` | [LGPL-3.0](https://www.gnu.org/licenses/lgpl-3.0.html) |

这七个文件与上游固定提交逐字节一致（UTF-8、LF），没有改编码、换行或内容：

| 文件 | SHA-256 |
| --- | --- |
| `sources/cantonese/jyut6ping3.chars.dict.yaml` | `9b053a594c80eae76545bcdd83239884a49b1ba48b89ca0ab9e8daac79f33013` |
| `sources/cantonese/jyut6ping3.words.dict.yaml` | `54d174ad2bb997e4a678b7b076b84e4dc5914481b32467cdea3b43e88b7d5474` |
| `sources/cantonese/essay-cantonese.txt` | `d08836175f598219f43c2f2f9e12e12711212dbefe08d57b2eddb7a9d9f22a5d` |
| `sources/zhuyin/tsi.csv` | `c889a1ac3ae1901b3f8f62748bc41b958f010bf995f7f88dbaf9e3494f341428` |
| `sources/zhuyin/word.csv` | `da55b8e599c1389bc486453554f3410cf9c621d0ffff0ce38855698d26b3892a` |
| `sources/zhuyin/phrase.occ` | `dcd11597090b2bef88fb385bdd567e235ccd916728c84b8d0d3f8069ccfc7b8d` |
| `sources/stroke/stroke.dict.yaml` | `b3e93dce89c185f45c3d6e189b86b3a8626913352cc85e1094c786579a665791` |

rime-cantonese 由粤语计算语言学基础建设组（[CanCLID](https://github.com/CanCLID)）开发和维护，按其 README，主体部分以 CC BY 4.0 发布（上游仓库的 `LICENSE-CC-BY`），拼写是香港语言学学会（LSHK）的[粤拼](https://www.lshk.org/jyutping)方案。上游另有按 ODbL 1.0 发布的 `jyut6ping3.maps.dict.yaml`，以及 `jyut6ping3.phrase.dict.yaml`、`jyut6ping3.lettered.dict.yaml`，这三个文件不收入本仓。上游默认分支是 `main`：`.github/workflows/fetch-upstream.yml` 每周把 [CanCLID/rime-cantonese-upstream](https://github.com/CanCLID/rime-cantonese-upstream) 的数据重建后推到 `main`，`release.yml` 从 `main` 打包并把 `latest` 标签移到 `main` 的最新提交；`master` 分支只有 2024-12-01 前的自动部署提交（“Deploying to master from @ rime/rime-cantonese@…”），之后不再更新，所以本仓改按 `main` 锁定。两个分支的 `LICENSE-CC-BY` 相同。

HKCanCor 是陆镜光（Luke Kang Kwong）整理的 1997–1998 年香港粤语对话与电台节目转写，按上游 `README.md` 与 `data/README`、`data/LICENSE` 以 CC BY 4.0 发布，要求引用：K. K. Luke and May L. Y. Wong (2015) The Hong Kong Cantonese Corpus: Design and Uses. Journal of Chinese Linguistics Monograph Series, 25, 312–333。本仓只收由它统计出的词频表 `sources/cantonese/hkcancor-word-counts.txt`，不收语料文本；转写文件由 msime 锁文件直接按上游提交固定。粤拼词库只用它给 `essay-cantonese.txt` 没收的词加权，换算方法见 msime `crates/dict-builder/src/cantonese.rs`。

McBopomofo 的 `phrase.occ` 是该项目在自己语料上统计的词语出现次数（上游 `Source/Data/README.md` 与 `AGENTS.md` 称之为 phrase frequency/occurrence data，`textpool.rc` 记着语料路径，语料本身不公开）；上游没有为数据单独声明许可，仓库根目录 `LICENSE.txt` 与 `README.markdown`“軟體授權”一节说明整个项目以 MIT 发布，Copyright (c) 2011-2026 Mengjuei Hsieh et al.。注音词库只用它给 `tsi.csv` 在任何读音下都没计数的词语加权。上游文档写它用 tab 分隔，实际文件用空格分隔，构建器两种都接受。

libchewing-data 的两个文件在文件头声明 `dc:rights,Copyright (c) 2025 libchewing Core Team` 与 `dc:license,LGPL-2.1-or-later`；上游仓库没有单独的许可证文件，许可以文件头为准。

rime-stroke 的上游仓库以 LGPL-3.0 发布（`LICENSE`）。`AUTHORS` 列出的作者是四季的風、雪齋、Kunki Chou 与宋天，并写明前三位的数据是依 CNS11643 全字库的授权声明（http://www.cns11643.gov.tw/AIDB/copyright.do）以 LGPL 再分发的。CNS11643 全字库网站以《政府資料開放授權條款－第1版》授权（见其[全字库授权](https://www.cns11643.gov.tw/pageView.jsp?ID=59)页），要求利用其资料时注明来源：數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw 。按 `stroke.dict.yaml` 的文件头：主码表源自 CNS11643 中文标准交换码全字库网站（http://www.cns11643.gov.tw），由 Kunki Chou 整理；附码表源自北大中文论坛，由孙海峰、徐孟罗、唐捺之、谢振斌整理；至扩展 J 区的超集扩充数据来自宋天；Rime 输入方案由四季的風、雪齋、Kunki Chou 制作。

msime 用 `msime-dict-build languages` 把它们构建成 `msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db`，与桌面词库一起放在本仓库的 `dict-v*` Release；随产物分发的署名、改动说明与许可证全文在 msime 的 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`（同时署名 HKCanCor）、`resources/licenses/libchewing-data-LGPL-2.1.txt`（同时收 McBopomofo 的 MIT 声明）、`resources/licenses/rime-stroke-LGPL-3.0.txt`，换上游提交时这些文件要一起改。`msime-stroke.db` 的排序权重取自 `sources/pinyin/single-chars.txt`。

粤拼库另按 `curation/` 下两张维护者表修改，源文件本身不改：`curation/cantonese-word-vetoes.txt` 在所有来源写入之后删掉 essay 里的错字写法（第一批 6 条，用「訓」代「瞓」、词表在同一读音上另有正确写法的词），`curation/cantonese-essay-keep.txt` 列出不受「essay 补词要有佐证」规则压低的常用口语词（第一批 66 个）。佐证之一是拼音库（rime-ice 词表与补充表、地名表）收了这个词的简体写法，读的是去掉 `curation/pinyin-vetoes.txt` 否决的读音之后的拼音行，所以拼音否决表也会影响 `msime-cantonese.db`；权重覆盖表 `curation/pinyin-weight-overrides.txt` 不影响。两张粤拼表列的都是 rime-cantonese 的词与读音，依据 CC BY 4.0 提供，署名同上。

## 日文与韩文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/japanese/dictionary00.txt` … `dictionary09.txt`、`sources/japanese/id.def`、`sources/japanese/connection_single_column.txt`、`sources/japanese/README.txt` | [google/mozc](https://github.com/google/mozc)，提交 `9fbd649bea4c5e99cd8ad5e487213b26a953a376` 的 `src/data/dictionary_oss/` 文件 | Mozc 仓库根目录 `LICENSE`：署名 Google Inc. 的三条款 BSD 文本适用于整个仓库，`src/data/dictionary*` 另附 IPAdic、ICOT 与冲绳辞书条款；后者与随附的 `README.txt` 所列条款相同 |
| `sources/japanese/aux_dictionary.tsv`、`sources/japanese/dictionary_filter.tsv`、`sources/japanese/places.tsv`、`sources/japanese/words.tsv` | [google/mozc](https://github.com/google/mozc)，同一提交 `9fbd649bea4c5e99cd8ad5e487213b26a953a376` 的 `src/data/dictionary_oss/aux_dictionary.tsv`、`src/data/dictionary_oss/dictionary_filter.tsv`、`src/data/dictionary_manual/places.tsv`、`src/data/dictionary_manual/words.tsv` | 与上一行相同：两份目录都在 `src/data/dictionary*` 之下 |
| `sources/japanese/LICENSE` | [google/mozc](https://github.com/google/mozc)，同一提交 `9fbd649bea4c5e99cd8ad5e487213b26a953a376` 的仓库根目录 `LICENSE`（git blob `15ef0f074d90c3856c3202a9ab1a8a349ae57570`） | 即上面两行所适用的许可文本本身 |
| `sources/korean/hanja.txt` | [libhangul/libhangul](https://github.com/libhangul/libhangul)，提交 `717409ce61524bb3d8426060a384822f21354c62` 的 `data/hanja/hanja.txt` | BSD-3-Clause |

`sources/japanese/` 与 `sources/korean/hanja.txt` 均逐字节保留上游文件。`msime-dict-build` 从 `sources/japanese/` 构建 `msime-japanese.dat`，并按 Mozc OSS 构建组装系统词典的方式使用后加的四个文件：`dictionary_filter.tsv` 从基础词库中删去匹配的行（`gen_filtered_dictionary.py`），`aux_dictionary.tsv`、`places.tsv`、`words.tsv` 生成补充词条（`gen_aux_dictionary.py`），`places.tsv` 与 `words.tsv` 来自 Mozc 的 `src/data/dictionary_manual/` 目录，保留上游文件名；从 `sources/korean/hanja.txt` 生成引擎内嵌的韩文 Hanja 表；日文词库发布时必须同时分发 `sources/japanese/README.txt`（附件名 `msime-mozc_dictionary_oss_README.txt`）与 `sources/japanese/LICENSE`：Google 三条款 BSD 文本要求的版权声明与免责声明不在这份 README 里，所以 Mozc 根目录的 `LICENSE` 逐字节收为 `sources/japanese/LICENSE`，作为 `msime-mozc_LICENSE.txt` 与 `msime-japanese.dat` 一起附在 `dict-v` 发布上；韩文数据的 BSD-3-Clause 文本见 msime 的 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`。更新任一上游提交时，需同步更新本仓的 `upstream.lock.json`，以及 msime 锁文件里对应的上游引用（Mozc 是 `mozc.commit`）、构建器路径和许可证说明。机器可读的上游提交与文件哈希记录在根目录的 `upstream.lock.json`，由 `scripts/check_upstream.py` 检查。

## 符号

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/symbols/wanxiang_symbols.yaml` | [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang)，标签 `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）的仓库根目录 `wanxiang_symbols.yaml`。文件头注明它最初复制自 Rime 发行版的 `symbols.yaml`（[rime/rime-prelude](https://github.com/rime/rime-prelude)），万象在其上扩充了分组 | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)（上游仓库 `LICENSE`）；最初的 `symbols.yaml` 部分为 rime-prelude 的 LGPL-3.0 |
| `sources/symbols/codex_sym.txt` | 同一提交的 `lua/data/codex_sym.txt`，由万象从 [typst/codex](https://github.com/typst/codex) 的符号命名数据生成 | CC BY 4.0（万象）；符号名称来自 Apache-2.0 的 typst/codex |
| `sources/symbols/LICENSE` | 同一提交的仓库根目录 `LICENSE` | 即 CC BY 4.0 许可文本本身 |
| `sources/symbols/wanxiang-unrenderable.txt` | 本项目生成的剔除清单：由 MSIME-Windows 的 `scripts/check-symbol-rendering.py` 用 Windows 自带字体检查上面两份文件里的符号，列出看不见、私用区、系统字体没有字形或不能合成一个字形的符号，判定环境与规则写在文件头 | 只含上面两份文件里的符号与出处，许可同上 |
| `sources/symbols/piliapp-symbols.txt` | 2026-08-17 采集自 [PiliApp 特殊符号](https://tw.piliapp.com/symbol/) 51 个分类页面上可直接复制的 Unicode 字符，按分类分组去重；原在 msime 的 `resources/dictionary-sources/symbols/piliapp_symbols.txt`，字节不变 | **未声明**，见下方「待解决」 |
| `sources/symbols/symbol-names.tsv` | 本项目自写：符号名称与拼音到符号，名称与码位是事实，没有整段复制任何现成的表 | MPL-2.0（全文见 msime 的 `resources/licenses/MPL-2.0.txt`），随 msime 引擎 `msime-engine` 内嵌分发 |
| `sources/symbols/latex-symbols.tsv` | 本项目自写：LaTeX 命令名到码位，命令名取自 LaTeX 内核与 amssymb、amsmath 等宏包的公开命令，不来自 unicode-math 或 fcitx5 `latex.mb` 的表 | MIT，Copyright (c) 2026 Metasequoia IME contributors；全文见 msime 的 `plugins/phrase_table/LICENSE` |

前三个文件与上游提交逐字节一致（UTF-8、LF）。后两个文件原在 msime 仓库，由 msime 的 `resources/symbol-tables.lock.json` 固定字节并同步回 msime。构建器只取 `wanxiang_symbols.yaml` 中 `symbols:` 下的符号分组，不收 Emoji 分类、元素周期表与 XCompose 合成三段，并剔除 `wanxiang-unrenderable.txt` 列出的全部符号。CC BY 4.0 要求保留署名并注明改动：随产物分发时在 MSIME-Windows 的 `THIRD_PARTY_NOTICES.txt` 中署名 rime-wanxiang 与 typst/codex。

## 下游影响

发布构建的拼音词库 `msime-pinyin.db` 由 `sources/pinyin/single-chars.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 与 `custom/words.txt` 构建，另含 msime 仓库自带的 `resources/dictionary-sources/mix/quick_phrases.txt` 快捷短语；`sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt`（含 CustomPinyinDictionary 的内容）不进入发布构建，见下方「发布构建不包含这些条目」。只有用 `--include-unlicensed` 做的本地完整构建才会读入这两份。中文数据主体来自 rime-ice（GPL-3.0），使用该数据库的前端本身以 GPL-3.0 分发，与 rime-ice 兼容，但**必须保留对 rime-ice 的署名**；与万象共有词条的权重来自 rime-wanxiang（CC BY 4.0），同样**必须保留对万象的署名并注明改动**（见上方「拼音权重与万象对齐」）；构建时还按 `curation/` 的表删词、改权重，见上方「维护者清洗表」。

词库由 [msime](https://github.com/metasequoiaime/msime) 的 Rust 构建器 `crates/dict-builder`（`msime-dict-build`）构建并以本仓库的 `dict-v*` release 发布（数据库按 `msime-<内容>` 命名），随产物送到用户手上的署名在 msime 的 `resources/licenses/` 与 [MSIME-Windows](https://github.com/metasequoiaime/MSIME-Windows) 的 `THIRD_PARTY_NOTICES.txt` 里。改动本文件的来源表时，这些文件要一起改。

## 待解决

以下部分目前没有明确的再分发授权，需要与上游作者确认后才能补上：

- [wuhgit/CustomPinyinDictionary](https://github.com/wuhgit/CustomPinyinDictionary) 是 `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt` 的主体。上游在 2026-09-30 的提交 `cf17f96af885cb818c2fad87184f383a52482351`（“Add License”）加入了 CC-BY-SA-4.0 许可，但本仓用的 `2023-09-28(No.82)` 快照早于这次加许可，许可是否覆盖这份快照没有确认，所以仍按未声明许可处理，继续排除在发布构建之外。
- `sources/symbols/piliapp-symbols.txt` 采集自 PiliApp 网站，网站没有给出数据许可。它和其余几项不同：不在 `sources/unlicensed/`，`licensing.rs` 也不排除它，而是照旧进入 `msime-others.db` 的符号页——这份数据在 msime 仓库里时就一直随发布分发，搬到本仓只是把来源和许可状态写明，由维护者决定先保持面板内容不变。内容是 Unicode 字符按主题的分组，可替换的来源是本仓已有授权的 `wanxiang_symbols.yaml` 与 `codex_sym.txt`（CC BY 4.0），但两者只覆盖它 2323 个符号中的 1138 个，动物、人员与活动、盲文、心形、分数等分类没有对应。确认授权或换成有授权的来源之前，不要往这份文件里加内容。
- `sources/unlicensed/single-char-whitelist.txt` 的来源没有记录。完整构建用它过滤单字条目，所以需要补上来源；在补上之前不要假定它可以再分发。
- `sources/unlicensed/oaldpe-words.txt` 提取自商业词典。词典本体曾以 `en/oaldpe.mdx` 放在本仓中，现已移除——构建只需要提取好的词形列表，不需要词典本体。提取脚本 `makecikudb/englishdb/extract_oaldpe_headwords.py` 已随旧的 Python 构建流程删除，需要时从 git 历史取回，自备 `.mdx` 作为参数运行。**注意移除只影响当前版本，词典本体仍留在 git 历史中。**改写历史会让所有 fork、clone 以及下游锁定的 commit 全部失效，因此暂不改写；是否改写单独决策。

### 发布构建不包含这些条目

上面这些条目除 `sources/symbols/piliapp-symbols.txt`（原因见该条）外，**默认不进入构建产物**。判定写在 msime 的 `crates/dict-builder/src/licensing.rs` 里，`msime-dict-build` 每次运行都会打印它排除了什么、为什么排除、以及换用了什么替代输入：

| 排除的输入 | 替代 | 后果 |
| --- | --- | --- |
| `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt` | `sources/pinyin/rime-ice.txt`（rime-ice，GPL-3.0） | 中文词库召回下降；rime-ice 是合并前的子集，构建不会失败 |
| `sources/unlicensed/single-char-whitelist.txt` | 无 | 不做过滤，`single-chars.txt` 里的单字全部收入 |
| `sources/unlicensed/oaldpe-words.txt` | 无 | 英文词表来自 `rime-ice-en.txt`、`sources/english/rime-ice-en-supplement.txt`、`sources/english/scowl-words.txt` 与 `custom/english.txt`，`sources/english/google-word-counts.txt` 提供词频 |

想构建完整词库（本地开发、评估召回率）用 `--include-unlicensed`，或设环境变量 `MSIME_DICT_INCLUDE_UNLICENSED=1`。**这样构建出来的产物不要附到 release 上。**

拿到上游的书面再分发许可之后，把对应条目从 `licensing.rs` 的 `UNLICENSED_INPUTS` 里移出，并在同一次改动里更新本文件。

### 上游状态与已知数据问题

以下各项不涉及再分发授权，记录在这里供更新上游或重新生成补充表时处理：

- [chewing/libchewing-data](https://github.com/chewing/libchewing-data) 的 GitHub 仓库已归档，仓库描述是 “Migrated to Codeberg”。`sources/zhuyin/tsi.csv`、`sources/zhuyin/word.csv` 仍按上面记录的 GitHub 提交锁定；Codeberg 上是否有更新的数据没有核实，换上游地址前要先核对。
- `sources/pinyin/rime-ice-supplement.txt` 生成时只排除了 `rime-ice.txt` 已有的同词同音行，没有对照 `sources/pinyin/single-chars.txt`：它的 8757 行单字中有 8740 对（字,拼音）与 `single-chars.txt` 重合，其中 725 对权重不同。msime 构建器把拼音词表逐行直接插入、不去重，这些单字在 `msime-pinyin.db` 里会出现两行。对候选排序的影响没有在引擎里测过。
- `sources/pinyin/rime-ice-supplement.txt` 沿用了 rime-ice `base.dict.yaml` 的容错读音，权重与正读相同，例如 `血型 xie'xing 94145`（正读 `血型 xue'xing 94145` 在 `sources/pinyin/rime-ice.txt`）。这是上游的行为，不是转换错误；是否在补充表里排除容错读音尚未决定。
- 发布拼音输入里有一批多音字读错的词条，例如「重」在“再次”“层”义下读 chong、「调」在“调整”义下读 tiao、「行」在“行列”“行业”义下读 hang，却被标成 zhong、diao、xing。错行留在按字节锁定的文件里不改，由 msime 构建器在构建时删除：清单在 msime 仓库的 `resources/dictionary-sources/pinyin-reading-corrections.txt`（每行 `词<TAB>错读<TAB>正读`），`msime-dict-build` 的 `reading-corrections` 阶段在全部拼音输入（含 `custom/words.txt`）并入之后删除每一条错读的全部行；某条错读一行也删不到（上游已改，条目过时），或删完后这个词没有正读的行，构建都会失败。清单目前 218 条，删除 218 行，分三类：
  - 本节早先记录的 24 条：`重采样 zhong'cai'yang`、`数据去重 shu'ju'qu'zhong`、`数组去重 shu'zu'qu'zhong`、`函数重载 han'shu'zhong'zai`、`可重入 ke'zhong'ru`、`可重入函数 ke'zhong'ru'han'shu`、`重入锁 zhong'ru'suo`、`重放攻击 zhong'fang'gong'ji`、`重签名 zhong'qian'ming`、`重编程 zhong'bian'cheng`、`重定时 zhong'ding'shi`、`末端重复 mo'duan'zhong'fu`、`重复精度高 zhong'fu'jing'du'gao`、`二十万军重入赣 er'shi'wan'jun'zhong'ru'gan`、`盛年不重来 sheng'nian'bu'zhong'lai`、`只缘妖雾又重来 zhi'yuan'yao'wu'you'zhong'lai`、`反调试 fan'diao'shi`（这 17 条的正读已在 `custom/words.txt` 末尾按错行权重补上），以及正读已在 `rime-ice-supplement.txt` 的 `调参 diao'can`、`调参侠`、`性能调优`、`数据库调优`、`重绘 zhong'hui`、`自动重连 zi'dong'zhong'lian`，和 `调优 diao'you`（10687，删除后正读 `调优 tiao'you` 9999 是这个词唯一的读音）。错行都在 `sources/pinyin/rime-ice.txt`。
  - rime-ice 自己改过读音的 182 条：错行在 `sources/pinyin/rime-ice.txt`，而 rime-ice 提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的 `cn_dicts/` 已没有这个读音、改收正读，正读随 `rime-ice-supplement.txt` 进入构建。只收按词义能判定的，例如 `重铬酸盐`、`重氮盐`、`重睑`、`重唇鱼` 的“重”读 chong，`重载列车`、`重钢结构`、`重装战士` 的“重”读 zhong，股价的 `回调`、`内调外养` 的“调”读 tiao，`行纪`、`首行缩进`、`太行路` 的“行”读 hang，`进行测量`、`时行感冒` 的“行”读 xing，`不长眼`、`徒长枝`、`长姐如母` 的“长”读 zhang，`三短一长`、`长上影线` 的“长”读 chang，`处理不了`、`上得了台面`、`了不了解吗` 的“了”读 liao。
  - 所有输入（包括 rime-ice `3aea6d3`）都只有错读的 12 条：`何日更重游`、`弃妾已去难重回`、`飞镜又重磨`、`两重心字罗衣`、`九重泉底龙知无`、`重楼翠阜出霜晓`（“重”读 chong），`行内元素`、`代码行数`、`单行注释`、`用品行业`、`文旅行业`、`直销行业服务网点设立管理办法`（“行”读 hang）。正读按错行的权重追加在 `custom/words.txt` 末尾。
  - 有歧义、没有处理的：`重设计 zhong'she'ji`（“重新设计”读 chong，“重视设计”读 zhong）、`重建设 zhong'jian'she`（同理）、`判重了 pan'zhong'le`（`rime-ice-supplement.txt`，“查重判定”读 chong，“判得重了”读 zhong）、`重配置 zhong'pei'zhi`、`重载版`、`重装上阵`、`重装秘术`、`霸者重装`、`重上君子堂`、`重鉴`；编程里的“回调”读 diao，所以 `出现回调`、`回调后`、`回调时`、`回调结束`、`快速回调` 两读都留着；`调侃儿 diao'kan'er`（《现代汉语词典》“说行话”义读 diào）、`改调解张`；`林花谢了春红`、`姓了`、`足了十人`；`全行`（“全银行”读 hang）、`修行业`；`增长睫毛`、`长恶不悛`、`长子线`、`张家长`（「张家长李家短」读 chang，「张＋家长」读 zhang；整句 `张家长李家短` 的错读已删除）、`甲长`（保甲的“甲长”读 zhang，龟鳖、甲壳类的“甲长”指背甲长度，读 chang）、`更无长物`（“长物”读 zhàng，rime-ice 新版改成的 chang 也不对）。人名、地名和品牌的读音无法从词义判定，也没有处理：`王行华`、`李时行`、`小西行长`、`徐行镇`、`刘行镇`、`曹行镇`、`高行中学`、`木村了`、`冯了性药酒`、`路学长`、`保长对应`、`快乐长门人`、`重野秀一`。`电子调速微型异步电动机通用技术条件`、`虢州岑二十七长史参三十韵` 两条错读只在 `rime-ice.txt` 里、不在 `--include-unlicensed` 构建换用的 `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`custom-pinyin-dictionary-part2.txt` 里，放进清单会让那种构建失败，权重都是 1，没有收。
  - 两读都对、不算错误的：单独的 `重载`（“再次加载”读 chong，“重载荷”读 zhong）、`调配`（调色调配读 tiao，人员调配读 diao）。

## 本项目自建部分

`custom/` 下的自定义词条、候选窗翻译覆盖、英文词条与人名，`curation/` 下维护者审定的否决表、权重覆盖表与粗俗词表，`packs/` 下由本项目自写的专业词库，以及 `scripts/validate_packs.py` 由本项目编写，依据 GPL-3.0 提供，与组织内其他仓库一致。`packs/birds/` 的 IOC 鸟名及其整理词表沿用 CC BY 3.0，见上方「可选鸟类名称词库」。`custom/cantonese.txt`（人工审校的粤语词与粤拼）是例外，以 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) 提供，署名为 msime-dictionary 贡献者：它由 msime 构建进 `msime-cantonese.db`，库里 rime-cantonese 与 HKCanCor 的数据都是 CC BY 4.0，用同一许可让整个库仍按单一许可分发；署名随库写在 msime 的 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`。

`sources/cn-en/machine-translations.txt` 是本项目整理的中译英释义，同样依据 GPL-3.0 提供。源词取自 `sources/pinyin/`、`sources/wubi/` 与 `custom/words.txt`（来源与许可见上文）；译文汇总了 `custom/translations.txt` 与人工整理的翻译，其余由大模型生成：DeepSeek（`deepseek-chat`、`deepseek-flash`、`deepseek-v4-pro`，按 DeepSeek 开放平台用户协议，输出内容的权利归使用者）与 ChatGPT（按 OpenAI 使用条款，输出归使用者），纯数字与年份按规则生成。生成的译文没有逐条人工审校，可能有误译；修正写到 `custom/translations.txt`，它在构建时覆盖本文件。

## OpenCC 简繁转换表

`sources/opencc/` 从 msime 的 `crates/client-base/data/opencc/` 原字节迁入，固定 [BYVoid/OpenCC](https://github.com/BYVoid/OpenCC) 提交 `26753884f1984add422f3b0249ccee8613deaff6`，许可为 Apache-2.0，全文保留在同目录 `LICENSE`。`CJK_Compatibility_Ideographs.txt`、`HKVariants.txt`、`STCharacters.txt`、`STPhrases.txt`、`TSCharacters.txt`、`TSPhrases.txt`、`TWPhrases.txt`、`TWVariants.txt`、`TWVariantsPhrases.txt` 是该提交 `data/dictionary/` 下的原样文本，其中 `TSCharacters.txt` 与 `TSPhrases.txt` 是后来为粤拼的繁转简输出从同一提交补入的，`HKVariants.txt` 是后来为粤拼词库的香港字形替换从同一提交补入的，都没有改写；`STPhrases_GeneratedFromRegionalPhrases.txt` 是同一提交的构建产物，由 `data/scripts/generate_st_phrases_from_regional_phrases.py` 使用 `t2s.json` 生成，迁移时不重新生成、不排序或改写。

这些表由 msime 构建时校验后内嵌，供 `s2t`、`s2tw`、`s2twp` 转换、粤拼输出简体时的繁转简（`TSPhrases` 与 `TSCharacters`，对应上游 `t2s.json` 去掉 `TSCharactersExt` 的部分）以及开发工具使用，不新增 `dict-v` Release 附件。`TWVariants.txt` 与 `HKVariants.txt` 另由 msime 的 `languages` 构建读取，只用来列出粤拼词库香港字形替换的候选异体对（换不换由 HKCanCor 词频决定，`custom/cantonese-variant-vetoes.txt` 列出的字不换），表的内容不写进 `msime-cantonese.db`；`HKVariants.txt` 不内嵌进客户端，不在 msime 的 `resources/opencc.lock.json` 里。更新时同步 msime 的 `resources/opencc.lock.json` 和许可证说明；保持上游文件名与逐字节锁定。

五笔补充数据（含新世纪五笔的全部词组）在发布构建时从本次固定原表、拼音和人工词条重新派生；本仓三份补充表为审计快照，来源和许可与上表相同。每周 rime-ice 更新预览生成审核候选，不改变本表记录的已固定源提交；候选通过审核并更新源数据时再同步来源、许可证和锁。

## msime 构建时另加的输入

下面几项不在 msime-dictionary 的 `NOTICE.md` 里：它们由 msime 仓库（`resources/dictionary-sources/`）或 msime 的 `resources/dictionary-sources.lock.json` 提供，构建器 `crates/dict-builder` 在同一次 `dict-v*` 构建里读入。

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `resources/dictionary-sources/kaomoji/kaomoji.txt`（`msime-others.db` 的颜文字） | [aoguai/rime_kaomoji_dict](https://github.com/aoguai/rime_kaomoji_dict) | MIT |
| `ecdict/ecdict.csv`（`msime-english.db` 的中英释义，由 `resources/dictionary-sources.lock.json` 固定） | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT)，提交 `bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b` | MIT |
| `resources/dictionary-sources/pinyin-overrides.txt`（表情、颜文字与符号关键词的整词读音） | [mozillazg/python-pinyin](https://github.com/mozillazg/python-pinyin) 0.55 的 `lazy_pinyin` 输出 | MIT |

构建器另有一组内置的英文蔑称（`crates/dict-builder` 的 `english_supplement::OFFENSIVE` 与 `curation::BUILT_IN_VETOES`，后者是 rime-ice 英文词表自己收的 nigger、tranny、beyatch），不靠 msime-dictionary 的 `curation/` 也总是生效：`english` 阶段从全部英文来源（`sources/english/rime-ice-en.txt`、`sources/english/rime-ice-en-supplement.txt`、`sources/english/scowl-words.txt`、OALDPE 词头和 `custom/english.txt`）删掉这些词，任何大小写都不写入 `msime-english.db`，它们的释义随之不出现，`custom/translations.txt` 里英文一侧含这些词的条目也不写。这是对 rime-ice（GPL-3.0）英文词表的修改，上面那份 `NOTICE.md` 只写了 `curation/english-vetoes.txt` 的删词，所以记在这里。

### SCOWL 版权声明

`msime-english.db` 的英文词表含有取自 SCOWL 的词。SCOWL 的条款要求在随附文档中保留下面的版权声明与许可声明（原文照录）。msime-dictionary 的 `dict-v` 发布把许可全文作为 `msime-scowl_Copyright.txt` 与 `msime-english.db` 一起附上：

```text
Copyright 2000-2026 by Kevin Atkinson

Permission to use, copy, modify, distribute, and sell any part of SCOWLv2, or
word lists created from it, is hereby granted without fee, provided that the
above copyright notice appears in all copies and that both the above
copyright notice and this notice appear in supporting documentation.  Kevin
Atkinson makes no representations about the suitability of this database for
any purpose.  It is provided "as is" without express or implied warranty.
```

Aspell 词典包里的 `Copyright` 文件另含 Benjamin Titze 的澳大利亚英语数据声明（与上面同类的宽松条款）和 Aspell 词缀文件的 Ispell 声明；构建不读取澳大利亚词表和词缀文件，全文仍原样收在 `resources/licenses/scowl-aspell6-en-Copyright.txt`。

### 整句词格的上下文表

`msime-bigram.bin` 与 `msime-trigram.bin` 不由 msime-dictionary 的文本构建，而是在构建时统计一份固定的语料：

| 语料 | 上游 | 上游许可 |
| --- | --- | --- |
| `ngram-corpus/`（`ngram` stage 的输入，只在构建缓存目录里，不入库） | [中文维基百科 20260901 pages-articles dump](https://dumps.wikimedia.org/zhwiki/20260901/) | CC-BY-SA 4.0（正文另受 GFDL 约束） |

第三方直链输入的版本、文件清单与 SHA-256 在 msime 仓库的 `resources/dictionary-sources.lock.json` 里，由 Rust 构建器 `crates/dict-builder`（`msime-dict-build`）下载并逐个校验；msime-dictionary 的文件由构建所用 checkout 的 Git 提交固定（release manifest 的 `custom_dictionary_commit`），其中上游原样文件另由该仓 `upstream.lock.json` 按 SHA-256 固定。

产物是词序列的统计量（相邻词对/词三元组的对数增量），不含语料原文。**再分发这两个文件时必须保留对中文维基百科的署名，并按 CC-BY-SA 4.0 提供该文件本身**；BY-SA 4.0 单向兼容 GPL-3.0，与前端现有的 GPL-3.0 分发方式相容。

选 zhwiki 而不是 C4 中文部分的理由不是授权而是测量：评测收割集 `sentences-v2.tsv`（客户端）本身就是从 C4 收割的，拿 C4 统计上下文等于让解码器把唯一的硬基准当训练集看。


香港街道与地方名称（香港特别行政区政府地政总署，CSDI Portal）
================================================================
数据及其知识产权归香港特别行政区政府所有。来源为地政总署通过 CSDI Portal 发布的
Road Centreline 与 Place Name，快照取得于 2026-11-05：
https://portal.csdi.gov.hk/geoportal/?datasetId=landsd_rcd_1637310758814_80061
https://portal.csdi.gov.hk/geoportal/?datasetId=landsd_rcd_1648571595120_89752

msime-dictionary 的 sources/hong-kong/ 保存原始名称、来源摘要、使用条款与派生词表。
本项目对中文名称去重、清理排版分隔符、补粤拼和普通话读音、生成简体字形并以低权重
补入词库；不包含坐标、门牌或用户地址。地名名称数据适用以下 CSDI 使用条款，
不改以 CC BY 4.0 或 GPL 再授权；字音的既有来源许可保持。下载、复制或分发这些
名称时请保留政府、地政总署、CSDI Portal 署名、以上来源及以下使用条款。

Terms and Conditions of Use of the CSDI Portal
https://portal.csdi.gov.hk/csdi-webpage/doc/TNC
Retrieved: 2026-11-05

General

The CSDI Portal is an online one-stop geospatial information service provided by the Development Bureau of the Government of the Hong Kong Special Administrative Region (the “Government”) to the public for obtaining geo-referenced information.

In these Terms and Conditions of Use of the CSDI Portal (“Terms of Use”):

“Data” include the “Spatial Data

#

” and all other contents provided by the Government and/or other organisations (such organisations are referred to as “the relevant organisations”) to the public on the CSDI Portal;

“Spatial Data” include all data in the form of digital maps, text, graphics, drawings, diagrams, photographs, compilation of data, data specifications, metadata and Application Programming Interface (“API”).

Use of Data on CSDI Portal

You are allowed to browse, download, distribute, reproduce, hyperlink to and print the Data for both commercial and non-commercial purposes on a free-of-charge basis on the condition that:-

you shall comply with the Terms of Use;

you shall identify clearly the Government and the CSDI Portal as the source of the Data and acknowledge the Government and the relevant organisations’ ownership of the intellectual property rights in the Data and in all copies thereof including but not limited to paper copies, digital copies and copies placed on other websites;

you shall indemnify the Government and the relevant organisations against any allegations or claims of infringement of the rights of any person and all costs, losses, damages and liabilities incurred by the Government and the relevant organisations, which in any case arise directly or indirectly in relation to your use, reproduction and/or distribution of the Data

Disclaimer and Limitation of Liability

The Data are provided by the Government and the relevant organisations to you on an “as is” and “as available” basis. No statement, representation or warranty of any kind (whether express or implied) is given by the Government or the relevant organisations in relation to the Data (including its accuracy, correctness, completeness, non-infringement, reliability, security, timeliness and appropriateness of the use of the Data in any particular circumstances). The Government or the relevant organisations shall not be liable for any errors, omissions, deficiencies, inconsistencies, misstatements or misrepresentations (express or implied), concerning any Data, and shall not have or accept any liability, obligation or responsibility whatsoever for any loss, destruction or damage (including without limitation consequential loss, destruction or damage or other costs or expenses) howsoever arising from or in respect of any use or misuse of or reliance on the Data, or any inability to use the Data (including any breakdown, suspension, non-function, mal-function or interruption of operation of the CSDI Portal).

In consideration of the Government and the relevant organisations providing the Data on the CSDI Portal, by using the CSDI Portal, including the tools, you hereby unconditionally accept the Terms of Use.

IF YOU DO NOT AGREE TO THE TERMS OF USE, PLEASE DO NOT USE THE CSDI PORTAL, INCLUDING THE TOOLS, OR THE DATA PROVIDED THEREON.

The Terms of Use may be revised and/or amended from time to time by the Government or the relevant organisations without prior notice to any person. Accessing or using or linking to the CSDI Portal or using any Data after such amendments constitutes acceptance of such amendments. Please check the Terms of Use regularly for any revisions and/or amendments which may be made.

The Government and the relevant organisations shall not be responsible for any loss or damage whatsoever arising out of or in connection with the Data. The Government reserves the right to revise, omit or edit the Data or to suspend or terminate the provision of the Data, the CSDI Portal or any part thereof at any time in its absolute discretion without giving any reason or prior notice to any person (including you).

Linked Information and other Third-Party Websites

The CSDI Portal may provide or assist in providing links to third-party websites through the CSDI Portal. The links are produced solely as a convenience to users of the CSDI Portal. Provision of, or assistance in providing, any links at the CSDI Portal to any third-party website does not give rise to any statement, representation or warranty, express or implied, that the Government agrees with, approves, recommends or endorses the contents of any such third-party websites. The provision of any links to third-party websites shall not constitute any form of co-operation or affiliation with the Government of any such third parties or external websites. The Government shall not have or accept any liability, obligation or responsibility whatsoever for any loss, destruction or damage (including without limitation consequential loss, destruction or damage or other costs or expenses) howsoever arising from or in respect of your use or misuse of or reliance on, or inability to use such links to any third-party website or your use or reliance on the contents of any such third-party websites. You should refer to the terms and conditions of each such third-party website when using such website.

Privacy Policy

The Government may record visits to the CSDI Portal without collecting your personal data. Information relating to visits to the CSDI Portal are collected by the Government for the compilation of statistical reports and the investigation and diagnosis of problems with or concerning computer systems.

Intellectual Property Rights

Unless otherwise indicated, the Government is the owner of the intellectual property rights of all contents available on the CSDI Portal, including but not limited to all Data.

Governing Law

The Terms of Use shall be governed and construed in accordance with the laws of the Hong Kong Special Administrative Region of the People’s Republic of China (“Hong Kong”). You also agree to submit to the exclusive jurisdiction of the Hong Kong Courts.

Chinese text

In the event of any conflict or inconsistency between the English text and the Chinese translation of these Terms of Use, the English text shall prevail.

Note

“Spatial data” is defined with reference to the definition of “Geospatial Data” published by the Open Geospatial Consortium as “location properties related to any terrestrial feature or phenomena”. “Location properties” may include any information about the location or area of, and relationships among, and descriptive information about geographic features and phenomena. This includes remotely sensed data, vector map data, addresses, coordinates, etc. Note that “geospatial data” is more precise in many contexts than “geographic data” because geospatial data is often used in ways that do not involve a graphic representation, or map, created from the data.”
