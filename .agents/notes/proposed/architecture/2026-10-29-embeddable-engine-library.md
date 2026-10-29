# Agent Note: msime-engine 作为可嵌入的输入引擎库：MPL-2.0、C ABI 与数据发布

Status: proposed

## Problem

`crates/engine`（`msime-engine`）只给本仓库的宿主用：唯一的 C ABI 是 host-api 的 `msime_client.h`，它把打字和账号、云、语音、皮肤混在一个 1368 行的头文件里，每次按键都走 JSON，错误只有字符串；引擎本身的公开面分成「镜像 C++ API 的核心层」和「产品门面 `host::*`」两层，后者在构造时写侧车文件、`!Send`、命令编号和核心层不同。整个 crate 继承工作区的 `GPL-3.0-only`，App Store 应用和非 GPL 的输入法都不能嵌入它；librime 是 BSD-3。

维护者 2026-10-29 决定把引擎做成给其他输入法用的库。要回答的是：库的定位、许可证和仓库形态；改许可证的法律前提是否成立、由谁签字；编进引擎的 GPL 数据怎么处理；对外 ABI 的形状和 ABI 稳定之前引擎要先修什么；数据格式怎么交给第三方；分几步做、哪些步骤不必等同意书。

最硬的约束在版权上：仓库没有 CLA 或 DCO，所有贡献都是以 GPL-3.0-only 进来的；Rust 引擎是 GPL-3.0 的 C++ `metasequoiaime/msime-engine`（固定在 `a9b9f09`）逐行引用的移植，原作者 `fanlusky` 持有该提交上约 32% 的存活 C++ 行，其中包括大部分被移植的逻辑。

## Proposal

### 定位与形态（已定）

- **定位：开箱即用的高质量拼音与多语言引擎**，卖点是已有的能力：词格加神经重排、触点与拼写纠错、九键、学习。不是 Rime 式的方案驱动框架，没有方案 DSL。v1 C ABI 只覆盖其中的词格整句、拼写纠错和学习，加 10 个方案；九键、触点纠错、神经重排与搭配模型在 Rust crate 里可用（经 `SessionOptions` 的 `sentence_association`、`collocation_model` 等字段），C ABI 留到 v1.x。自定义需求由三样东西承担：通用码表方案、已有的 `ShuangpinCustomTable`、wasm 脚本插件；其中只有 `ShuangpinCustomTable` 今天就在引擎库里：引擎没有通用码表方案（`SchemeType` 是封闭的 10 变体枚举，`types.rs:99-120`，现有的通用码表解析器只在 dict-builder 构建期），要新做；脚本运行时是 GPL、`publish = false`、只在桌面编译的 `msime-script-runtime`（`crates/script-runtime/Cargo.toml:6,9,11`），不随引擎库发布。v1 对第三方的自定义承诺只有 `ShuangpinCustomTable` 和个人词典。
- **许可证：`msime-engine` 和 `msime-path-trust` 改为 MPL-2.0，不附 Exhibit B**，按 MPL 3.3 保持与 GPL 兼容，仓库其余部分仍是 GPL-3.0-only。沿用已有的 MIT 例外先例：`crates/script-sdk`、`plugins/script`、`docs/script-plugin-template` 的代码和 `plugins/phrase_table` 的数据（`docs/third-party.md:15`；`98d4316239`，[脚本插件工具链](../../implemented/feature/2026-10-26-script-plugin-toolchain.md)、[脚本插件合集](../../implemented/architecture/2026-10-12-script-plugin-collection.md)）：每个 crate 自己的 LICENSE、不继承的 `license` 字段加注释、README「许可证」段、docs/third-party.md「本项目」段。
- **仓库：留在 monorepo。** 发布 `msime-path-trust`、`msime-engine` 和新建的 `msime-engine-capi`（C ABI，cdylib/staticlib 加头文件加 pkg-config）到 crates.io 与 GitHub Releases；数据格式写成有版本的文档，另出可下载的数据发布；msime 自己的宿主最终改用公开 API（自用）。

### 硬门禁：版权人书面同意

**改许可证需要每一位版权人的书面同意。** 以下是技术比对，不是法律意见；比对依据是 `git blame -C -C -M`、上游 `a9b9f09` 的 GraphQL blame、msime-windows 的提交历史和 Rust 注释里的 C++ 行号引用。上游 C++ 的行数取自 GraphQL blame（不忽略空白）；本地 `git blame -w a9b9f09` 的结果差几行到几十行，例如 `quanpin/autocorrect_table.h` 是 Jia Sui（`jsfaint`）763 行、维护者 67 行，`quanpin/quanpin_dictionary.cpp` 是 `fanlusky` 853 行、Jia Sui 109 行。

| GitHub 登录名 | 进入 msime-engine 的内容 | 类别 | 不同意时 |
|---|---|---|---|
| `fanlusky` | C++ MSIME-Engine 原作者（2023-10-15 首个提交）。`a9b9f09` 存活 C++ 9,708/30,164 行（32.2%），被移植文件里占绝大多数：shuangpin_dictionary.cpp 1212/1353、quanpin_dictionary.cpp 851/1341、quanpin_query.cpp 1061/1322、user_dictionary_journal.cpp 887/1470、english_dictionary.cpp 339/430、romaji_converter.cpp 234/295、helpcode_utils.cpp 216/312、japanese_sentence_decoder.cpp 316/429、providers/ 674/875、schemes/ 536/653。msime-windows 里他的代码还经 `pinyin/autocorrect.rs` 的 `QU:`/`TP:` 引用间接移植（未逐行核对）。上游的 git 作者名 `fanlumaster`、`fanlusky`、`Lu Fan(SonnyCalcr)` 用同一个邮箱，GitHub 都归到账号 `fanlusky`（id 69321395）；只接受这个账号的同意评论，另外存在的 `fanlumaster`（id 290236740，2026-06-03 注册）、`sonnycalcr`（id 168979954）账号不能当作同一主体 | 必须同意 | 改许可证作罢，见下文 |
| `jsfaint` | 上游 C++ 1,839 行（autocorrect_table.h 830、quanpin_utils.cpp 224、quanpin_dictionary.cpp 114、input_session_composition.cpp 111）；msime-windows 的 `octagram_gram.{cpp,h}`（GPL-3.0，371/371 行）被逐函数翻译进 `lattice/octagram.rs`，105 行注释里 28 行几乎逐字相同；`pinyin/autocorrect.rs` 的建表规则、邻键表、过滤与排序（约 200/1147 行，加测试夹具）移植自他的 `generate_quanpin_autocorrect.py`；本仓库 #7596、#7607 存活约 300 行 | 必须同意 | 同上（octagram.rs 可只照 librime-octagram 重写，但他的上游 C++ 分散在全拼各处） |
| `apple-ouyang` | 14 个 PR，存活约 4,776 行（引擎约 1.8%）：`lattice/octagram.rs`、`shuangpin/mid_sentence.rs`、`local/symbol_groups.{rs,tsv}`、五笔、排序、日期等；`symbol_candidates.tsv` 3 行；`policy.h` 17 行（引擎里只在测试用） | 必须同意 | 同上（分布在核心路径，重写不现实） |
| `EwigMidori` | `lattice/decode.rs`、`lattice/merge.rs` 的建图、beam、回溯、合并契约和测试表是 `243ceefbee` 的 `word_lattice.{cpp,h}` 逐函数翻译；`decode.rs:3` 的出处说明本身就是他的文字 | 必须同意 | 同上（句子组合的骨架） |
| `ManwoodIsolation` | `pinyin/segment.rs:9,127-324` 的纠错别名表（同序）、`RankedPath`、`cut_*_with_corrections` 和 `:541-570` 的测试表，来自 `84a48e1417`、`1017239127`；上游 `quanpin_dictionary.cpp` 另有他的 21 行存活（`git blame -w a9b9f09`），是否进了 Rust 未逐行核对 | 同意或重写 | 重写 segment.rs 纠错段（约 200 行）；别名顺序是行为契约，重写后要重录相关基准 |
| `cyj-hashtag` | #9379：`korean/romaja.rs`（320 行，自成一体，映射数据来自 rime-mungyeong 中文件头声明 public domain 的那一个码表文件）加 dubeolsik/scheme/mod.rs 的少量改动，存活约 538 行 | 同意或重写 | 重写 `korean/romaja.rs` |
| `SlimeNull` | #9549 两个提交，存活约 305 行，大半是测试：`dictionary/pinyin.rs`、`shuangpin/dictionary.rs`、`lattice/decode.rs` | 同意或重写 | 不是独立文件，按原问题重新修并重写测试 |
| `alex3236` | #8035，存活约 140 行，约 100 行是测试：`host/session.rs`、`session/{input,commit}.rs` | 同意或重写 | 重写这次修复 |
| `yyy735934-prog` | #2608：`host/glosses.rs` 的英文音标读取及测试；git 作者名是 `clearing`，`git blame -w` 存活 102 行（`host/glosses.rs` 49、`host/tests.rs` 48、`diagnostics.rs` 5） | 同意或重写 | 重写音标读取部分 |
| `ouchihao` | `pinyin/syllables.rs:94-113` 的 `normalize_umlaut_aliases`（约 6 行同名同条件的逻辑）加 jv/qv/xv/yv 四个音节串，来自 `d40e10718c`；同一提交在上游 `quanpin_dictionary.cpp` 有 15 行存活（`git blame -w a9b9f09`），它加进 `autocorrect_table.h` 的 v 别名行已被 `c508c845ee` 删掉，在 `a9b9f09` 没有存活 | 同意或重写 | 量小，可能不构成作品；稳妥起见重写 |
| `PathGao` | `user_dictionary/ranking.rs:447,451` 意译了他的注释，`:458` 的 `owns_entry_key` 同名同概念（`d8267e7bfe`）；上游另有 `86c5fa76af`（`quanpin_dictionary.cpp` 里一个上限常量 3→4，存活 1 行）和 `7c4446f8d7`（只删行与改测试） | 同意或重写 | 重写注释与该闭包 |
| `houko` | 维护者本人（提交身份 小莫、Evan、Evan Hu）；`crates/path-trust` 全部、`policy.h` 上游原始提交 | 无需 | — |
| `Aegon8719` | 代码只在 `shared/contracts/ipc_negotiation.h` 和 Windows 宿主，不进引擎；那部分保持 GPL | 无需 | — |
| dependabot、Copilot/Claude 署名行 | 3 个一行的版本号提升；AI 署名行 | 无需 | — |
| `Jiangpip`、`metasequoiaime-dev`、`codex` | 上游各 1 次提交：`91f472e0a6` 只加词库数据包（`packs/unreal_houdini/*`、`translations.txt`、`tools/validate_packs.py`）；`18bf54fc22` 是合并 `jsfaint` PR 的合并提交；`b6ab06af15` 只加 CI 工作流 `notify-apple.yml` | 无需 | — |
| libchewing 作者 | `zhuyin/` 核对后没有复制代码：只共享大千键位（事实）、注音槽位名和模仿的编辑行为，标识符零命中 | 无需 | — |
| libpinyin 作者 | 只借了 Viterbi 加 beam 的思路和 beam=32 这个参数，没有共同标识符或插值公式 | 无需 | — |

未核实、P0 内要补完的：sunpinyin 源码没有比对（只声称借了 lattice columns 的概念）；msime-windows（`2dc940dc`）的 `engine/` 除 `fanlusky`、Jia Sui、`ouchihao` 外还有 HowGeGe、chr331、沐雪清泽、mushiyuki、古明四季、luojiyin、normaltank、AkaneIwasa、Jiaxin Ran、時崎 ギャラクシー、AaGMix、alayahbridge319-boop 的提交，是否进了 Rust 没查：其中 `engine/quanpin/quanpin_utils.cpp` 有 HowGeGe 59 行、chr331 38 行存活（`git blame -w`），Rust 的 `QU:` 引用可能指向这个文件；沐雪清泽的 `93c7b812`（日文 `nn` 记作一个ん）与 `japanese/romaji.rs:398-402` 的规则相同；`apple-ouyang` 的 #9243、#9253、#9265、#9305 对齐的 Windows 行为（`mid_sentence`、`direct_helpcode`、`date_time_query`）据评审的 blame 大部分出自 `fanlusky`，若属实已在他的同意范围内（未复核）；`TP:` 引用指向的 `TypoProfile` C++ 文件在 `a9b9f09` 和 msime-windows HEAD 都找不到，来源未定；`ouchihao` 在 `467b9804` 之后改过 msime-windows 的生成器（`1671c3a467`），是否进了 Rust 没有查；Rust 里 `WL:`/`QQ:` 行号对应的确切 C++ 版本（大概是固定提交加当时的 overlay）没有找到。

**同意结果（2026-10-29）。** 维护者确认表中「必须同意」和「同意或重写」两类版权人全部同意把各自被移植或包含进 msime-engine、msime-path-trust 的贡献以 MPL-2.0（不附 Exhibit B）再许可；不开跟踪 issue，不在 GitHub 上逐个提及。同意记录由维护者自行留存，本仓库不保存联系方式或同意书原件。P2 据此与 P1 同时推进，合并仍按依赖顺序：P1 的 rink 自写定义、latex 表进 crate、符号分组重建先合，P2 后合。

**（同意之前的退路）`fanlusky` 不同意时，改许可证作罢。** 库照样可以以 GPL-3.0-only 发布：P3 到 P7 不依赖许可证，crates.io 接受 GPL crate，GPL 和 LGPL-2.1-or-later 的前端（如 fcitx5、ibus）仍能用，只是 App Store 和非 GPL 的采用方被挡在外面。净室重写不现实：按 `.cpp:`/`.h:` 行号或 `QQ/QU/QUH/QD/SD/WL/TP/GEN/J/NS/RP` 缩写引用 C++ 的 Rust 文件有 80 个、约 9.5 万行（含测试；引擎 `src` 共约 17.1 万行；只算 `QQ/QU/WL/TP/GEN` 是 72 个、约 8.8 万行），他的部分覆盖每个中文方案的核心（全拼、双拼词典、用户词典日志与排序与回放、英文、日文、辅助码、provider、方案）；净室要求作者没读过 C++ 和现有 Rust，维护者和现有贡献者都不合格，从 C++ 录下的黄金基准也是派生物。这等于重写引擎核心，不在本计划范围内。`jsfaint`、`apple-ouyang`、`EwigMidori` 不同意的后果实际相同。

P0 期间还有外部 PR 合进引擎时，同意名单会变长。P0 期间这些 crate 仍标 GPL-3.0-only，按 GitHub 服务条款 D.6「入站等于出站」，入站贡献默认只按仓库标明的许可证授权，单靠 CONTRIBUTING 的一段话证据很弱。所以 P0 起：`crates/engine`、`crates/path-trust`、`crates/engine-capi` 各自的 README 写明入站贡献以「GPL-3.0-only OR MPL-2.0」授权，CONTRIBUTING 同步；PR 模板加一条触及这些路径时必勾的确认项；P0 期间触及引擎的外部 PR，合并前要求作者在 PR 里回复同意文字。P2 之前对最终的树跑一次 `git blame -w -C -C -M`（按路径过滤的 shortlog 漏掉从别的 crate 搬进来的代码，P3 就要搬维护锁），并统计触及这些路径的提交里非 AI 的 `Co-authored-by` 尾行（`cyj-hashtag` 已经在引擎提交 `5a3b741191` 里以尾行出现）。

### 编进引擎的 GPL 数据和仓库外文件

| 项 | 事实 | 处理 | 阶段 |
|---|---|---|---|
| rink-core `bundle-files` | `crates/engine/Cargo.toml:23` 打开 `bundle-files`，`simple_context()` 把 GNU Units 的 `definitions.units`（294,252 字节，FSF，GPL-3.0-or-later）编进每个二进制。引擎只在 `local/units.rs` 用约 20 个单位名加 6 个自定义的 `shi*` | **去掉 `bundle-files`**，`context()` 改为 `Context::new()` 加一段 msime 自写的 `BASE_DEFINITIONS`（约 19 行：`kg !`、`m !`、`lb 0.45359237 kg`、`inch 2.54 cm`……，按法定值写，不抄上游行和注释）再加载现有的 `SHIZHI_DEFINITIONS`。`units.rs:329-436` 的精确字符串测试做判据；输出逐字节不变是推断，未编译验证。同时改 `docs/third-party.md:43`、macOS 与 Linux 的 THIRD_PARTY_NOTICES 和 Cargo 注释。备选是 `unit-conversion` 特性默认关闭、只在 input-runtime 打开，产品照样带 GPL 数据，多出 cfg 面，只在维护者不接受自写定义时用 | P1 |
| `nine_key/syllable_frequency.tsv` | 424 个按音节聚合的权重，取自 dict-v2.0.39 的 `msime-pinyin.db`（rime-ice GPL-3.0 加补充），不含任何词、读音或单条权重，只作排序先验。求和的权重里，与万象共有的词条自 `dict-v2.0.30` 起改编自 rime-wanxiang（CC BY 4.0），见[词库里改编自 rime-wanxiang 的权重重新署名](../../implemented/process/2026-10-29-rime-wanxiang-weights-attribution.md) | 先保留并在 docs/third-party.md 与 NOTICE 加一行，说明它是 rime-ice 派生库的聚合统计；维护者在 P2 之前裁定（见待拍板 1）。同时用 Unihan `kHanyuPinlu`（Unicode-3.0）原始计数加缺失音节的下限做原型（约 26 个音节缺失，约占权重 0.4%），九键评测集不退步才替换；替换要改 `nine_key.rs:4501` 的覆盖测试并重跑[九键音节频率](../../implemented/bug-fix/2026-10-27-nine-key-syllable-frequency-table.md)里的基线 | P1 起，P2 前定 |
| `local/symbol_candidates.tsv` | 自写，首行写「随引擎以 GPL-3.0-only 发布」；源头是 msime-dictionary `sources/symbols/symbol-names.tsv` 和其 `NOTICE.md:108`（在锁文件所指的提交 `3d4b67cb67`；msime-dictionary HEAD 上行号已变），本地有字节校验 | 先改 msime-dictionary，再提升 `resources/symbol-tables.lock.json` 的提交与 sha256 并重新同步 | P2 |
| `plugins/phrase_table/latex-symbols/symbols.tsv` | MIT 自写，`local/symbol_groups.rs:20-21` 在非测试代码里从 crate 外 `include_str!`；`cargo package` 不会带上它，打包校验会失败 | 在 `symbol-tables.lock.json` 加一个 `copy` 目标，同步一份到 `crates/engine/src/local/`，改为 crate 内路径，随库带 MIT 声明 | P1 |
| 其余内嵌表 | `places.tsv` WTFPL、`hanja.tsv` libhangul BSD-3、`chaizi*.tsv` kfcd/chaizi CC BY 3.0 加 Unihan Unicode-3.0 加 OpenCC Apache-2.0、`symbol_groups.tsv` rime-wanxiang CC BY 4.0，其分组上游自述最初抄自 rime-prelude 的 `symbols.yaml`（rime-prelude 仓库是 LGPL-3.0；msime-dictionary `NOTICE.md` 的 `wanxiang_symbols.yaml` 一行、`docs/third-party.md:295`），这条授权链未审 | 数据保留各自许可证，随库带署名与许可证文本；chaizi 与主许可证的关系 `docs/third-party.md:296` 写明从未审过，未经律师审阅。`symbol_groups.tsv` 若按 LGPL-3.0 处理，静态编进 MPL 库对 App Store 采用方不可接受（LGPL-3.0 §4 的替换要求），届时改为运行时数据，或只用码位事实另行重建分组 | P1 起核，P2 前定 |
| 只在测试里用的仓库外文件 | `shared/contracts/punctuation/policy.{json,h}`、`resources/eval/sentences-nine-key-v1.tsv`、`resources/helpcodes/shouyou2_0_helpcode.txt`（无再分发授权，见下文辅助码一段）、`resources/grammar-models.lock.json` | 不进 crate 包；`shouyou2_0` 永远不能打包 | P6 |

### 第三方改编代码与声明

| 文件 | 来源 | 许可证 | 要做的 |
|---|---|---|---|
| `handwriting/model.rs`、`features.rs` | Zinnia 提交 `581faa8f` | BSD-3 | 保留声明 |
| `lattice/octagram.rs` | 格式与算法来自 librime-octagram 提交 `57d18b9f`；代码是 `jsfaint` GPL C++ 的翻译；trie 读取逻辑重实现自 darts-clone | BSD-3、GPL-3.0、BSD-2-Clause（darts-clone） | 文件头写了 RIME Developers 和旧 Windows 文件路径 `engine/ngram/octagram/octagram_gram.cpp`（`octagram.rs:7-8`），但没写 msime-windows 仓库、作者 `jsfaint` 与 GPL-3.0：补上这些和 darts-clone 署名（P1，与改许可证无关都该补） |
| `lattice/cxx_sort.rs` | 照 libc++ 的 `__algorithm/partial_sort.h`、`sort.h`、`sift_down.h`、`pop_heap.h`、`push_heap.h`「逐步」复现（`cxx_sort.rs:1`） | Apache-2.0 WITH LLVM-exception | 需要署名，是既有缺口：LLVM exception 只免除编译进目标码时的 §4(a)(b)(d)，源码级翻译仍按 Apache-2.0 §4，在文件头写出处与改动说明，在 `resources/licenses` 加 LLVM 许可证全文并接入各声明渠道；`docs/third-party.md` 现在没有 libc++ 一行（P1） |
| `korean/romaja.rs` | rime-mungyeong 提交 `34c1b14` 的 `mungyeong_romaja.dict.yaml` | 文件头声明 public domain；仓库整体是 GPL-3.0，没有复制它的 Lua（`docs/third-party.md:292`） | 无 |
| `korean/hanja.tsv` | libhangul 提交 `717409ce` 的 `data/hanja/hanja.txt` | BSD-3（单文件声明；仓库整体 LGPL-2.1；部分读音整理自国立国语院材料，能否以 BSD 再授权无法核实，`docs/third-party.md:293`） | 二进制声明 |
| `local/chaizi*.tsv` | kfcd/chaizi | CC BY 3.0 | 署名 |
| `local/symbol_groups.tsv` | rime-wanxiang v18.0.16；分组最初抄自 rime-prelude `symbols.yaml` | CC BY 4.0；上游链条 LGPL-3.0，未审 | 署名；LGPL 链条见上表 |
| `local/places.tsv` | modood/Administrative-divisions-of-China | WTFPL | 声明 |
| `zhuyin/` | 只有大千键位与 libchewing 的行为 | — | 可选：`layout.rs:1` 改写为「标准大千布局（libchewing 也用）」；`conversion.rs:33`、`scheme.rs:746` 把平方和计分误称为 libchewing 的算法，改成准确描述（P1） |
| `zhuyin` 词库 `msime-zhuyin.db` | libchewing-data | LGPL-2.1-or-later | 数据，不随代码改许可证，声明已接好 |

### 改许可证时要改的声明（P2 清单）

- [ ] crate 元数据：`crates/path-trust/Cargo.toml:6` 改为 `license = "MPL-2.0"`；`crates/engine/Cargo.toml:6` 不能只写 `MPL-2.0`，因为 `.crate` 里还带着各自许可的数据和改编代码（`hanja.tsv` BSD-3、`chaizi*.tsv` CC-BY-3.0 加 Unicode-3.0 加 Apache-2.0、`symbol_groups.tsv` CC-BY-4.0、`places.tsv` WTFPL、latex `symbols.tsv` MIT、Zinnia 与 librime-octagram 的 BSD-3、darts-clone 的 BSD-2-Clause、libc++ 的 Apache-2.0 WITH LLVM-exception），crates.io 和 cargo-deny 把 `license` 当作整个包的 SPDX 表达式。写成 `MPL-2.0 AND BSD-3-Clause AND BSD-2-Clause AND CC-BY-3.0 AND CC-BY-4.0 AND Unicode-3.0 AND Apache-2.0 AND Apache-2.0 WITH LLVM-exception AND MIT AND WTFPL`（`symbol_groups.tsv` 若按 LGPL-3.0 定性且仍内嵌，再加 `LGPL-3.0-only`），注释说明代码是 MPL-2.0、内嵌表保留各自许可证；或改用 `license-file` 加 crate README 里的逐文件表。两处都照 `crates/script-sdk/Cargo.toml:7-8` 加注释，工作区 `Cargo.toml:29` 不动；两个 crate 各加 MPL-2.0 全文的 LICENSE（取自 `resources/licenses/MPL-2.0.txt`），不加 Exhibit B；全文本身不等于附上了 Exhibit A 声明（MPL §1.4 的 Covered Software 以带 Exhibit A 为准，Exhibit A 允许放在相关目录的 LICENSE 里），所以 LICENSE 开头或 README「许可证」段写 Exhibit A 原文「This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. …」，可选在 `lib.rs` 顶部加同一句；`crates/engine/Cargo.toml:3` 的描述改写成对外的说法。
- [ ] README：`README.md:159`；`docs/readme/README.{en,ja,ko,zh-Hant}.md:157`。徽章（`README.md:18` 和各语言版 `:20`）不动：产品整体仍是 GPL-3.0-only，MIT 例外的先例 `98d4316239` 也只改了许可证段。
- [ ] 文档：`docs/third-party.md:15,19,43,168,293-296`；`docs/open-source-release.md:15`（`:22` 保留 C++ 出处）；CONTRIBUTING 的入站许可证条款转为正式规则。
- [ ] 数据头：`symbol_candidates.tsv:1`（经 msime-dictionary 与锁文件）。
- [ ] 桌面声明：`platforms/linux/data/THIRD_PARTY_NOTICES.txt:4,8`、`platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt:4`、`platforms/windows/Collect-Notices.ps1:20` 及其测试 `platforms/windows/tests/tools/collect_notices.ps1:12-26`、`platforms/linux/collect-notices.py:9,43`、`platforms/windows/Notices.md:11,14`。
- [ ] 网页声明：`scripts/web-engine-notice.sh:4-5,56,116`，重新生成 `resources/licenses/web-engine-NOTICE.md`；若把 `crates/engine/LICENSE` 编进网页声明，同步 `ci.yml:12`、`ci-platforms.yml:89-92` 的路径过滤。
- [ ] 发行版打包：`platforms/linux/packaging/debian/copyright:19-25` 加 `Files: crates/engine/* crates/path-trust/*` 的 MPL-2.0 段；`platforms/linux/nix/fcitx5.nix:287-291` 加 `mpl20`。rpm spec、arch PKGBUILD/.SRCINFO 已列 MPL-2.0，要改就三处一起改（`scripts/test-arch-gentoo-packaging.py:123-135` 要求一致）。
- 不改：产品整体仍是 GPL-3.0-only，`packaging.cmake`、`msime-linux.xml.in`、winget/scoop、Windows 安装包与关于页、`license-rows.tsx`、tauri 配置、`packages/web-engine`、`release-pack-tool.yml` 保持原样。

P1 顺手补的既有缺口（与改许可证无关）：移动端声明渠道都没有 MPL-2.0 全文，rink-core 早已是 MPL（iOS `project.yml:111-145`、Android `build-native.sh:64-97`、HarmonyOS `stage-resources.sh:40-57`）；网页声明缺 rime-wanxiang、latex-symbols 的 MIT 和 librime-octagram；debian copyright 没有现有 MIT 例外的段；gentoo `msime.ebuild.in:30`、`msime-9999.ebuild:20` 的 `LICENSE=` 缺 MPL-2.0；`docs/third-party.md:38` 还把只在开发依赖里的 wana_kana 列成引擎依赖，`:36` 的 lru 写 0.16 而锁文件是 0.18.5；rpm spec（`msime.spec:31`）、arch `.SRCINFO` 和 gentoo `msime.ebuild.in:30` 都缺 chaizi 的 CC-BY-3.0，gentoo 同时缺 Unicode-3.0；`platforms/linux/nix/fcitx5.nix:287-291` 只列 `gpl3Only`（缺 CC-BY-3.0/4.0、BSD-3、WTFPL、Unicode-3.0、MPL-2.0）；`lattice/cxx_sort.rs` 没有 libc++ 署名（见上表）。

### C ABI 草案（msime-engine-capi）

新 crate `crates/engine-capi`，包名 `msime-engine-capi`，`crate-type = ["cdylib", "staticlib"]`，`#![allow(unsafe_code)]`，符号前缀 `msime_engine_`。它包装核心层 `msime_engine::Session`，不包装 `host::Session`，也不扩展 host-api。它自己的文件只有维护者的代码，从第一天起就是 MPL-2.0；二进制发布等 P2，或在改许可证作罢时以 GPL 发布。

- **版本**：头文件宏 `MSIME_ENGINE_ABI_MAJOR`/`MINOR`，`msime_engine_abi_version()` 返回 `(major<<16)|minor`；每个进出结构体以 `uint32_t struct_size` 开头，库只写 `min(调用方大小, 库内大小)` 并把剩余部分清零；同一个 major 内只追加函数和结构体尾部字段；Linux SONAME 的 major 等于 ABI major。数值枚举冻结且只追加，沿用 `SchemeType`（`types.rs:99-120`）和 `CandidateSource`（`types.rs:5-38`）的序号，命令用**核心层** `Command` 的编号（`types.rs:1085-1109`），不用 host 门面的。被排除的候选来源保留编号。
- **错误**：所有函数返回 `int32_t`：`OK=0`、`INVALID_ARGUMENT=1`、`FAILED=2`、`STORAGE=3`、`UNSUPPORTED=4`、`STALE=5`（revision 不符）、`BUSY=6`（重入或并发）、`PANIC=7`（会话已毒化，只能释放）、`NULL=8`。`msime_engine_last_error()` 返回线程局部的说明文字，到该线程下一次调用前有效。
- **内存**：输入是借用的 `(ptr, len)` UTF-8。热路径输出是 `msime_engine_str {ptr, len}`，指向会话按 revision 缓存的快照，到同一会话下一次改变状态的调用（按键、命令、选择、`set_*`、`switch_scheme`、`reset_context`）前有效；`state_get`、`candidate_get`、`last_error` 这类只读调用不使它失效，前端可以先取状态再逐个取一页候选。ABI 边界不为返回值另外分配、不用 JSON；引擎内部的分配不在此承诺内（核心 `Session::snapshot()` 返回自有的 `SessionSnapshot`，`String` 字段加 `Vec<WordItem>`，`session/mod.rs:659`）。只有冷路径（创建会话的选项、调试快照、个人词典）用 JSON，返回库持有的 `char*`，用 `msime_engine_string_free` 释放，不要用 `free()`。
- **线程**：会话可在任意线程使用但不能并发，内部 `try_lock` 失败返回 `BUSY`；不同会话可并行；进程级函数线程安全。能否跨线程以 P3 的线程亲和结论为准（见硬点表）：类型层面是否 `Send` 只是必要条件，日志连接按线程缓存才是实际的亲和；P3 修不掉时退回 host-api 式的线程绑定，ABI 写明「会话固定在创建它的线程」。每个入口都包 `catch_unwind`，panic 毒化会话。
- **函数（33 个）**：库 9 个（`abi_version`、`version`、`last_error`、`string_free`、`prepare_runtime`、`flush_learning`、`release_caches`、`close_databases`、`preload_model`）；会话 6 个（`session_new(options_json)`、`session_free`、`switch_scheme`、`set_int`、`set_string`、`reset_context`）；输入 4 个（`key_char`、`key_punct`、`command`、`set_caret`）；状态 4 个（`state_get`、`candidate_get(revision, index)`、`expand_candidates`、`state_json`）；选择与逐候选学习 7 个，都带 `revision`（`select`、`select_edge`、`finish`、`pin`、`remove`、`fix_position`、`clear_position`）；个人词典 3 个，不需要会话（`user_dict_list`、`user_dict_edit`、`reset_learning`）。
- `options_json` 是 `SessionOptions` 的一个有文档、有版本的子集（路径、方案与启用方案、双拼/五笔/越南语/韩语布局、辅助码、标点、学习、个人上下文、模糊音、纠错、英文混输、首页行数、语言词库路径），未知键拒绝。`set_int`/`set_string` 的选项编号：`CHINESE_PUNCT`、`PAIRED_PUNCT`、`PUNCT_LOCK`、`DEDICATED_ENGLISH`、`HELPCODE_ENABLED`、`HELPCODE_SCHEMA`、`PERSONAL_CONTEXT`、`PREDICTION_ALLOWED`、`FIRST_PAGE_ROWS`、`RESCORING_CONTEXT`。
- **不进 ABI**：在线、云与 AI 候选；插件指令；翻译与释义；宿主喂入的表（指令表、联系人、快捷短语，可在后续 minor 作为通用「用户短语表」加入）；同步与备份；诊断常量、`backend`、wasm 专用的时钟、octagram 驱逐、`set_clock`、`set_touch_tuning`；只为黄金基准公开的 setter；排序内部（`ordering::*`、重排模型）；实验性能力（多语言、日文混输、触点/滑行/九键双拼/14 键网格留到 v1.x、手写、emoji 目录、英文补全与音标、`hanzi_to_pinyin`）。第三方拿到的是引擎原始顺序，`sentence_alternatives` 保持 false。
- **交付物**：头文件用 cbindgen 生成并提交，CI 检查漂移；`msime-engine.pc` 带 `Version`、`Libs`、`Cflags` 和默认共享数据目录变量；显式设置 SONAME（macOS 设 `install_name`）；文档列出 staticlib 需要的系统库；一个 C 示例分别用静态库和动态库链接。每个二进制归档附 `THIRD_PARTY_NOTICES`（用 `platforms/linux/collect-notices.py` 按 `msime-engine-capi` 的依赖图生成，覆盖全部传递 crate 和 bundled SQLite）、MPL-2.0 全文及 rink 源码位置（MPL §3.2）、内嵌数据表的许可证与署名。staticlib 内含 bundled SQLite（`crates/engine/Cargo.toml:24` 的 `rusqlite` 开 `bundled`）的全局 `sqlite3_*` 符号和整份 Rust std：C 示例另加一个同时链接系统 libsqlite3 的用例，文档写明一个进程只能链接一个 Rust staticlib（这类重复符号问题是已知类别，没有在工具链上实测）。

### ABI 稳定之前引擎侧要修的硬点（P3）

| 硬点 | 现状 | 方向 |
|---|---|---|
| 收尾在门面不在核心 | 已解决：核心 `Session` 的 `Drop` 冲刷本日志队列、交还日志连接、`note_settled`，`LocalDatabaseLease` 也在核心会话里（[核心会话收尾](../../implemented/architecture/2026-10-29-core-session-lifecycle-and-journal-threads.md)）；CommitRaw 的英文学习策略是核心选项 `SessionOptions::learn_typed_spellings_on_enter`（缺省关，门面打开），`CommitRawWithoutLearning` 是核心 `Command` 的 14 号（[回车学词策略进核心](../../implemented/architecture/2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)）。辅助码注释规则仍只在门面 | 收尾进核心 `Drop` 或 `close(self)`；租约和提交策略改为核心层的可选项 |
| 引擎里没有 revision | host-api 的 `generation` 来自 input-runtime | 核心会话自己维护候选 revision，每次改变候选的调用递增 |
| 没有分页和按键路由 | 字符/标点/命令/数字选择由宿主决定（参考 `spelling_symbols`、`takes_local_separator`）；分段移动在 `input-runtime/src/engine.rs:197-238` 组合；分页只在 input-runtime 和 engine-wasm；单字母全拼只给前 24 行（`quanpin/dictionary.rs:80`），要 `expand_initial_candidates` | 头文件写明路由规则；分页留给宿主，但 `candidate_get` 按下标随取 |
| `SessionOptions` 约 50 个公开字段、两套默认值 | 核心默认辅助码 `lantian`、学习开（`options.rs:129,141`）；门面默认 `ziranma`、学习关（`host/options.rs:234,247`）；加字段就是 Rust 的 semver 事件 | 只保留一套默认值；对外字段集合以 `options_json` 的文档为准 |
| 错误是字符串 | `EngineError` 4 个变体，其中 `InvalidArgument`、`Failed` 带 C++ 原文，`Sqlite`、`Io` 是透明包装（`error.rs:5-17`）；`KeyResult.diagnostic` 是 `diagnostics.rs` 89 个常量的自由文本 | 加错误码枚举和诊断码映射表 |
| 字节与标量单位混用 | 输入只收 ASCII `u8`；`caret_position` 按字节，`conversion_focus` 按 Unicode 标量 | ABI 字段名带单位；内部统一前先在包装层换算 |
| 进程级副作用 | 冲刷线程 `msime-personal-context` 从不 join，创建失败就 panic（`ngram_store.rs:746-760`）；日志连接按线程缓存；`reset_cache` 清进程级缓存；`Box::leak` 的驻留表；约 163 处 `eprintln!`，没有日志出口；不能干净卸载 | 冲刷线程创建失败返回错误；加 `shutdown` 冲刷并停线程；日志改走可注入的出口；文档写明「每进程一个实例」直到去掉全局状态 |
| 跨进程维护锁在 GPL 的 client-services | `.msime-dictionary-access.lock` 在 `crates/client-services/src/dictionary/access.rs` | 把锁搬进引擎，或 ABI 写明「有会话打开时不调用 `user_dict_edit`/`reset_learning`」。`access.rs` 与它依赖的 `crates/client-base/src/file_lock.rs` 经 `git blame -w -C -C -M` 只有维护者的两个提交邮箱，搬进引擎无需额外同意 |
| 线程亲和 | `Session` 在类型层面大概率是 `Send`（`RefCell<T: Send>`、`rusqlite::Connection` 都是 `Send`，只是 `!Sync`；未编译验证）。实际的亲和来自按线程的缓存：日志连接存在 `thread_local! CACHED_JOURNAL`（`user_dictionary/journal.rs:109-111`），`release_thread_journal` 只清调用线程的那份，其他线程的连接要等它们下次访问日志才关（`journal.rs:370-376`），在 Windows 上挡住重置的改名和代次删除；`lattice/decode.rs:193`、`lattice/ngram.rs:615`、`ime/personal_rerank.rs:309` 也有线程局部缓存；单位换算另起后台线程 `msime-unit-context`（`local/units.rs:145-147`） | 收尾、`close_databases` 改为关掉所有线程缓存的连接（代次计数之外加主动关闭），或 ABI 写明「会话固定在创建它的线程」；`Send` 静态断言只是必要条件，另加「会话跨线程使用后 `reset_learning` 成功」的集成测试 |

### 要写成文档的数据格式（P5）

运行时文件（根目录由 `RuntimePaths{resources, user_data, cache, dictionaries}` 给出，都必须是绝对路径；`prepare_runtime_paths` 在 `user_data/dictionaries/<content_id>` 原子地准备一个代次并回放日志）：

| 文件 | 格式 | 读端版本检查 | 谁需要 | 数据许可证 |
|---|---|---|---|---|
| `msime-pinyin.db` | SQLite，`tbl_<1..7\|others>_<首字母>(key, jp, value, weight)`，`quick_parases` | 无 | 全拼、双拼、五笔、九键 | GPL-3.0（rime-ice 及补充） |
| `msime-wubi.db` | SQLite，`wubi86`/`wubi98(key, value, weight)` | 只看表在不在 | 五笔 | Apache-2.0（wubi86 极点）、LGPL-3.0（rime-wubi 补入的词条）、GPL-3.0-or-later（fcitx wubi98）、Unlicense（98 五笔小组）、本项目生成的补充表 |
| `msime-english.db` | SQLite，`english_words`、`en_zh_glosses`、`zh_en_glosses`、`source_notices` | 写 `user_version=3`，读端不查 | 全拼、双拼、五笔要求；其余方案只有专用英文用它，可缺（`SchemeSet::requires_english_dictionary`） | GPL-3.0（rime-ice-en 及补充）；附 SCOWL 版权声明（`crates/dict-builder/README.md:3`）；Google 词频「以来源页面说明为准」 |
| `msime-bigram.bin`/`msime-trigram.bin` | `MSNG`，u32 版本 1，排序的 FNV-1a 64 键加 f32 | 魔数、版本、排序 | 句子打分，可缺 | 计数来自 zhwiki（CC-BY-SA-4.0）；词表是 GPL-3.0 的 `msime-pinyin.db`（msime-dictionary `README.md` 的补充说明） |
| `msime-others.db` | SQLite，emoji、颜文字、符号目录 | 无 | 本地模式，可缺 | emoji 来源没有记录、PiliApp 符号未声明 |
| `msime-japanese.dat` | `MSJPDT1\0`，56 字节头 | 魔数、版本、边界 | 日文，可缺（只出假名） | Mozc 开源词库，随 Mozc 的 README 与 LICENSE（BSD-3 加词典附加条款） |
| `msime-{cantonese,zhuyin,stroke}.db` | SQLite，`metadata`、`syllables`、`entries` | `metadata.format_version == "1"` | 对应方案，缺了激活失败 | 粤语 CC BY 4.0（rime-cantonese、HKCanCor）；注音 LGPL-2.1-or-later（libchewing-data）加 McBopomofo 的 MIT；笔画 LGPL-3.0（rime-stroke） |
| `sentence-model.safetensors` | safetensors，`__metadata__` 的 `config`、`vocab` | crate 内几何校验，64 MiB 上限 | 神经重排，默认关 | Apache-2.0（chinese-ime-lm 权重） |
| octagram `.gram` | Rime `Rime::Grammar/` 头加 Darts 双数组 | 只看前缀，不验校验和 | 搭配打分，可缺 | 按模型：rime-wanxiang-data CC-BY-4.0、rime-octagram-data LGPL-3.0、rime-frost GPL-3.0 |
| `handwriting-zh_CN.model` | Zinnia 二进制 SVM | 魔数异或文件大小、版本 | 手写 | LGPL-2.1（`docs/third-party.md:28`） |
| `helpcodes/*.txt`、`custom_translations.txt` | 文本，`char=code`、`source<TAB>gloss` | 无 | 辅助码、释义 | 辅助码见下文：只有 `wubi86` 有来源说明，其余六张没有再分发授权；`custom_translations.txt` 未核 |
| 用户数据 `msime_user.db` | SQLite WAL，13 张表，`dictionary` 列取值冻结 | `CREATE IF NOT EXISTS`，`user_version=4` 只写不查 | 学习、个人词典、n-gram | 用户自己的数据 |

产出方：`msime-dict-build` 主构建出 pinyin/wubi/english/others/japanese/bigram/trigram 和清单，由 msime-dictionary 的 `dict-v*` 发布；`msime-dict-build languages` 出粤语、注音、笔画库；octagram 模型来自 amzxyz/rime-wanxiang-data（CC-BY-4.0）、lotem/rime-octagram-data（LGPL-3.0）、gaboolic/rime-frost（GPL-3.0），按 `resources/grammar-models.lock.json` 固定；神经模型来自 chinese-ime-lm `model-v1`；手写模型来自 msime-engine `a9b9f09`。

最小数据集（按 `prepare_runtime_paths_for` 只为用到的方案准备时，见 [回车学词策略进核心，代次准备按方案集合要求词库](../../implemented/architecture/2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)）：全拼/双拼/九键 = `msime-pinyin.db` + `msime-english.db`（产品同等排序再加 bigram/trigram）；五笔再加 `msime-wubi.db`；日文 = `msime-japanese.dat` + Mozc 的 README 与 LICENSE（缺了只出假名）；韩语、越南语、藏文什么也不用带；粤语/注音/笔画只要各自的库；这些非中文方案里 `msime-english.db` 只是专用英文的可选项。

文档化之前要修的缺口：代码引用的 `data-formats.md`、`quanpin.md`、`user-dictionary.md` 等规格文档不在仓库里；pinyin/wubi/others 三个库没有文件内版本，读端要像语言库一样加 `metadata` 表并检查；`.gram` 写明是 Rime 上游格式；`GLOSS_CACHE` 等死常量；只读布局（`dictionaries == resources`）没有写成受支持的模式；`assets.rs:1` 引用的运行时资源契约 `contracts/assets/assets.json` 不在仓库里；代次目录的 `.ready` 标记（`user_dictionary/generation.rs:124,145`）也是磁盘格式，要写进规格。

数据发布的许可证跟着数据走，不随引擎变，逐文件见上表的「数据许可证」列：默认中文库是 rime-ice 的 GPL-3.0；全拼、双拼、五笔要求的 `msime-english.db` 同样含 rime-ice-en 的 GPL-3.0（其余方案可以不带）；n-gram 计数来自 zhwiki（CC-BY-SA-4.0），词表来自 GPL-3.0 的 `msime-pinyin.db`；手写模型 LGPL-2.1，笔画库 LGPL-3.0，注音库 LGPL-2.1-or-later，五笔库混有 LGPL-3.0 与 GPL-3.0-or-later。`msime-others.db` 的 emoji 来源没有记录、PiliApp 符号未声明；辅助码表里 `lantian`、`ziranma`、`shouyou2_0`、`shouyouplus`、`xiaohe`、`jiajia` 六张都没有再分发授权（`resources/helpcodes/NOTICE.md:27`），核心默认的 `lantian` 和门面默认的 `ziranma` 都在其中，只有 `wubi86` 有自己的来源说明（`NOTICE-wubi86.md`）。没有授权的这些不能进数据发布；P3 统一默认值时，默认辅助码要么关掉，要么默认 `wubi86`，其余表由采用方自备。

### 与 librime 对照

照搬：`struct_size` 版本协商且只追加（Rime 的 `data_size`，改用定宽类型）；每种返回分配配一个释放函数；不透明的会话句柄（用类型化指针，不用 `uintptr_t` 加全局表）；按下标取候选；带 `void* user_data` 的通知回调，但事件用枚举、按实例注册；`version()` 返回静态字符串，头文件按字段标注引入版本；SONAME 的 major 长期不动；pkg-config 让前端按版本做编译期开关（fcitx5-rime 用 `rime>=1.9.0`）。

不照搬：部署/维护流水线（`deploy_*`、`start_maintenance`，为编译 YAML 方案而存在，数据构建留在离线的 dict-builder）；通用 `config_*` 树和方案列表 API；进程级 `setup`/`initialize`/`finalize` 和全局通知回调；以 X11 keysym 加修饰掩码作按键契约（引擎已有语义按键 `SchemeKey` 和 `Command`，键位映射是前端的事，与 libhangul、libchewing 一致）；静态构造器注册插件模块；`#define Bool int` 和没有大小头的冻结结构体；字符串选项名。

### crates.io 打包（P6）

- 发布顺序 `msime-path-trust` → `msime-engine` → `msime-engine-capi`；`crates/engine/Cargo.toml:11` 的路径依赖补 `version`。三个 crate 改为各自的 `version` 字段，不继承工作区的 `0.1.0`（`crates/engine/Cargo.toml:4`、`crates/path-trust/Cargo.toml:4`；先例 `crates/script-sdk/Cargo.toml:3`）。`msime-path-trust` 上 crates.io 之前，单独 `cargo publish -p msime-engine --dry-run` 解析不到它，要用多包形式 `cargo publish --dry-run -p msime-path-trust -p msime-engine -p msime-engine-capi`（据 Cargo 文档，未实测）。2026-10-29 查过 `msime-engine`、`msime-path-trust` 两个名字空着；`msime-engine-capi` 没查。
- 照 `crates/script-sdk/Cargo.toml:16` 用 `include` 白名单，不用 `exclude`（否则 `examples/` 的 21 个文件都会进包）；要排除的是 `tests/golden/**`（5.0 MB）、`src/**/testdata/**`、`src/pinyin/autocorrect_table_fixture.txt`（`jsfaint` 生成表的转储）和读仓库外资源的 examples。包内源码跑不了 `cargo test`（测试读仓库外文件），文档写明。估计 `.crate` 约 2.15 MB（`tar | gzip -9` 测得，不含测试），远低于 10 MB 上限。
- 公开面：`lib.rs` 把 `host`、`backend`、`ordering`、`web` 等产品模块都设为 `pub`（`crates/engine/src/lib.rs:7-47`），发布后都成了 crate 的 semver 面；0.x 期间 Rust API 不承诺稳定，产品模块加 `#[doc(hidden)]` 或放到 feature 后面。
- 工作区 `[profile]` 覆盖不随 crate 发布：`ewts` 的 `overflow-checks = false`（`Cargo.toml:131-136`，否则藏文 `bw`/`yw` 在 `converter.rs:136` 下溢 panic）和 `chinese-ime-lm` 的 dev `opt-level = 3`（`Cargo.toml:78-79`）对下游无效，第三方的 debug 构建会在这一键 panic、重排变慢。首发前要么等上游 emgyrz/ewts-rs#1 发版并升级，要么在引擎里绕开这个下溢；README 写明 dev 构建的重排延迟。capi 的二进制由我们构建，不受影响。
- `rusqlite` 开 `bundled`，`libsqlite3-sys` 声明 `links = "sqlite3"`：用别的 rusqlite 大版本的 Rust 下游不能同时依赖 msime-engine，写进 README。
- 补 `readme`、`keywords`、`categories`、`homepage`/`documentation` 和 README；`vi 0.8.0` 没有 SPDX 字段（`license-file` 是 MIT），许可证扫描要加说明。
- 每个 crate 的首个版本用令牌手动发布（Trusted Publishing 只能给已存在的 crate 配置），之后照 `release-script-sdk.yml` 的写法各配一个 Trusted Publisher 的手动工作流。capi 的二进制、头文件和 `.pc` 走 GitHub Releases。

### 分阶段计划

| 阶段 | 内容 | 退出条件 | 与 P0 的关系 |
|---|---|---|---|
| P0 收集同意书 | 已完成：2026-10-29 维护者确认全部同意（见「同意结果」）。剩余：各引擎 crate 的 README、CONTRIBUTING 与 PR 模板加入站双授权条款；补完上文未核实项；P2 前对最终树重跑 blame 并统计 `Co-authored-by` 尾行 | 维护者确认全部同意（已满足）；入站双授权条款随 P2 落地 | — |
| P1 无论结果都安全的清理 | rink 自写定义；latex `symbols.tsv` 进 crate；`syllable_frequency.tsv` 的声明行和 kHanyuPinlu 原型；octagram 与 cxx_sort 的出处；zhuyin 注释措辞；既有声明缺口 | `cargo test -p msime-engine local::units` 不改测试通过；`cargo tree -p msime-engine -e normal,features -i rink-core` 的输出不含 `bundle-files`；`rg -n 'include_(str\|bytes)!' crates/engine/src` 列出的每个非测试数据文件都在「其余内嵌表」里有许可证行，且没有一行是 GPL；非测试代码没有 crate 外的 `include_str!`；`docs/ARCHITECTURE.md:66` 写上第三份符号表副本；各声明测试通过 | 可并行 |
| P2 改许可证 | 上面的 P2 清单；`symbol_candidates.tsv` 经上游改头 | 清单全部勾掉，声明相关测试更新并通过 | P0 已完成；合并要等 P1 的 rink 自写定义与 latex 表进 crate 已合并、待拍板 1 和 5 已定论之后（否则 MPL 版本仍内嵌 GPL 数据，而发出去的版本收不回） |
| P3 引擎 API 加固 | 上面的硬点表 | 核心 `Session` 自己收尾；revision 在引擎；一套默认值；错误码；线程亲和按硬点表解决（`Send` 静态断言加跨线程 `reset_learning` 集成测试）；维护锁有归属；golden、input-runtime、host-api 测试通过 | 可并行 |
| P4 msime-engine-capi | crate、cbindgen 头、pkg-config、SONAME、C 示例、ABI 测试 | C 示例在 Linux、macOS、Windows 上分别静态和动态链接，用仓库内生成的最小 SQLite 夹具（几十个词条，建表 SQL 同 `session/tests.rs` 的夹具）打出一句全拼、选词、上屏、冲刷学习；头文件漂移检查进 CI；`docs/ARCHITECTURE.md` 分层图加上 `crates/engine-capi` | ABI 冻结依赖 P3 的 revision、核心收尾和线程结论；构建可并行，发布等 P2 |
| P5 数据格式文档与数据发布 | 规格文档；三个库加文件内版本与读端检查；按方案组装最小数据集，每个文件带许可证，排除无授权文件 | 运行时文件表的每一行都有规格；不兼容版本的库被拒绝而不是误读 | 可并行 |
| P6 发布到 crates.io | 打包元数据、手动首发、发布工作流 | `cargo publish --dry-run -p msime-path-trust -p msime-engine -p msime-engine-capi` 通过并完成首发 | 需要 P1（latex 表进 crate，否则打包校验找不到它）、P2（或决定以 GPL 发布）、P3、P4 |
| P7 msime 宿主自用 | input-runtime、host-api、engine-wasm、backend-engine 改用公开 API；engine-wasm 去掉自己的分页；dict-builder 是构建工具，可继续用内部接口 | 产品 crate 只用文档化的公开 API；`host` 门面只剩产品胶水 | P3 之后即可，可并行 |

### 维护者已拍板（2026-10-29，均按推荐）

1. `syllable_frequency.tsv` 算不算 rime-ice 的派生物。已定：先按聚合事实保留并写明来源，kHanyuPinlu 原型在九键评测不退步时替换。
2. 默认数据（中文库 rime-ice GPL-3.0、n-gram CC-BY-SA-4.0、全拼双拼五笔要求的 `msime-english.db` 含 rime-ice-en GPL-3.0）不能让 App Store 采用方直接带上。已定：v1 写明「引擎 MPL、数据按各自许可证」，由采用方选数据；宽松许可的最小数据集另立项目，前提是去掉非拼音方案对 `msime-english.db` 的硬性要求——这个前提已满足（[回车学词策略进核心，代次准备按方案集合要求词库](../../implemented/architecture/2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)）。
3. `fanlusky` 不同意时库还做不做。已无需决定：全部版权人已同意。
4. 构建工具用 cargo-c（一步生成 cbindgen 头、`.pc`、SONAME 和两种库）还是 cbindgen 加自写脚本，二者都是新的构建依赖。已定：cargo-c。
5. `symbol_groups.tsv` 的分组最初抄自 rime-prelude `symbols.yaml`（LGPL-3.0），这条链要不要按 LGPL-3.0 对待。已定：按 LGPL-3.0 对待，只用 Unicode 字符数据库的事实（区块、类别、字符名）重建分组，不再内嵌由 rime-prelude 派生的分组。

### 相关笔记（审计）

按 engine、MPL、crates.io、C ABI、移植、rink 检索活跃笔记：[脚本插件工具链](../../implemented/feature/2026-10-26-script-plugin-toolchain.md) 是 MIT 例外和 crates.io 首发流程的先例，部分重叠，互链；[脚本插件合集](../../implemented/architecture/2026-10-12-script-plugin-collection.md) 是 MIT 例外范围的先例，部分重叠；[九键音节频率表](../../implemented/bug-fix/2026-10-27-nine-key-syllable-frequency-table.md) 记录该表的生成与被否的数据包方案，本笔记只处理它的许可证，部分重叠；[宿主注入会话存储](2026-10-16-host-injected-session-storage.md) 讲 host-api 的账号 ABI，与引擎 ABI 无关。以下几篇也部分重叠，互链：[符号数据归词库仓](../../implemented/process/2026-10-25-symbol-data-in-dictionary-repo.md) 定下的副本规则是 P1 要扩展的；[符号模式](../../implemented/feature/2026-10-21-symbol-mode.md) 记录了 `symbols.yaml` 授权链的悬而未决；[内置拆字模式](../../implemented/feature/2026-10-13-builtin-chaizi-mode.md) 是 chaizi CC BY 问题的出处（`docs/third-party.md:296`）；[日文模型进程级缓存](../../implemented/architecture/2026-10-10-japanese-model-process-cache.md) 与「每进程一个实例」的进程级状态有关；[有符号单位换算](../../implemented/bug-fix/2026-10-16-signed-unit-conversion.md) 和 [单位目标分配预算](../../implemented/testing/2026-10-07-unit-target-allocation-budget.md) 拥有 P1 用作判据的 `units.rs` 测试。没有被本笔记吸收或取代的笔记。

## Alternatives considered

- **Rime 式框架，带方案 DSL。** 最强的理由是覆盖面：第三方能自己定义方案，不必等我们加。不用：`SchemeType` 是封闭的 10 变体枚举（`crates/engine/src/types.rs:102`），序号进了在线身份和宿主 ABI；`ProviderRegistry` 按方案硬编码字段（全拼、双拼、五笔、粤语、注音、笔画各有自己的字段）。改成 DSL 驱动等于做一个新产品，去和 Rime 已有的方案生态正面竞争，而我们的优势（词格、神经重排、纠错、九键）恰好是 DSL 表达不了的。定制需求今后由通用码表方案（要新做）、`ShuangpinCustomTable` 和脚本插件（GPL 的桌面运行时，不在库内）覆盖，v1 对第三方只承诺 `ShuangpinCustomTable` 和个人词典。
- **把引擎搬到独立仓库。** 最强的理由是外部贡献者和发布节奏与 GUI 宿主分开，许可证边界一目了然。不用：ARCHITECTURE.md「上游 Engine」一节记录了引擎是有意搬进本仓库、去掉锁文件和 overlay 这一层的；引擎改动现在和宿主改动在同一个 PR 里原子落地，拆仓库会把这层重新带回来。MIT 例外的先例表明，同仓库内按 crate 分许可证是可行的。
- **先把 msime-engine 拆成多个内部 crate。** 最强的理由是对外 API 面更小、各部分可以独立发布。不用：模块之间有环，`ime` 是所有方案的枢纽，方案又反过来依赖它，`session` 和 `user_dictionary` 互相引用；拆 crate 要先解环，工作量大，而第三方要的是一个稳定的 C ABI 和一个 crate，不是内部结构。
- **库保持 GPL-3.0-only。** 最强的理由是不需要任何人签字，今天就能发布。不用：它挡住 App Store 应用和所有非 GPL 的采用方，而这正是做库的目的；librime 是 BSD-3。作为 `fanlusky` 等人不同意时的退路保留。
- **扩展现有 host-api 的 C ABI（`msime_client.h`），不另起 capi。** 最强的理由是它已有、六个平台在用，句柄注册表、`catch_unwind`、重入保护都做好了。不用：它是 GPL 的客户端门面，混着账号、云、语音、皮肤和遥测；每次按键走 JSON；错误只有字符串；「ABI 3、只追加」没有功能探测；严格线程绑定。它的好做法（句柄、`catch_unwind`、重入检查、单一释放函数、带 generation 的选择）搬进 capi。

## Acceptance criteria

- 版权人同意：2026-10-29 维护者确认全部同意，记录由维护者留存。
- `cargo tree -p msime-engine -e normal,features -i rink-core` 不含 `bundle-files`，`include_str!`/`include_bytes!` 引入的每个非测试数据文件都有非 GPL 的许可证行；非测试代码没有 crate 外的 `include_str!`；`cargo package -p msime-engine --list` 不含 `tests/golden`、`testdata` 和自动纠错夹具。
- P2 清单全部勾掉，`collect_notices.ps1`、`bundle_contents.py`、`test-symbol-groups.py`、`test-language-data-notices.py` 和 engine-wasm 的声明测试通过。
- `msime-path-trust`、`msime-engine`、`msime-engine-capi` 的 `cargo publish --dry-run` 通过并完成首发；capi 的 C 示例在三个桌面平台上用安装后的头文件和 `.pc` 链接并能打字、选词、上屏。
- 运行时文件表的每一行都有规格文档，三个主库在读端检查文件内版本。
- msime 的产品 crate（input-runtime、host-api、engine-wasm、backend-engine）只经文档化的公开 API 使用引擎；dict-builder 例外。

## Risks

- 同意书可能收不齐（例如 `EwigMidori`、`ManwoodIsolation` 联系不上），P2 无限期阻塞；P1、P3 到 P5、P7 不依赖它，照样有用。
- 多处判断只是技术比对、没有经过律师审阅：`syllable_frequency.tsv` 是否算事实、CC BY 数据放在 MPL 库里、`symbol_groups.tsv` 经 rime-prelude `symbols.yaml` 的 LGPL-3.0 链、`hanja.tsv` 里整理自国立国语院材料的读音能否以 BSD 再授权、`ouchihao` 与 `PathGao` 的小量代码是否构成作品、`zhuyin/` 是否独立。任何一项被推翻，名单或数据处理就要变。
- 移植来源不止 `metasequoiaime/msime-engine`，还有 msime-windows（octagram、自动纠错生成器、`QU:`/`TP:` 引用的运行时代码），后者没有逐行归属，名单可能还缺人（HowGeGe、chr331、沐雪清泽等，见上文未核实项）。
- rink 自写定义的值必须按法定值写；输出逐字节不变没有编译验证，单位测试是判据。
- P3 动的是会话收尾和学习冲刷，回归会表现为产品丢学习数据；golden、input-runtime 和 host-api 测试要全跑。
- ABI 一旦标为稳定，命令和枚举编号就冻结。核心层和门面的命令编号不同；新 ABI 用核心编号，host-api 的 ABI 3 编号（如 `msime_client.h:912` 的 `MSIME_OPEN_CANDIDATE_LIST = 16`）不变、继续按名字映射，平台宿主无需迁移，改它反而破坏 ABI 3 的只追加。
- 回滚：P2 之前任何阶段都可单独回退；P2 发布后已发出的 MPL 版本无法收回，只能停止后续 MPL 发布。
