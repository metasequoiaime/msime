# Agent Note: 打拼音出符号：整段拼出符号名称时混入符号候选

Status: implemented

## Problem

八条 issue 点名要「打拼音出符号」：W#40（`zhengfu` 出 ±，`pai` 出 π，`aerfa` 出 α）、W#636（`pingmi` 出 ㎡，要能开关）、W#759（`lifang` 出 m³，评论者用 K 模式短语自己拼了一张码表）、W#777（`gamma` 出 γ Γ，要靠前）、W#795（希腊字母、几何和数学符号）、W#909、W#736（关了 emoji 但想「通过词语打出符号」，比如大于、小于）、W#33 第一条。W#758、W#842 补了位置上的约束：符号不能抢首位，不能因候选记忆挤掉汉字。

现状下唯一能出符号的路是 `msime-others.db` 里 `emoji.txt` 追加的 `Symbols` 类行（发布版里 `renminbi` 的 ¥ 就是这样来的），但它受 emoji 混输开关控制（Linux、Android、iOS、HarmonyOS 默认关），按前缀匹配，还会出现在表情面板里；改一行要发新的 `dict-v*` 词库。

## Decision

- **数据编进引擎。**（源数据后来迁到 msime-dictionary 的 `sources/symbols/symbol-names.tsv`，这里的文件是按锁同步的副本，见[补齐希腊字母与数学符号名称](2026-10-25-math-greek-symbol-names.md)。）本项目自写的 `crates/engine/src/local/symbol_candidates.tsv`（`名称<TAB>拼音<TAB>符号...`，约 300 行，按数学、单位、希腊字母、形状与记号、标点与括号、其他、数字序号分组）由 `crates/engine/src/local/symbol_candidates.rs` 用 `include_str!` 编入，第一次查询时在 `OnceLock` 里建三个索引：全拼（去掉 `'`）、两个音节以上非数字中文名称的首字母、拉丁名称。拼音列取 `msime-pinyin.db` 里那个词的键（圆周率是 `yuan'zhou'lv`），同一个拼音有几个名称时词库里有的写在前面。
- **只认整段完全相等，不按前缀。** 26 键（`query_mixed_symbols`）：全拼认名称的全拼、两个音节以上非数字名称的首字母（`ssd` 摄氏度，只限全拼）和至少 4 个字母的拉丁名称（`gamma`）；双拼用 `normalize_input` 归一成全拼后比全拼，另按原样比拉丁名称。微软双拼的 `;` 是 `ing` 韵母键，在归一化前保留（`p;mi` 读作 `pingmi`），只在方案实际使用分号键时接受；全拼和其他双拼方案仍拒绝分号。九宫格（`query_symbol_readings`）只认读法与名称全拼完全相等，并且要过 `splits_into_syllables_at` 的边界检查。输入不到 `MIXED_EXPRESSIVE_MINIMUM_INPUT`（2）键不查。单音节名称只收派、度及一至十，依据分别为 W#40、W#759 和 #9088，由表检查钉死。
- **来源复用 `CandidateSource::Emoji`（6）。** 于是不学习、不调频、不进个人上下文（`learn_candidate`、`record_personal_context`）、选中即整段上屏结束组字（`commit_selection`，所以部分选词后选符号不会造出「大于≥」），不能置顶、固定或删除（`is_editable_source`），重排器不碰它（`CATALOG_SOURCES`），有云候选、AI 候选时按 emoji 的座位坐（`normalize_online_slots`），不请求释义，Windows 选中即结束组字。`CandidateSource::Emoji` 的文档注释写明它也用于符号行。
- **位置**（`crates/engine/src/ime/queries.rs` 的 `place_symbol_rows`，26 键和九宫格的 `insert_expressive_rows` 共用）：
  - 锚定词是名称本身（中文名称那个词，拉丁名称是英文混输给出的同名英文行）。在候选里时紧跟第一个锚定词；输入含糊时最早排在下标 `SYMBOL_AMBIGUOUS_SLOT`（3，第 4 位），含糊指单音节名称（派、度及一至十）和两个字母的首字母（`ax` 爱心、`jf` 积分、`oy` 欧元、`bq` 版权）。两个字母的简拼和单音节一样能指的词太多：真词库上它们原本都把符号排在下标 1，挤掉第二个常用词。三个字母以上的首字母（`ssd`、`wjx`、`lfm`，issue 里举的例子都是这种）照旧紧跟锚定词。没选「只认三个字母以上的首字母」：W#759 点名要 `pm` 出 ㎡，那样要给它单独开口子。
  - 几个名称给出同一个符号时（`dg` 命中对钩、对勾、打钩、打勾，都给 √ ✓；`xigema` 的西格马、西格玛），符号只出一行，但每个名称都记作锚定词（`SymbolRow::anchors`），符号跟在其中在列表里排得最靠前的那个后面。此前只记表里第一个名称：`dg` 的 √ 跟着排在第 288 位的对钩，而不是第 87 位的打钩；表里靠前的名称不在候选里时，首字母匹配的符号整个消失。
  - 锚定词不在候选里时，全拼两个音节以上和拉丁名称排在下标 `SYMBOL_UNANCHORED_SLOT`（5，默认每页 6 个的第一页最后一位），列表短时接在末尾；首字母匹配和单音节不出。
  - 所有位置 ≥ 1，列表空时不加，所以混排时符号不占首位。
  - 符号在 emoji 之后插，同一个词后面依次是符号、emoji、颜文字；固定位置照常在混排之后应用。
  - 混排之后名称词还会被挪走：只出单字筛掉多字的名称词（`zhengfu` 剩下 ± 正 政），固定位置把名称词挪到后面（「大于」固定在第 3 位时 > ＞ 顶到前面）。所以列表定型后再过一道 `keep_expressive_rows_off_the_lead`（26 键在 `InputSession::mixed_from` 的单字筛选之后，九宫格在固定位置之后）：首位是 emoji、颜文字或符号行时，把后面第一个没有固定位置、也不坐在线座位的词提到首位，这些行各往后挪一位，夹在中间的固定词和云候选、AI 行原地不动。这一步对 emoji 和颜文字同样生效，它们以前也有同样的漏洞，只是 emoji 在多数平台默认关，暴露得少。没选整体改成「先固定位置、后混排」：英文行可以固定位置，要把 `CandidateQueries::mixed` 拆成两段，emoji 的锚定规则也跟着变，超出这次的范围。每次最多 `MIXED_SYMBOL_LIMIT`（8）行，一个名称最多 6 个符号。
  - 去重：符号行按文字与候选、英文去重，排在 emoji 之前进去重表，所以与它文字相同的 emoji 行被去掉；文字只差 U+FE0F 的 emoji 行（✔️ 对 ✔）也去掉，留下文本样式的符号。`MIXED_DEDUP_CAPACITY` 加上 `MIXED_SYMBOL_LIMIT`。
- **门槛。** 只有全拼、双拼（`allows_english_emoji_mixing`）；本地模式、专用英文时 `CandidateQueries::mixed` 原样返回；会话层在光标前缀解码和辅助码激活时传 `symbols_allowed = false`（`InputSession::mixed_from`）；九宫格在单字、笔画筛选时不查。五笔不混排，四码唯一自动上屏（`wubi_unique_four_code`）不受影响。
- **开关与默认值。** 新偏好 `mixed_input.symbols`（`MixedInputPreferences::symbols`），所有宿主默认开，`#[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]`：缺键的旧文档读作开，只有用户关掉时文档里才出现 `"symbols": false`。它与 emoji 开关独立（W#736 就是关了 emoji 想要符号的人）。host-api 在 `apply_pending` 和 `into_engine_options` 两处映射到 `EngineOptions::mixed_symbols`，引擎层默认关（`MixedExpressiveOptions::symbol_candidates`），所以 golden 基准、`convert_eval`、网页引擎（`MixedExpressiveOptions::default()`）都不变。
- **各端开关**，各端读到缺键一律按开：共享设置页「符号混输」（`packages/ui/src/settings/mixed-input-preferences.ts` 的 `mixedInputControlState` 按 `symbols ?? true` 读取，由 `mixed-input-section.tsx` 绘制，Linux 设置窗口和 HarmonyOS 设置页都用它）、Windows 原生设置（`bool_row(..., L"mixed_input.symbols", true)`）、macOS 原生设置（`mixedSymbolInput`，回退 `YES`）、iOS 候选设置（`symbolsEnabled` 缺键读作开）、Android 表达页「候选带符号」（`optBoolean("symbols", true)`，`mixedInput()` 不补这个可选字段）、Linux IBus 属性 `SymbolCandidates`（「符号候选」）和 Fcitx5 状态动作 `-mixed-symbols`（「混合符号」）。

### 写回整个 `mixed_input` 对象的宿主

macOS 的 `sharedPreferencesByMerging` 整对象重写 `mixed_input`，所以 `symbols` 必须带上（只在关掉时写 `false`，与 client-core 一致），否则在共享页关掉后，到原生页改一次 emoji 就会把它冲回开。原生页的四个混输 setter 改为从当前生效的值（有共享快照时是快照）出发再改一项，此前它们只从本机 NSUserDefaults 出发，共享页改过、本机没存的项会在原生页改一次别的开关后退回默认值。Fcitx5 读开关状态的两处回退原来写死 `false`，改成按键给（`mixedCandidateDefault`，`symbols` 为开），否则缺键的符号混输会显示成关、第一次点击把它「打开」成关。

### 真词库探针

`crates/engine/examples/symbol_candidates_dictionary.rs` 对表里每个名称打全拼（两个音节以上的非数字名称另打首字母，拉丁名称打原文），断言首位不是符号行、该出现时出现，并列出锚定词不在前 20 个候选里的名称。在 `dict-v2.0.14` 的资源上：`zhengfu` 是「政府 征服 正副 正负 ± 整幅」，`pai` 是「派 排 拍 π」，`du` 是「度 独 毒 °」，`lifang` 是「立方 m³ ³」，`pingmi` 是「平米 ㎡ m²」，`pm` 在第 4 位的「平米」后面，`gamma` 是「gamma γ Γ 伽马」（英文混输开着，默认触发数 5）。锚定词不在前 20 位的有因为号、所以号、卡方、西格玛、欧米伽、实心圆、空心圆、对勾、打勾、双直角引号、章节号和拉丁的 beta、zeta、iota，它们只能在第 6 位出现（`iota` 拆不出拼音，列表为空，不出；ι 由「约塔」给）。

## Alternatives considered

- **往 `emoji.txt` 加行（SELECTION.md 的建议）。** 最强理由：零代码，资源实测追加行确实进了 `emoji_pinyin`，打字时能出来。不用：受 emoji 开关控制，四个平台默认关，W#736 这种关了 emoji 的人拿不到；追加行进 `emoji` 表的 `Symbols` 类，表情面板会显示它们（W#40 维护者说要避免）；emoji 按前缀取，打 `zheng` 也取回 ±，接不上词就进末尾，给常用音节加噪音；每改一行都要发 `dict-v*`、改锁文件，已装的旧词库不受益。
- **在 `msime-others.db` 加独立的 `symbol_pinyin` 表（W#40 维护者的方向二）。** 最强理由：语义干净，不进表情面板。不用：仍要发词库、改锁文件，资源缺表时还要降级；表只有几百行，放进 SQLite 换不来什么。
- **新增 `CandidateSource::Symbol = 14`。** 最强理由：语义准确，日志里不叫 Emoji。不用：线上值是冻结契约，新值要同时改 `from_u8`、`CATALOG_SOURCES`、`normalize_online_slots` 的 `is_local`、Windows `candidate_finishes_composition`、Android 释义跳过、client-core 和 host-api 的释义判断，漏一处就是只在某个平台出现的组字残留。`SymbolRow` 在引擎里单独存在，真要区分时换来源的代价和今天一样。
- **快捷短语（K 模式）或可编辑短语混进候选（W#636 建议二、W#759 评论者的做法）。** 最强理由：用户能自定义。不用：K 模式短语今天只在 `LocalInputMode::QuickPhrase` 里查，「短语混进普通候选」要另做设计，范围大得多；内置表解决开箱即用，两者不冲突。
- **前缀模式（W#777 评论者提议，类似微软的 `u` 模式）。** 最强理由：不占普通候选的位置。不用：八条直接请求里六条要的是普通打字时出现，前缀模式解决不了「不知道有这个功能」。
- **部分拼写也出（按前缀匹配，像 emoji 那样）。** 最强理由：少打几个字母。不用：表是封闭的小集合，前缀命中几乎都是噪音，emoji 用前缀是为了靠目录关键词接候选词，符号不需要。
- **默认关。** 最强理由：升级用户不会突然看到新候选。不用：只在完整打出名称时出现、跟在名称后面、不占首位，默认关等于大多数人永远不知道有这个功能；W#736 抱怨的是前缀匹配加旧的固定第二位，本规则避开了这两点。退路是只改 `Default` 里一个布尔值。

## Consequences

- **收益**：开箱即用的「打拼音出符号」，六个平台同一套规则（都走 host-api → input-runtime → engine），九宫格、14 键也有；每个平台都有开关。
- **代价**：
  - 默认开，升级后完整打出符号名称时会多一两行候选；名称是常用词时会把后面的词往后挤一位（打 `pingfang` 时「平房」从第 4 位降到第 6 位，前面多了 m² ²）。
  - 来源名叫 `Emoji`，读日志和代码的人要知道它也指符号行。
  - 旧版本读不了含 `"symbols": false` 的文档（与 `japanese`、`chaizi` 相同的已接受取舍），只在用户关过开关后降级或跨版本导入时发生。
  - 全角下 ASCII 部分照常变宽（`m²` 上屏成 `ｍ²`），表里专用码位在前。
  - 有云候选或 AI 候选时，第二个以后的符号行排到本地候选之后，与 emoji 现状相同。
  - 改表要发引擎版本；表只有几十 KB。
- **数字序号**：一至五十的序号及数字只认完整拼音的规则由[数字序号候选](2026-10-21-number-symbol-candidates.md)持有。#9088 的参考截图提供了「一」作为 ① 的自然名称依据，数字序号不再排除在表外；其单音节仍受锚定及最早第 4 位约束。
- **没有收录**：用户可编辑的符号表。μ、ν、ρ、τ、φ、χ 的单字中文名当时以单音节不收为由没有收录，3 个字母的拉丁名称也一并排除；两者后来由[补齐希腊字母与数学符号名称](2026-10-25-math-greek-symbol-names.md)放开，拉丁名称的下限改为 3 个字母且不能拆成拼音。
- **验证缺口**：Windows（MinGW）和 Android 原生编译留给 CI；HarmonyOS 只跑了类型检查与键盘逻辑测试，没有打 HAP；Windows 候选窗是否把 ★ ✓ 这类码位画成 emoji 字形（`platforms/windows/msimeui/src/Fonts.cpp` 按码位区间映射 emoji 字体）没有在装机上看过。
- 已检索的相关笔记：[2026-10-08-expressive-rows-follow-their-word.md](2026-10-08-expressive-rows-follow-their-word.md) 是 emoji 行跟着词的规则，符号沿用它的锚定做法，找词的线性扫描与排序二分抽成 `with_first_position` 共用，那篇已加互链；[2026-10-08-nine-key-expressive-candidates.md](2026-10-08-nine-key-expressive-candidates.md) 的九宫格读法与边界检查被符号复用，已加互链；[2026-10-07-mixed-candidate-dedup-state.md](../testing/2026-10-07-mixed-candidate-dedup-state.md) 的栈上去重表容量加上了符号上限，事实已就地更新；[2026-10-12-mixed-japanese-candidates.md](2026-10-12-mixed-japanese-candidates.md) 是「后加的可选偏好成员」的先例，本篇反过来默认开，用 `skip_serializing_if = "is_true"`，无需改那篇。
