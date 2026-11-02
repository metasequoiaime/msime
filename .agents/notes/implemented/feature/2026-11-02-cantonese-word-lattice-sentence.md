# Agent Note: 粤语词图组句与粤语双拼展开预编辑

Status: implemented

## Problem

#10557 报告粤语双拼没有整句：整段读音不是 `msime-cantonese.db` 里的一个词时，候选只有开头的单字或短词，要一段一段地选（`vodanz` 的「我打你」不在库里）。普通粤拼也一样。报告者还希望粤语双拼的组字区像普通话双拼那样显示展开后的读音。普通话整句靠的是按普通话拼音和语料训练的 n-gram 与重排模型，不能拿来给粤语用；[粤语整词与整句的分阶段评估](../../proposed/feature/2026-10-30-cantonese-word-sentence-evaluation.md)把「有界的粤语词图组句」列为路线 2，n-gram 留作独立实验。

## Decision

`CantoneseScheme::candidates_into` 在原有候选之外插入一行整句，粤拼和粤语双拼共用。

- 位置：双拼时每个完整两键码一个位置，读音是 `shuangpin::decode` 在库存里解出的全部音节；普通粤拼时是 `syllable::best` 切出的完整音节，每个一个读音。末尾单独的声母键和还没拼完的音节不参与。
- 转换：复用注音的 `zhuyin::conversion::convert_filtered`（可见性从 `pub(super)` 放宽到 `pub(crate)`，算法不动）。每个区间取 `CantoneseLexicon::lookup_best_reading` 的最重词条：发布库 `LanguageDictionary::lookup_readings(.., 1)` 的那一条和用户在个人词库加的粤语词（`UserWords`，#10594）按同一规则比（权重从重到轻，再按文字、键），同一个键下的同一个词取较大的权重，与候选各段的合并规则一致。用户加的词因此和发布词一样能拼进整句，也同样受下面的多读音下限约束。路径分数先比各段长度平方和，再比权重和。
- 何时不出：位置少于 2 个或多于 `SENTENCE_MAX_SYLLABLES`（20，与注音上限相同）；已有候选行正好覆盖全部位置（整段本来就是词条，它已排在前面）；结果只有一段；或有位置连单字都查不到，兜底段的文字是拉丁读音本身。
- 排序：整句插在第一行「覆盖音节数不超过它」的行前面。末尾单键或末尾半个音节补全出的行覆盖更多音节，仍在它前面，连续打字时首选不在整句和补全之间来回跳。
- 多读音下限：经过多读音双拼码的多音节词，权重乘 `SENTENCE_AMBIGUOUS_WORD_FLOOR`（1000）要不低于区间内最弱的单字权重，做法与注音九键的 `AMBIGUOUS_WORD_FLOOR` 相同。普通粤拼每个位置只有一个读音，不受影响。
- 缓存：`CantoneseQueryBuffer.best` 按区间各位置的读音缓存最重词条，连续打字时只有以新音节结尾的区间要查；条目超过 2048 时整体清空。发布词库在会话内只读，用户的粤语词却会在会话中途变（`UserWords::refresh` 读到设置页或个人词库队列提交的增删），所以 `UserWords` 每次读到不同的词就把 `generation` 加一，`CantoneseLexicon` 带着它，缓存记下填它时的版本（`best_generation`），对不上就整体清空。只看日志的 `data_version` 不够细：别的种类的词改了也会让它变，那时粤语词没变，缓存照留。
- 整句行的 `key` 是各段所用读音以空格连接，`weight` 是各段权重之和，`end` 是最后一个位置的结束字节。选中它沿用 `select_end`，一次上屏，提交路径不变。粤语不学习，上屏不写用户词库；用户手动加的粤语词只读不写。

预编辑展开跟随已有的「双拼显示原始按键」（`shuangpin_preedit_uses_raw`，默认仍是原始按键）。关掉时，宿主快照（`display_snapshot` / `snapshot_for_host`）的 `preedit` 改为 `CantoneseScheme::jyutping_display` 的结果：首选候选覆盖的完整码按它用的读音写，其余的码取库存里解出的第一个读音，末尾单独的声母键和 `'` 原样保留（`vo'danzd` → `ngo' daa nei d`）。`editing_text`、`normalized_segmentation` 和 `Session::snapshot` 的 `preedit` 都不变，因为粤拼的编辑文字会被 `set_raw_input` 读回成原始输入，换成粤拼就和按键对不上了。光标后编辑时不展开。不新增偏好，各平台的设置入口见下一节。

## 设置入口

偏好只有一个，出厂值不变，但每个平台都要在用粤语双拼时找得到它。原先只有 iOS 做到了：桌面共享设置页和 Windows 原生设置只在普通话双拼方案下画这一行，Android 的宿主能力 `shuangpin_preedit` 不含 Android、原生设置页也没有这一项。现在的入口：

- 桌面共享设置页（没有触屏方案的宿主，如 macOS 和 Linux）：`InputSchemeDetailsSection` 在 `scheme === "cantonese"` 且 `resolveCantoneseInputMethod` 为 `shuangpin` 时，在「粤拼方案」之后给出「双拼预编辑」，选项用 `cantonese_shuangpin` 模式，写成「原始按键 / 粤拼分词」，说明写明与普通话双拼共用。普通粤拼不画：它打的就是粤拼，没有按键要展开。双拼方案下的那一行不变，两行不会同时出现。
- Windows 原生设置：WinUI 设置（`main.cpp` 的 `build_scheme_group`）和 Windows 7 设置（`SchemeForm` 的 `Section::cantonese_shuangpin`，控件 `scheme_cantonese_preedit`）都在选了粤语双拼时给出同一行。
- Android：`HostCapabilities::shuangpin_preedit` 改为六个宿主都为真。键盘页「布局」组在「拼音显示位置」之后加「双拼显示原始按键」，标题与 iOS 相同，说明只讲输入框：Android 写进输入框的是 `preedit`，候选栏画的是读音（`display_reading`），不受这一项影响。本版本有双拼或粤拼时才列出。
- 触屏宿主和鸿蒙手机：共享页 `PreeditSettingsSection` 的这一行（鸿蒙手机在「候选栏 › 更多选项」里）说明改为「仅在双拼和粤语双拼下生效」。iOS 候选栏页原样保留。

不是每个宿主都在同一处画出展开结果。macOS、Windows、Linux 的候选窗和行内组字画 `preedit`；Android、iOS 和鸿蒙把 `preedit` 写进输入框。鸿蒙的组字栏画的是 `editing_text`（粤拼的 `SchemeCompositionPolicy.reading` 返回编辑文字），所以那里仍显示原始按键。要不要在组字栏也画展开，属于显示设计，本篇不改。Linux 原生状态菜单（IBus、Fcitx5）里的同名开关仍只在普通话双拼下出现，共享设置页已经能改，菜单本篇不动。

## 合成回归集与评估

没有母语者审校的开发集。`shipped_dictionary_leave_one_out_sentences`（`#[ignore]`，用 `MSIME_TEST_CANTONESE_DICTIONARY` 指定发布词库，`--release` 运行）用 dict-v2.0.48 的 `msime-cantonese.db`（SHA-256 `be2412585eae0128c3a0905fb08a0c053e846da8813d1cbb3151277514e15ac4`）建三组合成样例。每组按 top-1 统计，也就是首选能否还原原文：

| 样例组 | 条数 | 粤语双拼首选还原 | 普通粤拼首选还原 |
| --- | ---: | ---: | ---: |
| 留一·常用：权重最高的 300 个 3 到 6 音节词条，先从词库副本删掉 | 300 | 99 | 118 |
| 留一·等距：其余 3 到 6 音节词条里等距取 300 个，同样先删掉 | 300 | 47 | 65 |
| 词序列：常用双音节词两两、三三相接（接起来的读音本身不是词条） | 500 | 490 | 452 |

留一组衡量的是把一个已收录的固定说法拆开后能不能拼回原样，是最难的一类。失败多是同音单字（嗰陣時 → 個陣時）、双拼码的另一读音带来更重的单字（屋企人 → 屋企一、人民幣 → 一密閉），以及只有上下文才能区分的说法。这类错误需要语言模型才能解决，本次没有处理。词序列组更接近「已有词拼成句子」的日常情形。普通粤拼不加分隔符连打时，`syllable::best` 的最长匹配有时切错音节，这是既有的切分行为，本篇不改。

多读音下限对双拼整句还原的影响（常用 / 等距 / 词序列）：

| 下限倍数 | 1 | 10 | 100 | 1000 | 10000 | 不设 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 还原条数 | 57 / 15 / 121 | 82 / 24 / 392 | 100 / 43 / 493 | 99 / 47 / 490 | 100 / 47 / 490 | 101 / 49 / 490 |

1000 与不设下限相比，三组分别少 2、2、0 条，都在 1% 以内，属于噪声范围。保留 1000 是为了和注音一致，并挡住这些样例没覆盖到的冷僻词凑巧压过常用单字的情形（单元测试 `a_rare_word_through_an_ambiguous_code_does_not_win_the_sentence`）。合成样例都取自词库里最重的词条，冷僻词插入的情形覆盖不到，这个取值仍需母语者开发集复核。

## 按键耗时

同一测试逐键打 40 组 16 个双拼码（988 次按键），每次按键都像会话那样整段重查，在 Mac Studio 上用 release 构建测得：

| 情形 | p50 | p95 | p99 | 最大 |
| --- | ---: | ---: | ---: | ---: |
| 组句，缓存跨按键保留（会话里的实际情形） | 1.66 ms | 10.89 ms | 13.73 ms | 16.10 ms |
| 组句，每键新缓存 | 2.53 ms | 14.47 ms | 17.17 ms | 21.80 ms |
| 不组句的对照 | 1.41 ms | 10.34 ms | 13.18 ms | 16.07 ms |

组句在 p50 上增加约 0.25 ms，在 p95 上增加约 0.55 ms。对照组的 p95 已接近 10 ms，主要来自既有的双拼展开：最多 64 种读音组合，每种都要把各前缀段查一遍。这不是本次引入的开销，需要单独处理，并在移动设备上实测。

## Alternatives considered

- **只扩充人工审校词表（路线 1）**：每条都是确认过的读音和词面，不会组出怪句；但它只能覆盖收录的说法，「我打你」这类任意组合永远差一条。它和组句并不冲突，照旧走 msime-dictionary 的 `custom/cantonese.txt`。
- **直接上粤语 n-gram 或整句模型**：能区分同音字和上下文，是留一组失败的根本解法；现有 HKCanCor 词频表已丢句序，无法训练 bigram，需要另立实验并定下体积和设备延迟预算（见评估笔记的路线 3）。
- **路径分数的权重部分改用对数权重之和**：单字权重是数百万量级，多音节词是数千到数万，权重和会偏向含重单字的切分。试过把每段权重换成 `ln(weight)`，常用组还原从 99 升到 104，等距组和词序列组不变。提升小，而且会让粤语和注音的评分规则不一致，因此没有采用。
- **整句另设开关、或预编辑展开另设开关**：维护者决定组句直接启用，预编辑复用已有偏好，不增加设置项。
- **把「双拼预编辑」放回不分方案的位置（桌面的候选窗口页）**：一处入口就能覆盖普通话双拼和粤语双拼，不用在两个方案下各画一行。但这一行当初就是从候选窗口页挪进双拼详情的：放在那里时全拼、五笔用户也看得到一个对他们不起作用的选项。所以保留按方案出现，在粤语双拼下多画一行，写的是同一个偏好。

## Consequences

- 收益：库里已有的词能拼成整句放在首位，「我打你」「周杰倫」这类整段不在库里的输入一次上屏（#10369 原先要分两次选）。粤语双拼可以显示展开后的粤拼，宿主的显示代码不用改；六个平台在用粤语双拼时都能在设置里找到这一项（见「设置入口」）。
- 代价：整句不对时，原先排第一的开头单字或短词退到第二位，空格会上屏错的整句。这会改变普通粤拼和粤语双拼用户的首位候选，属于默认行为变化，已经维护者批准。每次按键多约 0.25 ms（p50）。
- 验证缺口：没有母语者开发集和独立盲测集，评估笔记建议的「至少 200 个由母语者固定读音与可接受候选的样例」仍未建立。真机上的首候选延迟和内存没有测，标记文本显示粤拼后在第三方 App 里的删除、移动光标和回车也没有在真机上验证。Android 键盘页新开关、Windows 两种设置窗口的新行都没有在设备上点过，只有源码守卫和 Windows 7 设置的 UI 测试（要在 Windows 上跑）。
- 测试：`cantonese::scheme` 单元测试覆盖双拼与普通粤拼组句、排序、兜底、上限、多读音下限和展开文字；`session::tests::cantonese_shuangpin_sentence_commits_whole_and_preedit_follows_the_raw_key_preference` 覆盖整句一次上屏，以及两种偏好下的显示快照、编辑文字和原始快照。设置入口由 `input-scheme-details-section.test.tsx`、`preedit-settings-section.test.tsx`、`settings.test.tsx`（macOS 粤语双拼保存、鸿蒙手机候选栏页）、`host_surface::tests`（六个宿主都有 `shuangpin_preedit`）、`windows7_settings.cpp`、Android `check-host.sh` 的源码守卫和 `SettingsSearchIndexSmoke` 覆盖。`user_words_take_part_in_the_composed_sentence` 覆盖用户的多音节词拼进整句、单字词与发布的同字取较大权重；`sentence_cache_follows_user_word_changes` 用同一个会话缓冲覆盖加词、删光词后缓存作废，以及别的种类改词时缓存保留（去掉版本比较时这条测试失败）。

## Related notes audit

- [粤语个人词条](2026-11-02-cantonese-personal-words.md)：部分重叠。它定下 `UserWords` 与 `CantoneseLexicon` 的读取和合并规则，本篇组句沿用同一规则取最重词条，并给 `UserWords` 加了内容版本，供组句缓存作废。
- [粤语整词与整句的分阶段评估](../../proposed/feature/2026-10-30-cantonese-word-sentence-evaluation.md)：部分落实。本篇实现路线 2，路线 1（审校词表扩充）和路线 3（n-gram）仍是提案，母语者开发集的门槛也没有满足，所以那一篇保持 proposed，并加上指向本篇的链接。
- [粤语双拼两键定音节](../bug-fix/2026-10-29-cantonese-shuangpin-strict-expansion.md)：部分重叠。它持有展开、分段和候选排序规则，本篇沿用这些规则，只多插入一行整句，不改变它的决定。
- [粤语双拼](2026-10-26-cantonese-shuangpin.md)：部分重叠。编辑文字仍是两键一组（`nz hb`），本篇只给宿主显示另加展开文字。
- [iOS 部分上屏直接替换标记区](../bug-fix/2026-10-31-ios-cantonese-partial-commit.md)：无关。整句一次上屏走同一条替换路径，宿主代码不变。
- [预编辑偏好类型](../simplification/2026-10-15-preedit-preferences.md)：无关。它管设置界面的类型归属，本篇不改偏好的格式和默认值。
- [Windows 组字细节对齐 macOS](2026-10-10-windows-input-compose-mac-parity.md)：部分重叠。它给 Windows 加上 `shuangpin_preedit` 能力和双拼方案下的这一行，本篇在粤语双拼下多给一行，不改它的决定。
- [触屏把拼写写进输入框](2026-10-15-touch-composition-in-field.md)：部分重叠。它让触屏宿主把 `preedit` 写进输入框、共享页在触屏上只留「双拼预编辑」，Android 的新开关正是控制写进去的这段文字。
