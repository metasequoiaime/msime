> 本文件是 msime 随词库分发的来源与授权说明。从下一行「来源与授权说明」起到「msime 构建时另加的输入」之前，逐字取自 [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 提交 `8e8b7817c91600913625edb7995a7397fbb6899a` 的 `NOTICE.md`，这个提交是 `resources/desktop-dictionary.lock.json` 与 `resources/language-dictionaries.lock.json` 所固定的 `dict-v2.0.41` 发布的源数据提交（发布说明与 `msime-dictionary-manifest.json` 都记着它）；其中的相对路径指那个仓库。「msime 构建时另加的输入」一节不在那份文件里，记的是由 msime 仓库一侧进入同一批词库的来源。词库锁换到新的 `dict-v*` 时，按那次发布的源数据提交重新同步本文件。

# 来源与授权说明

本仓库**不对外提供统一的开源许可**，因为其中绝大部分词库并非本项目的作品，而是从其他项目收集、合并、去重而来（见 [README](README.md)）。给整个仓库挂一份 LICENSE 等于替上游作者重新授权，因此这里改为逐项说明来源与上游条款。使用或再分发本仓库的数据时，请以对应上游的条款为准。

## 中文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/pinyin/rime-ice.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，对照提交 `9e66b0729083b37d217312294f6d516c8d7234be`（见下）；与万象共有词条的权重取自 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0；共有词条的权重为 CC BY 4.0（rime-wanxiang） |
| `sources/pinyin/rime-ice-supplement.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的 `cn_dicts/base.dict.yaml`、`ext.dict.yaml`、`8105.dict.yaml`、`others.dict.yaml`，排除 `rime-ice.txt` 已有的同词同音行；与万象共有词条的权重取自 rime-wanxiang `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0；共有词条的权重为 CC BY 4.0（rime-wanxiang） |
| `sources/pinyin/places.txt` | [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China)，提交 `c49d495b40ac73eb1a66f6eeae5f8fd10696f035` 的 `dist/provinces.csv`、`dist/cities.csv`、`dist/areas.csv`（原始数据来自国家统计局的统计用区划代码）。由 msime `crates/dict-builder` 的 `msime-dict-build places-supplement` 生成省、地、县三级区划的全称与去后缀简称，读音须由 `rime-ice.txt`、`rime-ice-supplement.txt` 证实，只收这两份文件没有、或权重不超过 10 且低于分层下限的词，这两份文件已有排名（权重大于 10）的词不抬升；这两份文件里同一全拼的其他词权重超过 `rime-ice.txt` 全部行权重的第 90 百分位（常用线，由生成器从数据算出）时，地名权重降到比该词低 1，不抢常用词的首位，比较时跳过权重为占位值的词（`rime-ice.txt` 里权重超过常用线的行中被超过 1% 的行共用的单个权重值，由生成器从数据算出，当前为 9999）；规则与对照文件的 SHA-256 写在文件头 | WTFPL（区划名称）；读音的证实与权重下限依据 rime-ice 的 `rime-ice.txt`、`rime-ice-supplement.txt`（GPL-3.0） |
| `sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt` | [wuhgit/CustomPinyinDictionary](https://github.com/wuhgit/CustomPinyinDictionary)（`2023-09-28(No.82)` 版）与 rime-ice 合并去重 | GPL-3.0 与**未声明**的混合 |
| `sources/pinyin/single-chars.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，读音以 [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) 校正；与万象共有读音的字频取自 rime-wanxiang `v18.0.16`（见下方「拼音权重与万象对齐」） | GPL-3.0 + MIT；共有读音的字频为 CC BY 4.0（rime-wanxiang） |
| `sources/wubi/wubi86-jidian.txt` | [KyleBing/rime-wubi86-jidian](https://github.com/KyleBing/rime-wubi86-jidian) | Apache-2.0 |
| `sources/wubi/wubi86-jidian.txt` 新增词条来源 | [rime/rime-wubi](https://github.com/rime/rime-wubi)，提交 `152a0d3f3efe40cae216d1e3b338242446848d07` | LGPL-3.0 |
| `sources/wubi/wubi86-supplement.txt` | 本项目生成的补充表：由 msime `crates/dict-builder` 的 `wubi86-supplement` 子命令从本仓 `sources/wubi/wubi86-jidian.txt`、`sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt` 生成。词条取 86 表没有、而 98 表收录，或 rime-ice 主表与补充表权重不低于 100 的两字及以上词，或区划补充表与人工词条（不限权重；只收构建器支持的基本区汉字），编码按 86 版词组规则由 86 表的单字全码推出 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice 与人工词条、WTFPL 的区划名称、GPL-3.0-or-later 的 Fcitx 表与 Unlicense 的 98 五笔表；编码依据 Apache-2.0 的 jidian 单字码） |
| `sources/wubi/wubi98-supplement.txt` | 本项目生成的补充表：由 msime `crates/dict-builder` 的 `wubi98-supplement` 子命令从本仓 `sources/wubi/wubi98.txt`、`sources/wubi/wubi98-fcitx.txt`、`sources/wubi/wubi86-jidian.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 和 `custom/words.txt` 生成。词条取两张 98 表都没有、而 86 表收录，或 rime-ice 主表与补充表权重不低于 100 的两字及以上词，或区划补充表与人工词条（不限权重；只收基本区汉字），编码按词组规则由 98 表的单字全码推出 | GPL-3.0（词条依据 GPL-3.0 的 rime-ice 与人工词条、WTFPL 的区划名称及 Apache-2.0 的 jidian 表；编码依据 Unlicense 的 98 五笔表和 GPL-3.0-or-later 的 Fcitx 表） |
| `sources/wubi/wubi98.txt` | [yanhuacuo/98wubi-tables](https://github.com/yanhuacuo/98wubi-tables)（98五笔小组），提交 `6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697` 的 `98五笔含词表-【单义】.txt`（见下） | Unlicense |
| `sources/wubi/wubi98-fcitx.txt` | [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)，提交 `dbc7154a7f0b9fc04313160ae8066ed4d8cbc446` 的 `tables/wubi98.txt` | GPL-3.0-or-later（随仓库的 `LICENSES/GPL-3.0-or-later.txt`） |
| `sources/unlicensed/single-char-whitelist.txt` | **待确认**，见下方「待解决」 | **待确认** |

`sources/pinyin/rime-ice.txt` 的上游提交是后来补记的：此前它是唯一没有被记录提交的中文词库来源，没有办法判断树里那份是从哪个版本导出的。按 rime-ice `9e66b0729083b37d217312294f6d516c8d7234be` 比对，它的前身 `BaseDictIce.txt` 有 28185 条上游不存在的词条，另有 8328 条权重与上游不同。多出来的条目里既有上游没收的真词（地名、品牌），也有语料切分留下的碎片（例如「米高」「钟的」「之事」）。已确认的碎片在 msime-engine 的 `67fd083` 中从当时的 `cn/BaseDictIceV1.txt` 与 `cn/BaseDictAllV1Part1.txt`（即现在的 `sources/pinyin/rime-ice.txt` 与 `sources/unlicensed/custom-pinyin-dictionary-part1.txt`）删除，本仓库的这两个文件与 msime-engine `e92a9c7c64e262f30218ca9aaad5f40d0b18cf89` 的 `dictionary/cn/` 逐字节一致。这份文件的真实来源仍然没有查清，所以不要把它描述成「原样搬运」。

`sources/wubi/wubi98-fcitx.txt` 保留 Fcitx5 table-extra 的 UTF-8 原始表格式（表头、规则和数据区），构建器只读取数据区的 `编码 空格 词` 行，并与 98 五笔主表按“编码、词语”去重。该表补足主表没有的合法开源候选，例如 `ukuy` 的“冲凉”，同时保留 Fcitx 表中的完整候选集合，不把单个词条作为特殊补丁。

`sources/wubi/wubi98.txt` 与上游文件逐字节一致（SHA-256 `1b5a4c22eddae08d8e0e6aa8f926f92d45d6cdaa591e5cee0e8aad29271da1cc`，UTF-16LE 带 BOM、CRLF），没有改编码或换行。权利声明来自数据作者本人：上游仓库在 2024-01-20 的提交 `4dbcaa65` 加入 Unlicense，此前同一作者在 2021 年把这份表贡献给 [fcitx/fcitx5-table-extra](https://github.com/fcitx/fcitx5-table-extra)（`tables/wubi98.txt`，GPL-3.0-or-later 包内分发，Debian main 收录）。两份表 112584 条里只有 4 条不同，差异来自上游 2024-06-25 的拆分校正（`72a36bae`）。上游没有记录 GB2312/GBK 部分的单字编码与简码是怎样生成的，只能说它们依照 98 版五笔的编码规范；这里如实记下，不把它描述成独立生成。界面上称“98 五笔”，不使用“王码”“五笔字型”等商标。

### 万象词库原始文件

`sources/pinyin/wanxiang/` 下的以下 15 份文件全部来自 **万象词库 / 万象拼音（rime-wanxiang），作者 amzxyz**，直接来源为 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 提交 [`aa9947f3f513ff88a3f7ce4e342fcb519d517025`](https://github.com/amzxyz/rime-wanxiang/tree/aa9947f3f513ff88a3f7ce4e342fcb519d517025/dicts) 的 `dicts/` 目录。上游 README 将词库来源标为同一作者的 [万象词库（amzxyz/RIME-LMDG）](https://github.com/amzxyz/RIME-LMDG)，多份词库的文件头也保留了该链接。

| 本仓文件（均位于 `sources/pinyin/wanxiang/`） | 上游文件（均位于 `dicts/`） | 上游许可 |
| --- | --- | --- |
| `cuoyin.dict.yaml` | `cuoyin.dict.yaml` | CC BY 4.0 |
| `diming.dict.yaml` | `diming.dict.yaml` | CC BY 4.0 |
| `duoyin.dict.yaml` | `duoyin.dict.yaml` | CC BY 4.0 |
| `fangyan.dict.yaml` | `fangyan.dict.yaml` | CC BY 4.0 |
| `huaxue.dict.yaml` | `huaxue.dict.yaml` | CC BY 4.0 |
| `jichu.dict.yaml` | `jichu.dict.yaml` | CC BY 4.0 |
| `mingren.dict.yaml` | `mingren.dict.yaml` | CC BY 4.0 |
| `mixed.dict.yaml` | `mixed.dict.yaml` | CC BY 4.0 |
| `shici.dict.yaml` | `shici.dict.yaml` | CC BY 4.0 |
| `taifeng.dict.yaml` | `taifeng.dict.yaml` | CC BY 4.0 |
| `wuzhong.dict.yaml` | `wuzhong.dict.yaml` | CC BY 4.0 |
| `yaopin.dict.yaml` | `yaopin.dict.yaml` | CC BY 4.0 |
| `yiren.dict.yaml` | `yiren.dict.yaml` | CC BY 4.0 |
| `yixue.dict.yaml` | `yixue.dict.yaml` | CC BY 4.0 |
| `zi.dict.yaml` | `zi.dict.yaml` | CC BY 4.0 |

许可依据是该提交仓库根目录的 [LICENSE](https://github.com/amzxyz/rime-wanxiang/blob/aa9947f3f513ff88a3f7ce4e342fcb519d517025/LICENSE) 与 README 的 CC BY 4.0 声明。许可名称为 Creative Commons Attribution 4.0 International（知识共享署名 4.0 国际），[许可链接](https://creativecommons.org/licenses/by/4.0/)，全文亦收在本仓 [sources/symbols/LICENSE](sources/symbols/LICENSE)。

本次改动仅将上述文件从上游 `dicts/` 复制到本仓 `sources/pinyin/wanxiang/`，保留原文件名及全部字节，未修改文件头、词条、读音、权重、行序、编码或换行。再分发原文件或改编数据时，须保留作者 amzxyz 与万象词库的署名、来源链接、许可链接以及上游提供的版权与免责声明；改编时须注明改动并保留已有改动说明。上游按许可中的免责声明以原样提供数据。

### 拼音权重与万象对齐

`sources/pinyin/single-chars.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt` 中与万象共有的词条，权重改编自 [amzxyz/rime-wanxiang](https://github.com/amzxyz/rime-wanxiang) 标签 `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）`dicts/` 下的词库。上游仓库以 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) 发布（仓库根目录 `LICENSE`，全文也收在本仓 `sources/symbols/LICENSE`），词库文件头注明数据出自同一作者的 [amzxyz/RIME-LMDG](https://github.com/amzxyz/RIME-LMDG)。署名：万象拼音（rime-wanxiang），作者 amzxyz。CC BY 4.0 可以单向并入 GPL-3.0 作品，所以这三份文件仍按 rime-ice 的 GPL-3.0 分发，同时保留这里的署名与改动说明。

改动：只改权重列，词条、读音、行序、注释与换行都保持原样，没有从万象加入新词条。

- 对照范围是万象 `wanxiang.dict.yaml` 导入的词库：`zi`、`jichu`、`lianxiang`、`duoyin`、`shici`、`diming`、`yixue`、`huaxue`、`yaopin`、`mingren`、`yiren`、`wuzhong`、`renming`、`taifeng`、`fangyan`。错音错字表 `cuoyin` 收的是错误读音，不参与。
- 万象的带调拼音先去掉声调（NFD 分解后删去组合附加符号），ü 写作 v，音节之间用 `'` 连接，与本仓的全拼写法一致。去调后同一个「词＋全拼」在万象里出现多次（不同声调，或出现在多份词库里）时，取最高的权重，这也是万象候选去重后实际显示的排序。
- 本仓一行的「词＋全拼」在万象里有对应时，权重改为 `round(万象权重 × 系数)`，四舍五入取整；万象没有的行保持原权重。
- 两边的权重量级不同，所以每份文件各用一个系数，让改写后的权重与该文件未改动的行处在同一量级。系数 = 该文件与万象共有、且两边权重都大于 10 的行上，本仓权重之和 ÷ 万象权重之和，保留两位小数。本次 `single-chars.txt` 为 9.42（共有 45175 行），`rime-ice.txt` 为 9.2（共有 521669 行），`rime-ice-supplement.txt` 为 9.28（共有 30499 行）。三份文件的系数相差不到 3%，所以合并后共有词条之间的相对次序基本与万象一致；同一单字读音同时出现在 `single-chars.txt` 与 `rime-ice-supplement.txt` 时，构建取较高的一行。
- `sources/pinyin/places.txt` 是 msime 生成器产出的地名下限表，不手工改，这次没有动；构建时它按只升不降并入，只会把低于下限的地名调高。`custom/words.txt` 只追加，也没有动。

`sources/pinyin/places.txt`、`sources/wubi/wubi86-supplement.txt` 的文件头记录了生成时对照的 `rime-ice.txt`、`rime-ice-supplement.txt` 的 SHA-256。那是生成当时的输入，作为历史陈述保留，与现在的文件不同；`upstream.lock.json` 已按改写后的字节刷新。

## 英文词库

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `sources/english/rime-ice-en.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice) | GPL-3.0 |
| `sources/english/rime-ice-en-supplement.txt` | [iDvel/rime-ice](https://github.com/iDvel/rime-ice)，提交 `3aea6d3694fb3d94ec663641f021f788822897ad` 的 `en_dicts/en.dict.yaml` 与 `en_ext.dict.yaml` 中相对 `rime-ice-en.txt` 新增的纯 ASCII 单词 | GPL-3.0 |
| `sources/english/scowl-words.txt` | [en-wl/wordlist](https://github.com/en-wl/wordlist)（SCOWL）发布 `rel-2026.02.25`（提交 `7e99edab8e32f9f9ea2b15f249ca8d4d67237410`）的 Aspell 英文词典 `aspell6-en-2026.02.25-0.tar.bz2`（SCOWL 60 级官方拼写检查词典）中 `en-common.cwl`、`en_US-wo_accents-only.cwl`、`en_GB-ise-wo_accents-only.cwl` 的纯字母词形（至少 3 个字母），排除 `rime-ice-en.txt`、`rime-ice-en-supplement.txt`、`custom/english.txt` 已有或被 rime-ice 注释掉的词、蔑称、只有大写形式且拼出中文词全拼的专名，以及没有 Google 词频的词；由 msime `crates/dict-builder` 的 `english-supplement` 子命令生成 | SCOWL 许可：可自由使用、复制、修改、分发和出售，条件是在所有副本中保留版权声明，并在随附文档中同时保留版权声明与许可声明。文件头照录了版权声明与许可声明；`dict-v` 发布把许可全文作为 `msime-scowl_Copyright.txt` 与 `msime-english.db` 一起附上；词典包 `Copyright` 全文收在 msime 的 `resources/licenses/scowl-aspell6-en-Copyright.txt`。60 级官方词典不涉及 UKACD 条款，也不用澳大利亚英语数据 |
| `sources/english/google-word-counts.txt` | [Google 1/3 million 词频表](https://www.norvig.com/ngrams/count_1w.txt) | 以来源页面说明为准 |
| `sources/unlicensed/oaldpe-words.txt` | 自 oaldpe.mdx 提取的词形列表 | 权利归词典出版方 |

`sources/pinyin/rime-ice-supplement.txt` 第 1 行文件头里的 `BaseDictIceV1`、`sources/english/rime-ice-en-supplement.txt` 第 3 行文件头里的 `BaseDictIceEn.txt` 是生成时对照文件的旧名，分别指现在的 `sources/pinyin/rime-ice.txt`、`sources/english/rime-ice-en.txt`。这两个文件按字节锁定，msime 里也没有它们的生成器，文件头作为历史陈述保留不改；下次补上生成器、在原路径重新生成时再改成新名。

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

发布构建的拼音词库 `msime-pinyin.db` 由 `sources/pinyin/single-chars.txt`、`sources/pinyin/rime-ice.txt`、`sources/pinyin/rime-ice-supplement.txt`、`sources/pinyin/places.txt` 与 `custom/words.txt` 构建，另含 msime 仓库自带的 `resources/dictionary-sources/mix/quick_phrases.txt` 快捷短语；`sources/unlicensed/custom-pinyin-dictionary-part1.txt`、`sources/unlicensed/custom-pinyin-dictionary-part2.txt`（含 CustomPinyinDictionary 的内容）不进入发布构建，见下方「发布构建不包含这些条目」。只有用 `--include-unlicensed` 做的本地完整构建才会读入这两份。中文数据主体来自 rime-ice（GPL-3.0），使用该数据库的前端本身以 GPL-3.0 分发，与 rime-ice 兼容，但**必须保留对 rime-ice 的署名**；与万象共有词条的权重来自 rime-wanxiang（CC BY 4.0），同样**必须保留对万象的署名并注明改动**（见上方「拼音权重与万象对齐」）。

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

`custom/` 下的自定义词条、候选窗翻译覆盖、英文词条与人名，`packs/` 下的专业词库，以及 `scripts/validate_packs.py` 由本项目编写，依据 GPL-3.0 提供，与组织内其他仓库一致。

`sources/cn-en/machine-translations.txt` 是本项目整理的中译英释义，同样依据 GPL-3.0 提供。源词取自 `sources/pinyin/`、`sources/wubi/` 与 `custom/words.txt`（来源与许可见上文）；译文汇总了 `custom/translations.txt` 与人工整理的翻译，其余由大模型生成：DeepSeek（`deepseek-chat`、`deepseek-flash`、`deepseek-v4-pro`，按 DeepSeek 开放平台用户协议，输出内容的权利归使用者）与 ChatGPT（按 OpenAI 使用条款，输出归使用者），纯数字与年份按规则生成。生成的译文没有逐条人工审校，可能有误译；修正写到 `custom/translations.txt`，它在构建时覆盖本文件。

## OpenCC 简繁转换表

`sources/opencc/` 从 msime 的 `crates/client-base/data/opencc/` 原字节迁入，固定 [BYVoid/OpenCC](https://github.com/BYVoid/OpenCC) 提交 `26753884f1984add422f3b0249ccee8613deaff6`，许可为 Apache-2.0，全文保留在同目录 `LICENSE`。`CJK_Compatibility_Ideographs.txt`、`STCharacters.txt`、`STPhrases.txt`、`TSCharacters.txt`、`TSPhrases.txt`、`TWPhrases.txt`、`TWVariants.txt`、`TWVariantsPhrases.txt` 是该提交 `data/dictionary/` 下的原样文本，其中 `TSCharacters.txt` 与 `TSPhrases.txt` 是后来为粤拼的繁转简输出从同一提交补入的，没有改写；`STPhrases_GeneratedFromRegionalPhrases.txt` 是同一提交的构建产物，由 `data/scripts/generate_st_phrases_from_regional_phrases.py` 使用 `t2s.json` 生成，迁移时不重新生成、不排序或改写。

这些表由 msime 构建时校验后内嵌，供 `s2t`、`s2tw`、`s2twp` 转换、粤拼输出简体时的繁转简（`TSPhrases` 与 `TSCharacters`，对应上游 `t2s.json` 去掉 `TSCharactersExt` 的部分）以及开发工具使用，不新增 `dict-v` Release 附件。更新时同步 msime 的 `resources/opencc.lock.json` 和许可证说明；保持上游文件名与逐字节锁定。

五笔补充数据在发布构建时从本次固定原表、拼音和人工词条重新派生；本仓两份补充表为审计快照，来源和许可与上表相同。每周 rime-ice 更新预览生成审核候选，不改变本表记录的已固定源提交；候选通过审核并更新源数据时再同步来源、许可证和锁。

## msime 构建时另加的输入

下面几项不在 msime-dictionary 的 `NOTICE.md` 里：它们由 msime 仓库（`resources/dictionary-sources/`）或 msime 的 `resources/dictionary-sources.lock.json` 提供，构建器 `crates/dict-builder` 在同一次 `dict-v*` 构建里读入。

| 文件 | 上游 | 上游许可 |
| --- | --- | --- |
| `resources/dictionary-sources/kaomoji/kaomoji.txt`（`msime-others.db` 的颜文字） | [aoguai/rime_kaomoji_dict](https://github.com/aoguai/rime_kaomoji_dict) | MIT |
| `ecdict/ecdict.csv`（`msime-english.db` 的中英释义，由 `resources/dictionary-sources.lock.json` 固定） | [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT)，提交 `bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b` | MIT |
| `resources/dictionary-sources/pinyin-overrides.txt`（表情、颜文字与符号关键词的整词读音） | [mozillazg/python-pinyin](https://github.com/mozillazg/python-pinyin) 0.55 的 `lazy_pinyin` 输出 | MIT |

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
