# 来源与授权说明

本仓库**不对外提供统一的开源许可**，因为其中绝大部分词库并非本项目的作品，而是从其他项目收集、合并、去重而来（见 [README](README.md)）。给整个仓库挂一份 LICENSE 等于替上游作者重新授权，因此这里改为逐项说明来源与上游条款。使用或再分发本仓库的数据时，请以对应上游的条款为准。

## 中文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `source/BaseDict.txt`、`cn/BaseDictV1.txt` | [wuhgit/CustomPinyinDictionary](https://github.com/wuhgit/CustomPinyinDictionary) | **未声明** |
| `source/BaseDictIce.txt`、`sources/pinyin/rime-ice.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice) | GPL-3.0 |
| `sources/pinyin/rime-ice-supplement.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的开源词条，排除 `rime-ice.txt` 已有的同词同音行 | GPL-3.0 |
| `sources/pinyin/places.txt` | [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China)，提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` 的 `dist/provinces.csv`、`dist/cities.csv`、`dist/areas.csv`，由 `crates/dict-builder` 的 `places-supplement` 子命令生成行政区划全称与简称，只收 `rime-ice.txt`、`rime-ice-supplement.txt` 没有、或权重不超过 10 且低于分层下限的词（已有排名的词不抬升），构建时按只升不降并入 | WTFPL（区划名称，`resources/licenses/Administrative-divisions-of-China-WTFPL.txt`）；读音的证实与权重下限依据 rime-ice 的 `rime-ice.txt`、`rime-ice-supplement.txt`（GPL-3.0） |
| `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt` | 上面两者合并去重 | GPL-3.0 与**未声明**的混合 |
| `sources/pinyin/single-chars.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，读音以 [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) 校正 | GPL-3.0 + MIT |
| `source/SampleIMESimplifiedQuanPin.txt` | [microsoft/Windows-classic-samples](https://github.com/microsoft/Windows-classic-samples) | MIT |
| `sources/wubi/wubi86-jidian.txt`（`msime-wubi.db` 的 `wubi86` 表） | [KyleBing/rime-wubi86-jidian](https://github.com/KyleBing/rime-wubi86-jidian) | Apache-2.0 |
| `sources/wubi/wubi86-supplement.txt`（排在 `sources/wubi/wubi86-jidian.txt` 之后进入 `msime-wubi.db` 的 `wubi86` 表） | 由 `crates/dict-builder` 的 `wubi86-supplement` 子命令生成：词条取 `sources/wubi/wubi86-jidian.txt` 没有、而 `sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt` 收录，或 `sources/pinyin/rime-ice.txt` 权重不低于 5000 的二字词，编码按 86 版词组规则由 `sources/wubi/wubi86-jidian.txt` 的单字全码推出 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice、GPL-3.0-or-later 的 Fcitx 表与 Unlicense 的 98 五笔表；编码依据 Apache-2.0 的 jidian 单字码） |
| `sources/wubi/wubi98.txt`（`msime-wubi.db` 的 `wubi98` 表） | [yanhuacuo/98wubi-tables](https://github.com/yanhuacuo/98wubi-tables)（98五笔小组）提交 `6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697` 的 `98五笔含词表-【单义】.txt`，逐字节未改；同一作者 2021 年也把这份表贡献给 [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)（`tables/wubi98.txt`） | Unlicense。权利声明来自数据作者本人；上游没有记录基础单字编码与简码的生成过程，只能确认它们依照 98 版五笔编码规范 |
| `sources/wubi/wubi98-fcitx.txt`（与 `sources/wubi/wubi98.txt` 合并进入 `msime-wubi.db` 的 `wubi98` 表） | [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)，提交 `dbc7154a7f0b9fc04313160ae8066ed4d8cbc446` 的 `tables/wubi98.txt` | GPL-3.0-or-later（随上游仓库的 `LICENSES/GPL-3.0-or-later.txt`） |
| `cn/53013_single.txt` | Unicode 收录的汉字单字表 | 数据本身来自 Unicode 标准；不被任何构建阶段读取，因此不进入任何产物 |
| `sources/unlicensed/single-char-whitelist.txt` | **待确认**，见下方「待解决」 | **待确认** |
| `source/FanyExtDict.txt`、`cn/phrases.txt` | 本项目自建 | 见下方「本项目自建部分」 |
| `cn/HelpCode.txt` | 规则参考小鹤形码 | 权利归小鹤方案作者 |

`japanese_lexicon` 表曾经在构建时从下面这个引用仓库取。Rust 构建器 `crates/dict-builder` 不再构建这张表，发布的授权构建也从来不含它：

| 引用仓库 | 上游 | 上游许可 |
| --- | --- | --- |
| `rime-jp_sela`（`jp_sela.dict.yaml`） | [Selaube/rime-jp_sela](https://github.com/Selaube/rime-jp_sela) | **未声明** |

## 英文与符号词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/english/rime-ice-en.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice) | GPL-3.0 |
| `sources/english/scowl-words.txt`（`msime-english.db` 的 `english_words` 表，许可文本另存于该库的 `source_notices` 表） | [en-wl/wordlist](https://github.com/en-wl/wordlist)（SCOWL）发布 `rel-2026.02.25`（提交 `7e99edab8e32f9f9ea2b15f249ca8d4d67237410`）的 Aspell 英文词典 `aspell6-en-2026.02.25-0.tar.bz2`，即 SCOWL 60 级官方拼写检查词典；由 `crates/dict-builder` 的 `english-supplement` 子命令取其中美式拼写与英式 -ise 拼写的纯字母词形，排除 `rime-ice-en.txt`、`rime-ice-en-supplement.txt` 与 `custom/english.txt` 已有或被 rime-ice 注释掉的词、蔑称、只有大写形式且拼出中文词全拼的专名，以及没有 Google 词频的词 | SCOWL 许可：可自由使用、复制、修改、分发和出售，条件是在所有副本中保留版权声明，并在随附文档中同时保留版权声明与许可声明（见下方「SCOWL 版权声明」，全文见 `resources/licenses/scowl-aspell6-en-Copyright.txt`）。60 级官方词典不涉及 UKACD 条款（只适用于 80 级以上的词表） |
| `sources/english/google-word-counts.txt` | [Google 1/3 million 词频表](https://www.norvig.com/ngrams/count_1w.txt) | 以来源页面说明为准 |
| `sources/unlicensed/oaldpe-words.txt` | 自 oaldpe.mdx 提取的词形列表 | 权利归词典出版方 |
| `kaomoji/` | [aoguai/rime_kaomoji_dict](https://github.com/aoguai/rime_kaomoji_dict) | MIT |
| 候选翻译数据 | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT) | MIT |
| `resources/dictionary-sources/pinyin-overrides.txt`（emoji、颜文字与符号关键词的整词读音） | [mozillazg/python-pinyin](https://github.com/mozillazg/python-pinyin) 0.55 的 `lazy_pinyin` 输出 | MIT |

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

## 整句词格的上下文表

`msime-bigram.bin` 与 `msime-trigram.bin` 不由本仓库的文本构建，而是在构建时统计一份固定的语料：

| 语料 | 上游 | 上游许可 |
| --- | --- | --- |
| `ngram-corpus/`（`ngram` stage 的输入，只在构建缓存目录里，不入库） | [中文维基百科 20260901 pages-articles dump](https://dumps.wikimedia.org/zhwiki/20260901/) | CC-BY-SA 4.0（正文另受 GFDL 约束） |

第三方直链输入的版本、文件清单与 SHA-256 在 msime 仓库的 `resources/dictionary-sources.lock.json` 里，由 Rust 构建器 `crates/dict-builder`（`msime-dict-build`）下载并逐个校验；msime-dictionary 的文件由构建所用 checkout 的 Git 提交固定（release manifest 的 `custom_dictionary_commit`），其中上游原样文件另由该仓 `upstream.lock.json` 按 SHA-256 固定。

产物是词序列的统计量（相邻词对/词三元组的对数增量），不含语料原文。**再分发这两个文件时必须保留对中文维基百科的署名，并按 CC-BY-SA 4.0 提供该文件本身**；BY-SA 4.0 单向兼容 GPL-3.0，与前端现有的 GPL-3.0 分发方式相容。

选 zhwiki 而不是 C4 中文部分的理由不是授权而是测量：评测收割集 `sentences-v2.tsv`（客户端）本身就是从 C4 收割的，拿 C4 统计上下文等于让解码器把唯一的硬基准当训练集看。

## 下游影响

由 `sources/unlicensed/custom-pinyin-dictionary-part1.txt` 与 `sources/unlicensed/custom-pinyin-dictionary-part2.txt` 构建出的 `msime-pinyin.db` 同时包含 rime-ice（GPL-3.0）与 CustomPinyinDictionary（未声明许可）的内容，其 `japanese_lexicon` 表还包含 rime-jp_sela（未声明许可）的内容。使用该数据库的前端本身以 GPL-3.0 分发，与 rime-ice 兼容，但**必须保留对 rime-ice 的署名**。

三个前端都分发这份数据，署名各自落在这些文件里：

| 前端 | 分发的数据 | 署名现状 |
| --- | --- | --- |
| [MSIME-Linux](https://github.com/metasequoiaime/MSIME-Linux) | DEB／RPM 包内的 `msime-pinyin.db`、`msime-wubi.db`、`msime-others.db`、`msime-english.db` 与辅助码 | `THIRD_PARTY_NOTICES.txt`，已覆盖词库 |
| [MSIME-Apple](https://github.com/metasequoiaime/MSIME-Apple) | app bundle 内的 `msime-pinyin.db`、`msime-wubi.db` 与辅助码 | `THIRD_PARTY_NOTICES.txt`，已覆盖词库 |
| [MSIME-Windows](https://github.com/metasequoiaime/MSIME-Windows) | 安装包内的 `msime-pinyin.db`、`msime-wubi.db`、`msime-others.db`、`msime-english.db`、`msime-japanese.dat` 与辅助码 | `THIRD_PARTY_NOTICES.txt`，已覆盖词库，由安装包装到程序目录 |

改动本文件的来源表时，这几份文件要一起改；它们才是随产物送到用户手上的那一份。

## 待解决

以下部分目前没有明确的再分发授权，需要与上游作者确认后才能补上：

- [wuhgit/CustomPinyinDictionary](https://github.com/wuhgit/CustomPinyinDictionary) 未声明任何许可，而它是 `msime-pinyin.db` 的主体。
- [Selaube/rime-jp_sela](https://github.com/Selaube/rime-jp_sela) 未声明任何许可，`msime-pinyin.db` 的 `japanese_lexicon` 表由它构建。
- `sources/unlicensed/single-char-whitelist.txt` 的来源没有记录。它参与 `msime-pinyin.db` 的构建（Rust 构建器 `crates/dict-builder` 的 `quanpin` 阶段在 `--include-unlicensed` 构建里用它过滤单字条目，并补上 `resources/dictionary-sources/cn/SingleCharWhitelist.additions.txt` 里的字），所以需要补上来源；在补上之前不要假定它可以再分发。因此 msime 仓库不存放这份文件，构建时从 msime-dictionary checkout（`--dictionary`）读取。
- `sources/unlicensed/oaldpe-words.txt` 提取自商业词典。词典本体 `en/oaldpe.mdx` 曾经也在本仓中，现已移除——构建只需要提取好的词形列表，不需要词典本体。需要重新生成词表时，自备 `.mdx` 并作为参数传给 `makecikudb/englishdb/extract_oaldpe_headwords.py`。**注意移除只影响当前版本，该文件仍留在 git 历史中。**改写历史会让所有 fork、clone 以及下游 `product-lock.json` 里锁定的 commit 全部失效，因此暂不改写；是否改写单独决策。
- 辅助码规则参考自小鹤形码，权利归方案作者。

### 构建默认不再包含这些条目

上面这些条目现在**默认不进入构建产物**。判定写在 `crates/dict-builder/src/licensing.rs` 里，`msime-dict-build` 每次运行都会打印它排除了什么、为什么排除、以及换用了什么替代输入：

| 排除的输入 | 替代 | 后果 |
| --- | --- | --- |
| `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`custom-pinyin-dictionary-part2.txt` | `sources/pinyin/rime-ice.txt`（rime-ice，GPL-3.0） | 中文词库召回下降；rime-ice 是合并前的子集，构建不会失败 |
| `sources/unlicensed/single-char-whitelist.txt` | 无 | 不做过滤，`single-chars.txt` 里的单字全部收入 |
| `sources/unlicensed/oaldpe-words.txt` | 无 | 英文词表只来自 `rime-ice-en.txt`、`rime-ice-en-supplement.txt`、`scowl-words.txt` 与 `custom/english.txt` |
| `rime-jp_sela` | 无 | Rust 构建器没有 `japanese-lexicon` 阶段，`msime-pinyin.db` 不含该表 |

想构建完整词库（本地开发、评估召回率）用 `--include-unlicensed`，或设环境变量 `MSIME_DICT_INCLUDE_UNLICENSED=1`。**这样构建出来的产物不要附到 release 上。**

拿到上游的书面再分发许可之后，把对应条目从 `crates/dict-builder/src/licensing.rs` 的 `UNLICENSED_INPUTS` 里移出，并在同一次改动里更新本文件。

这一节此前写的是「未决条目并不妨碍产品当前正在分发这些数据」。那句话如实记录了当时的状态，但那个状态本身就是风险所在——它是所有发行版渠道的硬门槛，也是唯一一条可能导致已发布产物被要求下架的问题。现在默认构建只包含本项目有权再分发的数据，未决条目仍然要跟上游谈，但发版风险不再取决于谈判进度。

## 本项目自建部分

`source/FanyExtDict.txt`、`cn/phrases.txt`、`resources/dictionary-sources/` 下的人工维护条目、词库源仓库 [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 `custom/words.txt`、`custom/translations.txt` 与 `custom/english.txt`（构建时从 `--dictionary` 给出的 msime-dictionary checkout 读取，发布 manifest 记下其提交），以及构建词库的 Rust 构建器 `crates/dict-builder`（取代原先 `makecikudb/` 下的 Python 脚本）由本项目编写，依据 GPL-3.0 提供，与组织内其他仓库一致。
