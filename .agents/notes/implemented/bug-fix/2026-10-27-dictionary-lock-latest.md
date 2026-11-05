# Agent Note: 词库发布锁跟上设备实际在用的词库

Status: implemented

## Problem

`resources/desktop-dictionary.lock.json` 从 2026-10-14 起一直锁 `dict-v2.0.24`（[上一次换锁](2026-10-14-dictionary-v2-0-24-pin.md)）。2026-10-18 六个平台都接入了[词库独立更新](../feature/2026-10-18-dictionary-auto-update.md)，默认每天检查一次，补词不再需要客户端发版，换锁看起来就不必要了，于是没人再动它。到 2026-10-27，`msime-dictionary` 已经发到 `dict-v2.0.39`，锁落后 15 个发布。

锁并没有因为自动更新而失去作用，它还决定两件事：随包基线（新装、自动更新关闭或下载失败、已装代次损坏时用的词库），以及 `verify-local.sh` 与依赖真实词库的评测、测试量的是哪一份词库。后者落后之后，所有评测基线量的都是用户已经不在用的词库。九键音节频度那次回归就是这样漏掉的：同一份代码配随包的 `dict-v2.0.24` 是对的，iPhone 自动更新到 `dict-v2.0.32` 之后「我们今天去哪里」打不出来，只在真机验收时才发现（[九键音节频度取前三个字](2026-10-20-nine-key-syllable-prior-top-three.md)）。

## Decision

客户端锁改到 `dict-v2.0.39`，即换锁时最新的发布，评测基线在新锁上重跑并提交。做法与上一次相同：

- 主锁里十一个来自 `msime-dictionary` 的条目改 URL，SHA-256 和大小取自 `gh release view dict-v2.0.39 --json assets` 的资产摘要，不下载二进制自己算；再与该发布的 `msime-dictionary-manifest.json`、`msime-SHA256SUMS.txt`、`msime-language-dictionaries-SHA256SUMS` 逐项核对，全部一致。`sentence-model.safetensors` 来自 `chinese-ime-lm` 的 `model-v1`，不变。
- `source_commit` 跟随清单的 `source.commit`（`8bb181be2ed36ed9559a87434cc4c9048077ff29`，`dirty: false`，是 develop 上的提交）。清单的 `format_version`、`engine_compatibility.dictionary_format` 仍是 1，日文模型 magic 仍是 `MSJPDT1`。
- 组件锁和版本锁用 `python3 scripts/editions.py gen-locks` 从主锁生成，`resources/language-dictionaries.lock.json` 同步前进（`msime-stroke.db` 自 `dict-v2.0.30` 起从 2.3 MB 变为 3.8 MB，其余语言词库字节不变）。
- full 资源锁的代次从 `7356c4a4…` 变为 `576494c48ac513d15b9920111a73cf149fb79aedcb8cc6947d0799cf36006553`，`crates/client-core/src/edition.rs` 里钉住代次的测试一起更新。已装的用户词库按代次分目录，升级后照常从回放日志重建。
- `resources/eval` 下 `verify-local.sh` 覆盖的十一份基线在新锁上以 `--update-baseline` 重新生成。`baseline-web-articles.json` 的输入不在本仓库（TapTapGo 的文章分句），`voice-baseline.json` 与词库无关，两者不动。
- iOS `HelpcodeSettingsTests` 对自然码 `shi` 加 Shift `Y` 检查讠旁候选仍在且「是」被过滤，不固定「识」「试」「诗」的首选顺序；删除导入码表后，与同一会话导入前保存的完整候选数组比较。新词库把首选从「识」改为「诗」，不改变设置重载和码表回退的验收目标。导入后首选必须为合成码表指定的「是」、关闭辅助码后 Shift 字母成为大写字面片段，这两项精确断言保留。

### 两个版本之间换了什么

资产列表没有增减或改名。字节变化的是 `msime-pinyin.db`（73.7→73.9 MB）、`msime-wubi.db`、`msime-english.db`（大小不变）、两张 n-gram 表（大小不变）、清单和 `msime-stroke.db`；日文词典、`msime-others.db`、许可证文本不变。

`msime-wubi.db` 从 14,536,704 字节涨到 94,982,144 字节，86 与 98 两张表各从约 13 万行涨到约 73 万行。这是[五笔多字词覆盖](../feature/2026-10-18-wubi-vocabulary-coverage.md)与[构建时派生五笔词条](../feature/2026-10-18-dictionary-completion.md)的结果，从 `dict-v2.0.29` 起出现。全量下载随之从约 189 MB 变为约 270 MB（锁里各文件大小之和 189,155,520 → 269,793,614 字节），macOS 五笔版的 `EngineResources` 从约 120,400 KiB 变为约 199,200 KiB（按锁里大小逐个向上取整到 4 KiB 再加 `helpcodes/` 估算，没有实际打包）。

### 评测结果

同一个 release 二进制、同一份评测集，挂 `sentence-model.safetensors`，先在 `dict-v2.0.24`、再在 `dict-v2.0.39` 上跑，所以后两列的差别只来自词库数据。第一列是 develop 上提交的基线：`sentences`、`harvested`、`neutral`、`words`、`nine-key` 五份在 `dict-v2.0.24` 上也已经对不上（基线停在 10-13 到 10-22 的代码），本次一起覆盖。每格是 top-1 / top-5 / found，最后一列是两音节词的 top-1。

| 集合 | 条数 | 原基线 | dict-v2.0.24 | dict-v2.0.39 | 两音节 top-1（v2.0.24→v2.0.39） |
|---|---|---|---|---|---|
| `sentences` | 60 | 0.800 / 0.850 / 0.850 | 0.817 / 0.833 / 0.850 | 0.817 / 0.833 / 0.833 | |
| `harvested` | 312 | 0.324 / 0.817 / 0.939 | 0.503 / 0.827 / 0.939 | 0.494 / 0.772 / 0.885 | |
| `neutral` | 1105 | 0.641 / 0.884 / 0.928 | 0.708 / 0.876 / 0.928 | 0.648 / 0.825 / 0.878 | |
| `words` | 2791 | 0.781 / 0.960 / 0.989 | 0.781 / 0.957 / 0.989 | 0.750 / 0.943 / 0.988 | 0.727 → 0.683 |
| `nine-key` | 2791 | 0.571 / 0.847 / 0.963 | 0.571 / 0.847 / 0.963 | 0.518 / 0.815 / 0.954 | 0.423 → 0.366 |
| `nine-key-sentences` | 81 | 0.642 / 0.667 / 0.728 | 同左 | 0.519 / 0.605 / 0.704 | |
| `fourteen-key` | 2791 | 0.658 / 0.903 / 0.973 | 同左 | 0.619 / 0.888 / 0.972 | 0.535 → 0.487 |
| `fourteen-key-sentences` | 81 | 0.667 / 0.691 / 0.802 | 同左 | 0.580 / 0.654 / 0.728 | |
| `particles` | 103 | 0.680 / 0.971 / 1.000 | 同左 | 0.670 / 0.951 / 1.000 | |
| `particles-nine-key` | 103 | 0.194 / 0.728 / 0.951 | 同左 | 0.233 / 0.767 / 0.990 | |
| `particles-fourteen-key` | 103 | 0.311 / 0.854 / 1.000 | 同左 | 0.359 / 0.883 / 1.000 | |

除三份句末语气词集合外全部下降，九键、14 键最多。原因在权重：`dict-v2.0.30` 合入了 [msime-dictionary#84](https://github.com/metasequoiaime/msime-dictionary/pull/84)，把源数据里与万象（rime-wanxiang v18.0.16）共有的单字和词语的权重改成「万象词频 × 按文件算的系数」，词条和读音不变。这是词库仓有意的改动，发布说明里写明了，不是构建出错；该 PR 自己也指出万象的单字频率和 rime-ice 差别很大（女 nv 3,362,646→34,421），并选择照搬不手调。多字词：摆脱 199375→80850、拜托 53420→171736，本着 9999→57712、奔着 5555→72395，财力 125580→23681、彩礼 6515→50931；散步 101880→64915、跑步 66055→335220，两者在九键上同码，「我们一起去公园散步吧」于是打成「跑步吧」；来过 66575→31694、开过 34000→56792，同样九键同码，「我以前来过一次」打成「开过一次」。单字变化更大，而且方向不一：jing 下惊 1,161,953→9,580、静 968,467→11,690、睛 724,850→7,159、敬 255,846→8,054，惊和静在新词库里排在净（14,695）后面，连 jing 的前八名都进不去；休 334,462→30,794；同时就 11,305,679→18,266,953、杀 109,588→488,163，多字词杀人 194,905→3,119、均属 116,940→1,205、部分 501,714→2,806,239。`found` 也随之下降（harvested 0.939→0.885，neutral 0.928→0.878）：评测从不分页的全部候选里找金标准，但整句候选只有有限几条读法，权重一变，原来排第二、第三的整句就掉出了列表——「说破英雄惊杀人」（旧词库 rank 2，新词库首选「说破英雄经啥人」）、「如何肯休」（rank 3）、「部分衣物均属此类」（rank 3）在新词库上都不在候选里。词条本身都还在（上面的权重就是从新库里查出来的），丢的是排序。

这些不是代码回归：设备经词库自动更新早已在用这批数据，基线现在如实记下了用户手上的排序质量。但「改动是有意的」不等于「结果是对的」：惊、静这类常用字掉出前八，看起来是万象单字词频本身的口径（按读音拆分、语料来源）不适合直接当本引擎的单字权重，这一点没有核实，需要在词库仓跟进。改善要在词库仓（权重来源）或引擎（对权重的敏感度）上做，不在本次范围内。

### 换到 dict-v2.0.41

2026-10-29 锁从 `dict-v2.0.39` 换到 `dict-v2.0.41`，为的是把 #10270 改正的笔画、注音库许可证（库内 `license` 元数据与随库的两份许可证文本，见[语言词库许可证表达式笔记](../process/2026-10-29-language-dictionary-licence-expressions-and-mobile-notice.md)）送进安装包。那天推送 `custom/` 词条自动发布的 `dict-v2.0.40` 用的构建器（`5a15105f9`）还不含 #10270，语言词库与 `dict-v2.0.39` 逐字节相同，所以在 msime-dictionary 的 `main` 上手动运行 `release-built-dictionaries.yml`（`publish` 打开，`msime_ref` 固定为提交 `09a69f20b4` 而不是会移动的 `develop`）发布了 `dict-v2.0.41`，源数据提交 `8e8b781` 与 `dict-v2.0.40` 相同。

与 `dict-v2.0.39` 相比，源数据只多了 `custom/words.txt` 的 12 个词和 `custom/english.txt` 的 1 个词；构建器多了主库、五笔库与 others 库的 `metadata.format_version`（[文件内版本笔记](../architecture/2026-10-29-main-dictionary-in-file-version.md)）和上面的许可证改动。字节变化的是拼音、五笔、英文、others 四个库、两张 n-gram 表、清单、笔画与注音库和它们的许可证文本；日文词典、两份 Mozc 文本、SCOWL 声明、粤拼库及其许可证不变。做法与上面相同，代次变为 `641a853a12de31e99579e3370742c08b182033f35a19cf62e0722df81367975d`，`msime-engine-dictionary-NOTICE.md` 按 `8e8b781` 的 `NOTICE.md` 重新同步。九键音节频度表按新的 `msime-pinyin.db` 重新生成，29 个音节的总权重各多 100 到 3733。

评测用 `verify-local.sh` 的十三组参数（不带 `--first-page-rows`，即引擎默认的首页 6 行；挂 `sentence-model.safetensors`），另加打字流 `typing_sim --keyboard nine` 和双拼触点词级（`--jobs 5 --assert-top1-unchanged`），分三臂：develop 二进制配旧锁资源，同一个二进制配新锁资源（`--resource-lock` 指向新锁），本次的二进制（新的音节频度表）配新锁资源。十三份报告三臂逐字节相同，也与提交的基线逐字节相同；双拼触点词级同样相同；打字流只有记录资源来源的 `generation` 与 `source_commit` 两个字段变了，基线随之更新。新增的 12 个词和音节频度表的微调没有碰到任何评测用例。

### 换到 dict-v2.1.0

2026-11-05 锁从 `dict-v2.0.41` 换到 `dict-v2.1.0`，即词库大清洗第一轮的发布（[清洗方案](../feature/2026-11-04-dictionary-curation-tables.md)第 5 节 4.4 行）：msime-dictionary `a6b5ad6`（#128，`curation/` 否决表、覆盖表、粗俗词表、英文与释义否决、粤拼两张表，五笔 86 拆成极点表与 rime-wubi 两个文件，万象 `7ab73ee` 全套登记为上游，地名表加万象下限后重新生成）用构建器 `2f5de01dc`（#10858）构建。中间的 `dict-v2.0.42` 到 `dict-v2.0.56` 没有单独换锁，开着自动更新的设备早已用上它们，随包基线和九键音节频度表一直停在 `dict-v2.0.41`。

做法与前两次相同，现在全部可以照着重跑：主锁和语言词库锁用 `scripts/resource_lock_from_sums.py --url-base https://github.com/metasequoiaime/msime-dictionary/releases/download/<tag>/` 从下载的发布附件生成（它先逐个核对文件与 `msime-SHA256SUMS.txt`，语言词库把 `msime-language-dictionaries-SHA256SUMS` 改名放进只含那六个文件的目录，`--source-commit` 给构建器提交），再与 `gh release view --json assets` 的摘要和清单的 `files` 逐项核对；`editions.py gen-locks` 生成组件锁与版本锁，full 的代次变为 `1ccceecb4f95d74bcf12768079bd5195d8bfb3f71840c3d9c8d4b3a3de46f12a`。语言词库锁的 `source_commit` 记构建器提交（与主锁和 `dict-v2.0.41` 那次一致；#10691 记成了源数据提交）。`msime-engine-dictionary-NOTICE.md` 按 `a6b5ad6` 的 `NOTICE.md` 重新同步，此前「已登记、尚未随当前词库发布的来源」里的四项都已写进那份文件，这一节删掉，只把那份文件没写的构建器内置英文蔑称挪进「msime 构建时另加的输入」。万象署名补上地名表的万象下限（719 行）。

九键音节频度表按新的 `msime-pinyin.db` 重新生成：424 个音节里 368 个的总权重变了，最多 -4.3%（weng），排名的秩相关 0.99999，没有音节移动超过 3 位；同一份新词库配新旧两张表，九键、14 键词级和九键长句的 dump 配对比较 fixed / broken 都是 0。引擎单测 `shipped_syllable_frequency_covers_every_spelling_and_names_its_source` 改为要求表的来源行等于主锁里 `msime-pinyin.db` 的 sha256，换锁不重新生成表就失败；dict-builder 的 `shipped_table_is_rebuilt_from_the_locked_dictionary` 在设了 `MSIME_EVAL_RESOURCES` 时从那份词库重新生成并逐字节比较，防手改。

字节变化：除日文词典、两份 Mozc 文本和 SCOWL 声明外全部变了。`msime-wubi.db` 从 95.0 MB 涨到 142.5 MB（多了新世纪表 `wubixsj`，726,674 行），全量下载从 269,814,094 字节变为 317,262,783 字节，面向用户的「首次约 270 MB」改为约 320 MB；`msime-stroke.db` 从 3.8 MB 降到 2.4 MB（扩展区门槛）。拼音库第一次带 `sentence_excluded`（67 个词、19 条豁免写法，共 86 行）。Android 的 full 与拼音瘦包不带五笔码表，不受影响；五笔版 APK、Play 版 AAB、macOS 五笔版和 web-engine 的五笔 86 包都会变大，本次没有实测。

评测分三臂：提交的基线、同一个二进制配 `dict-v2.0.41`、同一个二进制配 `dict-v2.1.0`。第二臂已经与提交的基线对不上（十三份、打字流、双拼触点词级都是），基线停在 10-29，之后的引擎改动没有重录，所以基线这次一并记下那段代码漂移（逐例见下文「代码漂移」一段）；词库本身的影响看第二、三臂的配对比较（`eval_compare.py convert`，每格 fixed / broken，top-1）：

| 集合 | v2.0.41 | v2.1.0 | top-1 fixed / broken |
|---|---|---|---|
| `sentences` | 0.800 / 0.817 / 0.817 | 同左 | 0 / 0 |
| `harvested` | 0.494 / 0.779 / 0.875 | 0.494 / 0.776 / 0.872 | 0 / 0（top-5 坏 1：通过大改文风，文峰 按万象下限从 100 升到 1242） |
| `neutral` | 0.651 / 0.826 / 0.881 | 0.648 / 0.824 / 0.878 | 1 / 4 |
| `words` | 0.746 / 0.944 / 0.988 | 0.749 / 0.946 / 0.988 | 9 / 2 |
| `nine-key` | 0.550 / 0.834 / 0.968 | 0.551 / 0.836 / 0.968 | 4 / 2 |
| `fourteen-key` | 0.631 / 0.894 / 0.975 | 0.633 / 0.893 / 0.975 | 6 / 1 |
| 其余七份 | | | 0 / 0（两份前缀集只有 top-5 有零星变化） |

打字流 `typing_sim --keyboard nine`：test 段 cfa 0.527 → 0.530，dev 不变，整块首选修好 1、坏 0。双拼触点词级 125 组参数的 top-1 变化在 -0.001 到 +0.002 之间。`dict_eval_gate.py`（B = v2.0.41，C = v2.1.0）五道硬门槛全部通过：金标准对一个不少，must-outrank 6 行从 1 行通过到 6 行全过，全拼粗俗首屏输入 6 → 2，quanpin-words-v2 全拼 top-1 修好 48、坏 5（p = 7e-10），九键 14 / 7，14 键 21 / 8，全拼 953,156 行。

坏掉的用例分三类。第一类是地名重新定权（方案 3.3），按设计：地名表的阈值随 #84 的换算从 10 改成 92，rime-ice 里已经有权重的地名不再被抬到旧的 31460，回到与万象对齐后的权重（昌吉 2999、铜川 1297、株洲 3119、黄南 2515，排到 唱机、同传、煮粥 之后；辽源、甘孜 落在地级简称下限 5115），万象下限又把万象词频更高的县名抬到同码常用词前面（金州 11261 > 锦州 10700，福安 > 伏案，德城、惠东、宝兴、定远、安吉 在九键或 14 键上压过 得逞、挥动、暴行、定员、安居）。第二类是覆盖表压低的词伤到整句：下不（26320 → 1563）、头的（145565 → 3927）、打私（19999 → 8077，比 大寺 8078 低 1）在碎片一节被压低，是黄（25480 → 254）在本地损坏一节恢复成上游 rime-ice 的值；词网格原本靠这几条拼出 我们再也下不去了、拉人头的、主要是黄复兴党部、巩固打私成果，现在 h-0488、h-1084、h-0415 三句从第 1 掉出前 5，首选分别成了 我们在腋下不去了、拉人投的、主要石磺复兴党部，h-0097 巩固打私成果 从第 3 掉出前 5（首选仍是 巩固大四成果，第 3 位换成 肱骨大四成果；neutral 的 top-5 坏 4 例就是这四句）。下不 是 下不去、下不来 的正常切分单位，打私（缉私打私）是真词，两条都是碎片规则的误伤，覆盖表文件头列出的接受代价里也没有 打私 这一句；这两条在 `dict-v2.1.2`（msime-dictionary #130）修掉：打私 记进碎片一节的误伤、那一行撤掉，下不 改成 下部 的权重减 1（5777），见下一节「换到 dict-v2.1.2」。第三类是逐键模型把首选换给了紧挨着的同长读法：九键 制动（25217）、火把（16670）不挂 `sentence-model.safetensors` 时在新词库上仍排第一（火把、过吧、锅巴、过把、活靶；制动、只懂、之东、止动），挂上之后成了 过吧、火把 和 只懂、制动。旧词库上排第二的是碎片 过把（11480）、之东（15660），模型没有拿它们换掉首选；覆盖表把它们压到 7479、10745 之后，第二位换成 过吧（8952）、只懂（10746），模型更偏向这两个，按 a709092925（#10734，逐键模型只在和首选同样多字的读法之间换）的规则把它们换到第一。词库没有把 火把、制动 压到后面，引擎的九键排序也没变，这是清洗暴露出来的模型偏好：任何一次把碎片从强首选正下方压走、让另一个同长读法补上来的降权都可能再触发，记作模型侧的问题。neutral 第四句 h-0823 重回职场花了很多力气 退到第 2（首选 重回职场化了很多力气）也只在挂模型时出现：不挂模型时新词库上它仍排第一，这句切分里 重回、职场、花了、化了、很多、力气 的权重都没变，把两份 n-gram 表换回旧的也照样退，触发它的是哪一处词库变化没有查清，同样记在模型侧。后两类记作待查的退步，基线照实记下，没有改评测用例。

代码漂移：develop 上换到 `dict-v2.0.41` 那次合并（`b94ece1cf7`）的二进制配 `dict-v2.0.41`，十三份报告与提交的基线逐字节相同；它和本次的二进制在同一份 `dict-v2.0.41` 上逐例配对（top-1 fixed / broken）：`words` 0 / 11（p = 0.001），`nine-key` 51 / 59，`fourteen-key` 47 / 45，`neutral` 9 / 6，`harvested` 3 / 3，`sentences` 0 / 1，`nine-key-sentences` 2 / 2，`fourteen-key-sentences` 1 / 0，`fourteen-key-prefix` 1 / 0，`particles` 9 / 0，`particles-nine-key` 2 / 0，`particles-fourteen-key` 3 / 0，`nine-key-prefix` 0 / 0。这些全部是 10-29 之后合入的引擎改动带来的，与词库无关，但同样进了这次的基线，所以把坏掉的几处逐例记下（first-parent 二分，每一步重编 `convert_eval` 在 `dict-v2.0.41` 上看这几例的名次）：

- `words` 的 11 例都从第 1 退到第 4：电子计算机（dianzijisuan）、动力装置（donglizhuang）、飞黄腾达（feihuangteng）、观念更新（guanniangeng）、坚守岗位（jianshougang）、两袖清风（liangxiuqing）、情况汇报（qingkuanghui）、双管齐下（shuangguanqi）、宣传报道（xuanchuanbao）、邮政编码（youzhengbian）、装机容量（zhuangjirong）。补全出的整词让给了三个与输入同长的读法（如 电子计算机 让给 电子计算、点子计算、点字计算），起于 a709092925（#10734）。这 11 例都是 v1 的 12 字符截断杂质（`resources/eval/README.md` 记的第一类）：输入本身是一个更短的词的完整读音，新的首选对这串输入不算错，只是按 v1 的金标准记作坏，换到 `quanpin-words-v2.tsv` 量时它们不在集合里。
- `nine-key-sentences` 的 nk-051 周末我们去看电影吧 退到第 5（首选 周末我们去看电子你把）、nk-055 我们一起去公园散步吧 退到第 4（首选 我们一起去公园萨那他啊），同样起于 a709092925；同一段里 nk-059、nk-070 修好，top-1 持平。这两句是真的退步。
- `sentences` 的 s-048 这个项目的进度非常顺利（zheigexiangmudejindufeichangshunli）成了 这各项目的进度非常顺利，金标准不在候选里，起于 ef2c73e7d0（#10745，单独的轻声助词也拿一份成词加分）。这一句也是真的退步。

后两项和第三类的 制动、火把 都没有在本次修，记作引擎与模型侧待查的退步。`baseline-shuangpin-touch-sentences.json`（双拼触点整句，完整一组要几个小时）这次没有重录，它最后一次重录是 `c0e22eb15e`（10-29，`dict-v2.0.39`），既落后于这次的词库，也落后于之后的引擎改动，拿它比较时会看到与自己的改动无关的差别，`resources/eval/README.md` 的双拼一节写明了这一点。

### 换到 dict-v2.1.2

同一天锁再从 `dict-v2.1.0` 换到 `dict-v2.1.2`：msime-dictionary `c239ba1`（#130）用构建器 `24852caa38`（#10886 的合并提交）构建。中间的 `dict-v2.1.1`（#129）只把构建器升到 `24852caa38`，与 `dict-v2.1.0` 相比只有清单和 `msime-SHA256SUMS.txt` 不同，数据逐字节相同，没有单独换锁。`dict-v2.1.2` 改的就是上一节记下的两处碎片误伤：`curation/pinyin-weight-overrides.txt` 里 下不 xia'bu 从 1563 改成 5777（下部 5778 减 1，xiabu 的首选仍是 下部；九键上 下不 紧随其后排第 2，全拼上排在 夏不 和补全出的 呷哺呷哺、瑕不掩瑜、下不来 之后的第 6 位），打私 da'si 8077 那一行撤掉，打私 回到 19999（dasi 上排在 大四、打死 之后、大肆、达斯、大寺 之前）。覆盖表从 2279 行减为 2278 行，碎片一节从 1474 行减为 1473 行，写成由改编后的万象权重派生的值的行从 1733 减为 1732（下不 新的值取自 下部，仍算在内）。上游 `NOTICE.md` 同时把「没有从万象抄权重」改成写明这 1732 行的派生关系，`msime-engine-dictionary-NOTICE.md` 按 `c239ba1` 重新同步后，msime 一侧那段指出原句不准确的说明就删掉了；万象署名、`docs/third-party.md` 的行数和当前发布一并跟上。

字节变化：只有 `msime-pinyin.db`（长度不变，仍是 73,891,840 字节）、清单和 `msime-SHA256SUMS.txt` 变了。`msime-pinyin.db` 的 187 个对象逐表双向比对，只有 下不、打私 两行的权重变了，行数、表结构和 metadata 都不变。`msime-wubi.db` 逐字节相同：#10886 改了 `place_offensive_last_in_wubi`（已经排得更低的粗俗五笔行不再被提权），在现在的数据上没有改动任何一行。语言词库的六个文件逐字节相同，语言词库锁只换了 URL 和 `source_commit`（仍记构建器提交）。全量下载大小不变，「首次约 320 MB」不用改。full 的代次变为 `905db0dd958a973b3d7abf67f28c75402162c1d9e16b0186952793ffa8c3d2b1`。九键音节频度表重新生成，只有 bu、xia 两个音节的总权重各多 4,214（5777 − 1563），da、si 各多 11,922（19999 − 8077），其余 420 个不变。

评测与上一次一样分三臂：提交的基线（在 `dict-v2.1.0` 上录）、同一个二进制配 `dict-v2.1.0`、同一个二进制配 `dict-v2.1.2`。第二臂的十三份报告、打字流和双拼触点词级都与提交的基线相同，这次没有代码漂移。第二、三臂逐例配对（`eval_compare.py convert`，并逐条比较整张候选表）：

| 集合 | v2.1.0 | v2.1.2 | top-1 fixed / broken | 候选表有变化的用例 |
|---|---|---|---|---|
| `neutral` | 0.648 / 0.824 / 0.878 | 0.649 / 0.826 / 0.880 | 1 / 0 | 2：h-0488 我们再也下不去了 回到第 1，h-0097 巩固打私成果 回到第 3（top-5 修好 2、坏 0） |
| `nine-key` | 0.551 / 0.836 | 同左 | 0 / 0 | 5，最靠前的变化在第 16 名 |
| `nine-key-prefix` | 0.193 / 0.343 | 同左 | 0 / 0 | 9，最靠前的变化在第 11 名 |
| `fourteen-key-prefix` | 0.328 / 0.553 | 同左 | 0 / 0 | 1，在第 45 名 |
| 其余九份 | | | 0 / 0 | 0，dump 逐字节相同 |

表里数字是 top-1 / top-5（neutral 加 found）。九键和两份前缀集的 15 例都是 打私 回到 19999 后，含 打私 的候选（打私严惩、碧桃打私 一类）在第 11 名以后插进来或往前挪，金标准的名次一例都没变。另外对 xiabu、dasi、下不为例、吓不倒 单独探了一次：全拼、九键 xiabu 的首选都是 下部，dasi 的首选仍是 大四，全拼上 打私 回到第 3（大四、打死 之后），九键 dasi 上 打私 从第 59 位升到第 35 位，其余几条名次不变。

打字流 `typing_sim --keyboard nine` 的两臂报告只差记下的资源代次和 `source_commit`，指标逐字节相同；双拼触点词级（`--max-syllables 2 --limit 3000`，125 组参数）两臂都与提交的基线相同。`must_outrank.py` 6 行全过，名次与 `dict-v2.1.0` 相同。`dict_eval_gate.py`（B = `dict-v2.0.41`，C = `dict-v2.1.2`）五道硬门槛全部通过，计数与 C = `dict-v2.1.0` 那次一模一样：金标准对 24,851 对一个不少，must-outrank 1 → 6 行，全拼粗俗首屏输入 6 → 2，quanpin-words-v2 全拼 top-1 修好 48、坏 5，九键 14 / 7，14 键 21 / 8，全拼 953,156 行；只有不进门槛的 neutral 参考从 1 / 4 变成 1 / 3（h-0488 不再坏）。

按 `--update-baseline` 在 `dict-v2.1.2` 上重录了十三份、打字流和双拼触点词级，实际变了的只有 `baseline-neutral.json`（上面两句）和 `baseline-typing-chat-nine-key.json`（资源代次和 `source_commit`），其余逐字节相同。上一节「第二类」里剩下的 h-1084 拉人头的、h-0415 主要是黄复兴党部，以及「第三类」的 制动、火把、h-0823 都还在，`dict-v2.1.2` 没有碰它们。

## Alternatives considered

- **继续锁 `dict-v2.0.24`，靠自动更新把新词库送到用户手里。** 不动锁最省事，随包基线也最保守：自动更新出问题时用户回到一份评测过的词库。但评测、`verify-local.sh` 和依赖真实词库的测试会继续量一份没人在用的词库，九键那次回归正是这样只在真机上才暴露；锁越久越远，下一次不得不换时要一次吞下的差异越大。所以不采用。
- **CI 与评测自动跟随最新发布，不再手工换锁。** 能保证测量对象永远等于设备上的词库，也省掉换锁这一步。但评测基线是提交进仓库的数字，跟随最新发布意味着任何一次词库发布都会让与之无关的 PR 的评测门禁对不上，而且对不上的原因不在那个 PR 里；随包基线和资源代次也会随构建时间变化，同一个提交两次构建出两种安装包，`edition.rs` 钉住的代次和锁派生关系都失去意义。可复现性比时效性更重要，所以仍然手工换锁，只是把换锁当作词库发布后的例行一步，而不是只在补词时才做。
- **锁 `dict-v2.0.28`，即五笔码表变大之前的最后一个发布。** 能避开下面说的安装包体积门槛，但设备上早已是 `dict-v2.0.3x` 的五笔码表和权重，评测仍然量不到用户手上的东西，问题没解决，所以不采用。

## Consequences

- **收益**：评测基线、`verify-local.sh` 和 `MSIME_EVAL_RESOURCES` 下的真实词库测试量的是设备经自动更新已经在用的词库；新装用户和自动更新不可用的用户拿到的随包词库与其他人一致。
- **代价**：基线数字整体下降（见上表），这是词库数据带来的，记录在案而不是被掩盖；下一个动排序的改动要以新基线为准，包括正在 `dict-v2.0.39` 上做九键实验的 `feat/nine-key-word-ranking` 等分支。
- **安装包体积增长需要分别验证**。macOS 已按本版本锁计算预算，Android 门槛与实际压缩体积仍待处理：
  - Android 的 full 与拼音瘦包（`platforms/android/build-apk.sh`，`MSIME_ANDROID_MAX_APK_BYTES` 默认 135,000,000）带着五笔码表。`android-v0.6.0` 的 full 瘦包已是 133,833,900 字节；`msime-wubi.db` 用 gzip -6 估算从约 7.4 MB 涨到约 44.9 MB，拼音库约多 0.4 MB，瘦包估计约 171.7 MB，超出门槛约 37 MB。2026-10-26 刚为守住这个门槛把读音表改成资源包（[英文音标候选](../feature/2026-10-26-android-candidate-pronunciation.md)），直接抬门槛与那次的取舍相反；另一条路是像 macOS 一样把五笔码表改成按需资源包（[macOS 五笔按需下载](../feature/2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)）。
  - macOS 五笔版随包带五笔码表。`package-release.sh` 的 `EngineResources` 预算现在按本版本锁中实际随包的条目逐文件向上取整到 4 KiB，加仓库随附辅助码表，再留固定 4 MiB 余量；完整性校验仍先执行，按需资源排除检查不变。新锁的五笔版预算为 198880 KiB，full 与拼音版为 110508 KiB，不再让五笔版卡在旧的 126000 KiB 固定值。资源增长由锁文件改动显式评审，额外未锁定文件不会抬高预算。轻量回归执行真实预算函数，覆盖五笔版与省略五笔的 full；尚未实际打完整 DMG。预算和被量的一侧现在都在 `platforms/macos/scripts/engine_resources_budget.py`，被量的一侧按文件长度取整而不是 `du` 的已分配块数，见 [macOS 包里的 EngineResources 按文件长度量](2026-11-02-macos-engine-resources-measured-by-length.md)。
  - Android 五笔版不受 135,000,000 字节门槛约束（门槛只量 full 与拼音的 GitHub 瘦包），但 `android-v0.6.0` 的 `msime-android-wubi.apk` 已是 161,007,391 字节，按同样的估算会到约 199 MB。Play 版 AAB 是 full 瘦包加约 27 MB 语音运行库，换锁后也在约 200 MB 上下；Play 对单个模块压缩后的下载大小有上限（约 200 MB），这个数和按设备拆分后的实际下载大小本次都没有核实，发版前要用 bundletool 实测。把五笔码表改成按需资源包能同时解决这三处。
  - web-engine 受影响但没有门槛：`scripts/build-web-engine.sh` 把 `wubi86` 整张表打成 `msime-wubi86.db.gz`（`crates/dict-builder/src/web.rs` 不裁五笔表），86 表从约 13 万行涨到约 73 万行，下一个 web-engine 版本给 TapTapGo 的下载会明显变大；体积门槛只管 wasm、拼音与日文。发 web-engine 前要跑一次 `build-web-engine.sh` 看实际体积，再决定是否给 `wubi86` 加门槛或裁剪。
- Linux 面向用户的「首次约 170 MB」在换锁前就已不准，现在全量下载约 270 MB，本次一并改为约 270 MB：`msime-linux-setup` 的 `--help` 与运行时提示、`platforms/linux/README.md`、Arch PKGBUILD、两份 Gentoo ebuild、设置页 `linux-setup-page.tsx`，以及 `linux_setup.rs` 里 `SETUP_TIMEOUT` 的注释（30 分钟对应 270 MB 只要求约 150 KB/s，超时不用改）。
- **重访信号**：词库每发一次版就该评估一次换锁；评测基线与设备上的词库再次相差多个发布时，说明换锁又被跳过了。

## Verification

- macOS 预算回归先在旧脚本上失败，再在锁驱动预算上通过；`python3 scripts/test-macos-package-resources.py`、`bash -n platforms/macos/package-release.sh`、`git diff --check` 通过。未运行完整打包或 quick，不能据此称发布包已验证。

- iOS 辅助码断言修正后，`xcrun swiftc -frontend -parse platforms/ios/KeyboardTests/settings/HelpcodeSettingsTests.swift`、笔记目录/格式/归档封印三项校验和 `git diff --check` 通过。这一轮未运行 Xcode 构建、iOS 会话测试或完整 quick 门禁，设置重载与导入回退的运行时结果仍需 CI 或设备复验；Android 的实际 APK 体积与预算缺口仍待验证。
- `python3 scripts/editions.py gen-locks --check`、`scripts/test-editions.py`、`scripts/test-macos-package-resources.py`、`scripts/test-fetch-language-dictionaries-bounds.py`、`scripts/test-language-data-notices.py`、`scripts/test-linux-setup-download-links.py`、`scripts/test-reference-feature-log.py` 等与锁相关的检查通过。
- 在 Mac Studio 上 `cargo run --release -q -p msime-client-core --example install_resources --locked -- target/resources` 下载并校验了新锁全部资源，目录名即新代次。
- 评测：`convert_eval` 按 `verify-local.sh` 的十一组参数，先以 `--resource-lock` 指向旧锁在 `dict-v2.0.24` 上跑，再在新锁资源上以 `--update-baseline` 跑，结果见上表。完整 25,119 条全拼词级在新锁上 top-1 0.756、top-5 0.945；完整九键词级因构建机过载没有跑完。
- 测试：`cargo test -p msime-engine -p msime-input-runtime -p msime-client-providers -p msime-host-api -p msime-client-core` 不设资源时，client-core（含钉住新代次的 `full_keeps_the_desktop_lock_and_its_generation`）、client-providers、engine、input-runtime 全部通过；host-api 失败的是下面那两项，另有 `a_lookup_keeps_the_users_japanese_mixing`（合成夹具，不读锁）失败一次，单独重跑通过。再以 `MSIME_EVAL_RESOURCES` 分别指向两份词库跑 engine、input-runtime、host-api：两边各 4150 项通过、8 项失败，失败的是同一组，没有一项由换锁引起——四个 golden 真实集和 `japanese::decoder::tests::real_model_loads` 按设计要 `dict-v2.0.1`（golden 记录自 C++ 参考引擎）；`session::tests::real_resources_mix_japanese_conversions` 在两份词库上都给不出 ohayou 的日文候选，而两版的 `msime-japanese.dat` 字节相同；host-api 的资源包清单测试漏了 #9931 新加的 `pronunciations`；脚本插件的本地时间测试与词库无关。被忽略的 `shipped_rime_import_retains_reported_common_character_positions` 以新锁的 `msime-pinyin.db` 显式运行通过。
- 变基到 `1ff98ba20d`（含 #10055 英文纠错、#10053 云词库）后，按 `verify-local.sh` 的十一组参数不带 `--update-baseline` 重跑 `convert_eval`，全部 `at baseline`，提交的基线不受这几次合并影响。
- 英文：`msime-english.db` 这次字节有变，而 #10055 的真实词库验收 `mobile_english_shipped_dictionary` 是在旧词库上调的。以 `MSIME_ENGLISH_CORRECTION_TEST_DB` 分别指向两份词库运行这个被忽略的测试，94 条期望在新旧两份上都通过（exit 0）；构建机负载约 690，计时只作参考：冷启动均值 10.54 ms（旧 13.63 ms），逐键均值 3.96 ms（旧 4.26 ms）。
- 延迟：`rerank_latency` 在新旧词库上交替各跑两轮，构建机负载 60 到 180，挂模型的 p95 旧词库 29.15、15.17 ms，新词库 16.72、97.94 ms，同一词库两轮之间的差远大于两个词库之间的差，这次量不出结论，需要在空闲机器上复测。
- 换到 `dict-v2.0.41` 时：`gen-locks --check`、`test-editions.py`、`test-language-data-notices.py`、`test-engine-license.py`、`test-macos-package-resources.py`、`test-fetch-language-dictionaries-bounds.py`、`test-linux-setup-download-links.py`、`test-reference-feature-log.py` 通过；新发布的笔画、注音库读回的 `license` 分别是 `LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0` 和 `LGPL-2.1-or-later AND BSD-3-Clause AND MIT`。在 Mac Studio 上 `cargo test -p msime-engine -p msime-input-runtime -p msime-host-api -p msime-client-core -p msime-engine-capi` 不设资源时 5160 项全部通过（含钉住新代次的 `full_keeps_the_desktop_lock_and_its_generation` 和音节频度表的来源行测试）；以 `MSIME_EVAL_RESOURCES` 分别指向两份词库时各 5154 项通过、6 项失败，失败的是同一组且都与换锁无关：四个 golden 真实集（按设计要 `dict-v2.0.1`）、`japanese::decoder::tests::real_model_loads` 与 `session::tests::real_resources_mix_japanese_conversions`（两版的 `msime-japanese.dat` 字节相同）。双拼触点整句集那份要跑几个小时，这次没有跑。
- 换到 `dict-v2.1.0` 时：新锁资源用 `install_resources` 装到本机，旧锁资源由 `gh release download dict-v2.0.41` 加 `resource_lock_from_sums.py --stage` 搭出（生成的锁与提交的旧锁逐字节相同；`--stage` 用硬链接，`ResourceStore::verify` 拒绝链接数大于 1 的文件，要换成复制）。`MSIME_EVAL_RESOURCES` 指向新资源时 `cargo test -p msime-engine -p msime-dict-builder -p msime-client-core -p msime-input-runtime -p msime-host-api -p msime-client-providers -p msime-engine-capi -p msime-engine-wasm` 只有 6 项失败，在旧资源上同样失败：四个 golden 真实集（按设计要 `dict-v2.0.1`）、`real_model_loads` 与 `real_resources_mix_japanese_conversions`（日文词典两版逐字节相同）。被忽略的真实词库测试 `mobile_english_shipped_dictionary`、`cached_keypad_rows_match_per_prefix_queries_on_the_real_dictionary`、`english_keypad_shipped_dictionary_reported_words`、`english_page_append_matches_frozen_entry_on_shipped_dictionary`、粤拼与注音两项、`shipped_rime_import_retains_reported_common_character_positions` 在新词库上全部通过（前两项在 `dict-v2.0.41` 上反而失败，它们是后来对着更新的词库调的）。引擎快速上手和 C 示例在新数据包上的输出与 README 逐字相同。锁与声明的脚本检查（`gen-locks --check`、`test-language-data-notices.py`、`test-editions.py`、`test-engine-license.py`、`test-model-notices.py`、`test-resource-lock-from-sums.py` 等）与 `web-engine-notice.sh --check` 通过。整句的双拼触点基线（几个小时）这次没有重录。
- 换到 `dict-v2.1.2` 时：`install_resources` 把新锁资源装到本机（目录名即代次 `905db0dd…`，除 `msime-pinyin.db` 和清单外与 `dict-v2.1.0` 的文件逐字节相同）。`MSIME_EVAL_RESOURCES` 指向它时 `cargo test -p msime-engine -p msime-dict-builder -p msime-client-core -p msime-input-runtime -p msime-host-api -p msime-client-providers -p msime-engine-capi -p msime-engine-wasm` 6313 项通过、6 项失败，就是 `dict-v2.1.0` 那次的同一组（四个 golden 真实集、`real_model_loads`、`real_resources_mix_japanese_conversions`）；`full_keeps_the_desktop_lock_and_its_generation`、`shipped_syllable_frequency_covers_every_spelling_and_names_its_source`、`shipped_table_is_rebuilt_from_the_locked_dictionary` 通过。被忽略的真实词库测试（英文纠错、九键英文三项、粤拼、注音、`shipped_rime_import_retains_reported_common_character_positions`）全部通过。评测、`must_outrank.py` 和 `dict_eval_gate.py` 的结果见「换到 dict-v2.1.2」一节；`bash scripts/verify-local.sh --quick` 在本机通过（quick 不跑评测集；本机没有 Docker、Windows 与 macOS 隔离构建、鸿蒙工具链，相应几项跳过）。整句的双拼触点基线这次也没有重录。
