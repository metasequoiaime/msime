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

## Alternatives considered

- **继续锁 `dict-v2.0.24`，靠自动更新把新词库送到用户手里。** 不动锁最省事，随包基线也最保守：自动更新出问题时用户回到一份评测过的词库。但评测、`verify-local.sh` 和依赖真实词库的测试会继续量一份没人在用的词库，九键那次回归正是这样只在真机上才暴露；锁越久越远，下一次不得不换时要一次吞下的差异越大。所以不采用。
- **CI 与评测自动跟随最新发布，不再手工换锁。** 能保证测量对象永远等于设备上的词库，也省掉换锁这一步。但评测基线是提交进仓库的数字，跟随最新发布意味着任何一次词库发布都会让与之无关的 PR 的评测门禁对不上，而且对不上的原因不在那个 PR 里；随包基线和资源代次也会随构建时间变化，同一个提交两次构建出两种安装包，`edition.rs` 钉住的代次和锁派生关系都失去意义。可复现性比时效性更重要，所以仍然手工换锁，只是把换锁当作词库发布后的例行一步，而不是只在补词时才做。
- **锁 `dict-v2.0.28`，即五笔码表变大之前的最后一个发布。** 能避开下面说的安装包体积门槛，但设备上早已是 `dict-v2.0.3x` 的五笔码表和权重，评测仍然量不到用户手上的东西，问题没解决，所以不采用。

## Consequences

- **收益**：评测基线、`verify-local.sh` 和 `MSIME_EVAL_RESOURCES` 下的真实词库测试量的是设备经自动更新已经在用的词库；新装用户和自动更新不可用的用户拿到的随包词库与其他人一致。
- **代价**：基线数字整体下降（见上表），这是词库数据带来的，记录在案而不是被掩盖；下一个动排序的改动要以新基线为准，包括正在 `dict-v2.0.39` 上做九键实验的 `feat/nine-key-word-ranking` 等分支。
- **安装包体积增长需要分别验证**。macOS 已按本版本锁计算预算，Android 门槛与实际压缩体积仍待处理：
  - Android 的 full 与拼音瘦包（`platforms/android/build-apk.sh`，`MSIME_ANDROID_MAX_APK_BYTES` 默认 135,000,000）带着五笔码表。`android-v0.6.0` 的 full 瘦包已是 133,833,900 字节；`msime-wubi.db` 用 gzip -6 估算从约 7.4 MB 涨到约 44.9 MB，拼音库约多 0.4 MB，瘦包估计约 171.7 MB，超出门槛约 37 MB。2026-10-26 刚为守住这个门槛把读音表改成资源包（[英文音标候选](../feature/2026-10-26-android-candidate-pronunciation.md)），直接抬门槛与那次的取舍相反；另一条路是像 macOS 一样把五笔码表改成按需资源包（[macOS 五笔按需下载](../feature/2026-10-20-macos-on-demand-voice-runtime-and-wubi.md)）。
  - macOS 五笔版随包带五笔码表。`package-release.sh` 的 `EngineResources` 预算现在按本版本锁中实际随包的条目逐文件向上取整到 4 KiB，加仓库随附辅助码表，再留固定 4 MiB 余量；完整性校验仍先执行，按需资源排除检查不变。新锁的五笔版预算为 198880 KiB，full 与拼音版为 110508 KiB，不再让五笔版卡在旧的 126000 KiB 固定值。资源增长由锁文件改动显式评审，额外未锁定文件不会抬高预算。轻量回归执行真实预算函数，覆盖五笔版与省略五笔的 full；尚未实际打完整 DMG。
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
