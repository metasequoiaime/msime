# Agent Note: 全拼、双拼里混进一行日文整段转换（实验选项）

Status: implemented

## Problem

「自动识别语种」的第二阶段：中文用户偶尔要打一句日文（`ohayou`、`arigatou`、`sugoi`），今天得先按 Shift+R 进临时日文。真实词库里打 `ohayou` 首选是拼音凑出来的 哦哈有（`Generated`），`arigatou` 是 阿日尬头，日文根本不在列表里。想要的是：打完整段罗马字时，列表里直接有一行日文转换，拼音给不出像样的词时它领先，拼音有整段的词时它排在后面，平常的拼音照旧。漏一个键、对调两键的拼音也常常碰巧是完整罗马字，甚至正好是某个日文词条的读音，不能让日文抢在拼音前面。

难点在信号。绝大多数拼音本身就是完整罗马字（`nihao` 是にはお，`women` 是をめん），`ohayou`、`arigatou`、`watashi` 又都能整段切成拼音音节，所以「只有罗马字才有的拼法」不够用；日文行还不能被当成拼音行去推进组字、学进拼音词库。

## Decision

新选项 `SessionOptions::japanese_mixing`（宿主侧 `EngineOptions::mixed_japanese`，`prepare_options_for` 默认关，`session_options` 映射）。开着时会话构造里调 `ImeSession::warm_up_japanese`，经 `ProviderRegistry::warm_up_japanese` 在后台预热日文模型，拼音会话的第一个键不必在按键线程上读这个 66 MB 的文件。

### 偏好与门控

- 偏好是 `Preferences.mixed_input.japanese`（`MixedInputPreferences::japanese`），默认关，`#[serde(default, skip_serializing_if = "is_false")]`，沿用 `NavigationPreferences.mouse_wheel` 的先例：只写前四项的旧文档和宿主（Android `mixedInput()`、macOS 合并字典、Linux `patch_preference_object`）照样能读，关着时文档逐字节不变。
- host-api 在 `apply_pending` 和 `into_engine_options` 两处把它映射到 `EngineOptions::mixed_japanese`，再由 `apply_local_mode_resource_gates` 关掉：`options.mixed_japanese &= has_japanese_model && edition.features.temporary_japanese`。条件和临时日文一样：日文 provider 只在会话方案集含 `JapaneseRomaji` 时才建（版本要有 `temporary_japanese`），模型不在时只拼得出假名，不算转换。五笔版即使状态目录里留着下载来的日文资源包也不开。
- 共享设置页（`packages/ui/src/settings/mixed-input-section.tsx`）在 emoji、颜文字混输之后加一行「日文混输（实验）」。它和临时日语开关一样跟着版本的 `edition.temporary_japanese` 走：不带临时日文的版本（五笔版等，host-api 始终把它关掉）不列出，由输入页经 `InputSharedSettingsSection` 传给 `MixedInputSection` 的 `temporaryJapanese`；带它的版本始终显示，由运行时按词典是否在场门控，打开它不触发日文资源包下载。TS 类型里 `japanese` 是可选成员，不进 `defaultMixedInput`，没有这个键时按关显示。

### 来源

新的 `CandidateSource::MixedJapanese = 13`（不复用空着的 10）。它不属于 `is_dictionary`、`is_generated_or_fallback`、`is_sentence_learning`、`is_online` 中任何一个，于是：`commit_selection` 走整段结束组字的分支（与混输英文同一条路），`learn_candidate` 什么都不学，`record_personal_context` 断开个人上下文链，`is_editable_source` 为假（不能置顶、固定或删除），辅助码注释也不给它算。`from_u8` 认 13，golden 回放的 `source_from` 经它也认 13。

### 门槛

全部满足才查：选项开着；方案是全拼或双拼；没有本地模式、专用英文、光标前缀解码和激活的辅助码（会话这一层在 `InputSession::mixed_japanese_conversion`）；输入只有 a-z（不带撇号、`;`），`is_romaji_complete`，读音至少两个假名；会话带日文（`enabled_schemes` 含 `JapaneseRomaji`，否则没有 provider）且模型读得到。只能拼出假名时不算（`ImeSession::mixed_japanese_conversion`、`JapaneseProvider::whole_reading_lemma` / `whole_reading_best_path`）。

### 信号与给不给

- `exact`：整段平假名读音有完全相同读音的词条，并且读音至少 `MIXED_JAPANESE_EXACT_MIN_MORAE = 3` 拍（`counts_as_exact_japanese`）。拍数由 `romaji::count_morae` 算：拗音、外来音里的小写ぁぃぅぇぉゃゅょゎ并进前一个假名，不单算，促音、拨音、长音各算一拍，所以しょう、きょう是两拍，かいし、わたし、すごい是三拍。provider 只交事实（`WholeReadingLemma { exact, morae }`），三拍的规则在 `ImeSession` 这一层。
- `known`：最佳整句没有用到未知假名回退，并且路径成本不超过每假名 `MIXED_JAPANESE_KNOWN_COST_PER_KANA = 1800`（`reads_as_known_japanese`）。
- `pinyin_incomplete`：第一阶段的音节判定为假（全拼 `is_complete_pinyin_input`，双拼 `is_complete_input`），抽成 `queries::spells_whole_syllables` 两处共用。

`exact || (pinyin_incomplete && known)` 才给一行。先算便宜的两个，拼音拆得开又没有精确词条时不跑整句搜索。最佳整句由新的 `japanese::matrix::best_path` 算：和句子搜索的第一项同一条路径（同样的词条跨度、每个位置留前八条、同样的先后），只多记未知假名个数和假名数，`best_path_is_the_first_sentence_of_the_search` 钉住两者一致。

`known` 原本只想看「没有未知假名回退」。用真实模型一量，随包模型里每个假名（连小写的ぁぃぅぇぉ）都有单字词条，回退从来用不上：`xiexie` 的最佳整句ぃエィ絵也是零回退，造出来的 36180 个拼音错字里 33683 个零回退。所以加了成本上限，数字见下面的校准。

### 位置

领不领先在 `InputSession::mixed_japanese_row`：

- 拼音拆不开时 `lead = known`。只有精确词条、整句不认得（比如打错的拼音碰巧是某个词条的读音，最佳整句成本超过上限）时给一行但不领先，进优先位置。
- 拼音拆得开时 `lead = !chinese_whole_word`。`chinese_whole_word` 是打头的非英文行是词典行（`is_dictionary()`），且选它能结束整段组字（快照算 `answers_key` 用的同一个 `selection_completes_composition`）。拼音拆得开时能给一行的只有精确词条。

领先时占首位；英文已经按第一阶段的规则占了首位时排第二。不领先时进和英文相同的 `priority_slot`（首选之后，有云候选、AI 候选或占位行时再往后），排在英文那一行前面。文字和已有候选（含刚混进来的英文）重复时不加。始终最多一行。这些都在 `insert_mixed_rows_with_japanese`；`insert_mixed_rows` 和九宫格的 `insert_expressive_rows` 不带日文，行为不变。

行的字段：`word` 是转换文字，`pinyin` 是整段输入，`canonical_pinyin` 为空，`scheme` 为 `JapaneseRomaji`，权重 0。

### 下游配套

- **句子模型重排**：`ordering.rs` 的 `rerank_pick` 在首行来源是 13 时不重排（与英文领先同一道检查），来源 13 的行也不参与比较、不会被转到首位（`MIXED_JAPANESE_SOURCE`）。
- **学习**：第一阶段「英文领先时第二行是默认选择」推广成「跟在领先的英文、日文行后面的那一行是默认选择」，英文和日文各占前两位时第三行才是。
- **在线座位**：`msime-input-runtime` 的 `normalize_online_slots` 把 13 当作和英文同类：领先时保持首位（英文领先时第二位），云候选、AI 候选到达不挪；不领先时坐英文的优先座位、排在英文前面。
- **简繁转换**：上屏日文行时提交上下文的 `script_conversion` 为假（`Runtime::commits_mixed_japanese`）；前面暂存着中文词段、和日文一起上屏时照旧转换，否则暂存的中文会漏转。

### 宿主与工具

- **查询（MCP `lookup_candidates`、`msime expand`）**：查询会话照用户的设置保留日文混输，它不学习、不改用户数据，查询看到的就是用户打这串键时的那一行日文。来源 13 映射成 `CandidateOrigin::Japanese`，MCP 输出的 `origin` 是 `japanese`；以前任何不认识的来源都让查询报错「not local」。
- **Windows**：`candidate_finishes_composition` 含 13，和英文 4 一致：引擎选中它就结束组字，Server 选词时不带 `HOST_HOLDS_PIECES`、随后发 `MSIME_CANCEL`，整段上屏照常出声、计连击，不被当成暂存的词段。`scripts/test-candidate-sources.py` 只核对 `CandidateActionAvailability.h` 的命名常量，13 不在置顶、固定、删除的白名单里，所以它不用改。
- **macOS**：`CandidateDisplay` 对来源 13 不做简转繁（学校不显示成學校），上屏由 `commit_context.script_conversion` 为假保证；只有 macOS 宿主读这个字段。
- **释义**：日文行的汉字（私、開始、寿司）是日文，当中文释义会在日文下面写出中文意思，在线释义还花掉账号额度。所以 client-core 的 `source_gets_no_gloss`（原来的 `is_emoji_or_kaomoji_source`，现在含 6、7、13）让 host-api 翻译查询里来源 13 的 `online_gloss` 为假，Engine 的 `candidate_gloss_key` 对 `CandidateSource::MixedJapanese` 返回 `None`，离线释义和自带词典的释义都不查。翻译计划（`ffi/translation.rs`）本来就跳过 10 以上的来源，不用改。这与日文方案（`SchemeType::shows_glosses` 为假）和临时日文不出释义一致。

### 校准（真实模型 `msime-japanese.dat`，66547565 字节）

`real_model_calibrates_mixed_japanese_signals` 的数字（括号里是收紧前的宽规则：任何精确词条都算精确，拼音拆不开时给了就领先）：

- 拼音拆不开、没有精确词条、确实是日文的 20 条（食べます、わかった、知っている、言ってらっしゃい、私は学生です……），`known` 召回 19 条，都领先，只漏了 読んだ（每假名 2003）。两种规则一样，精确信号碰不到它们。
- 每假名上限取 1500 时召回 13 条、2000 时 20 条。
- 拿 `quanpin-words-v1` 25119 个词删一个键或对调两键，造出 36180 个拆不开却是完整罗马字的输入：给一行的 1516 个（4.2%），其中领先的 413 个（1.14%）；宽规则下给出的 2162 个（6.0%）全部领先，其中靠精确词条的 2007 个。只靠 `known` 给出的仍是 155 个（0.43%；上限 1500 时 27 个，2000 时 564 个），多是 `n` 规则拼出的ちゃうん、じゃん一类。
- 打字途中经过的 2104 个拆不开的完整罗马字前缀里，给一行的 293 个，领先的 63 个（3.0%）；宽规则下 467 个全部领先（精确 457、只靠 `known` 10）。
- 拼得开的拼音只靠精确词条混进日文，都进优先位置：25119 个词里 931 个（3.7%；宽规则 1537 个，6.1%），421 个单音节里 20 个（宽规则 177 个：`ai`、`hao`、`dai`、`kai`、`you`、`shou`……）。剩下的 20 个都是读成三拍的拼音：`bian`（びあん）、`jian`、`tian`、`miao`、`kuai`、`yuan`、`suan` 一类。

出货词库加真实模型（`real_resources_mix_japanese_conversions`，产品默认混输英文）：

| 输入 | 前几行（括号里是来源） |
| --- | --- |
| `ohayou` | おはよう(13) 哦哈有(8) 哦 噢 欧 |
| `arigatou` | ありがとう(13) 阿日尬头(8) 啊日 阿日 啊 |
| `watashi` | 私(13) 哇他是(8) 袜套 挖炭 哇 |
| `konnnichiha` | こんにちは(13) 空 恐 控 孔 |
| `sugoi` | すごい(13) |
| `kaishi` | 开始 開始(13) 开始了 开始就 开始时 |
| `tamen` | 他们 多面(13) tameness(4) 他们的 他们是 |
| `sushi` | 苏轼 sushi(4) 素食 俗世 速食（没有日文行，宽规则下寿司在第二位） |
| `konnichiha` | 恐你吃哈(8) 空 恐 控 孔（没有日文行） |
| `nihao`、`women`、`xiexie`、`shenme`、`zaijian`、`wode` | 没有日文行 |
| `hao`、`kai`、`you`、`shou` | 没有日文行（宽规则下はお、回、よう、性进第二位） |

## Alternatives considered

- **宽规则：任何精确词条都算精确，拼音拆不开时给了就领先**（这个功能最初的规则） — 最强的理由是召回最全：两拍的日文词（すし、よう）也有一行，打 `sushi` 能直接选寿司，规则也少一条。没有采用，是因为校准下噪音太大：421 个单音节里 177 个、25119 个拼音词里 1537 个（6.1%）多出一行日文；拆不开的错字里 2162 个（6.0%）、打字途中 2104 个拆不开的前缀里 467 个让日文领先，这时按结束组字的标点会上屏日文。收紧后单音节剩 20 个、整词 931 个，错字领先 413 个（1.14%）、前缀领先 63 个，招牌例子（`ohayou`、`arigatou`、`watashi`、`sugoi`、`konnnichiha`）一个没少，代价是 `sushi` 这类两拍的日文不再出现。
- **拍数按字符数算（しょう算三个假名）** — 最强的理由是和「认得」的每假名成本上限用同一个计数（`BestPath::kana`），不用另写计数。没有采用，是因为拗音和外来音的读法在字符数下够三个，挡不住：同样的校准里单音节有 45 个给一行（按拍数 20 个），多出来的是 `shou` しょう、`chou`、`shen` しぇん、`fan` ふぁん、`wei` うぇい 一类，拼音词 1047 个（按拍数 931 个），错字给一行 1698 个（按拍数 1516 个）；按拍数算才和日文的长度感一致。
- **复用空着的来源 10** — 最强的理由是不占新编号，10 原是 `NeuralDesktop`，没发布就删了，宿主的翻译计划本来就跳过 `10..=255`。没有采用，是因为老客户端或删之前录的数据里 10 仍可能指 `NeuralDesktop`，13 没有任何歧义，代价只是一个编号。
- **把 provider 的行原样混进来，只把 `scheme` 改成 `JapaneseRomaji`** — 最强的理由是不加新来源，宿主和插件 ABI 一个字不改，还能混进多行日文。没有采用，是因为这些行的来源是 `Database` / `Generated`：`commit_selection` 和 `learn_candidate` 都按来源判断，得在每一处另加按 `scheme` 的守卫，漏一处就会把罗马字当拼音学进用户词库；新来源不属于任何谓词，这些路自然不走。
- **只看「没有未知假名回退」当 `known`** — 最强的理由是它是结构性的事实，不用调阈值，也是最初的设计。没有采用，是因为随包模型每个假名都有单字词条，回退从来用不上，造出来的拼音错字九成以上都会被当成认得的日文领先。
- **每假名上限取 1500** — 最强的理由是更保守，错字里只靠 `known` 给出的从 155 个降到 27 个，打字途中的前缀从 10 个降到 2 个。没有采用，是因为召回从 19/20 掉到 13/20，食べたい、飲みます、疲れた、待って都给不出来；而精确词条本身已经给出 2007 个错字行，`known` 收紧那一点改变不了总量。
- **按整句的分段数卡（平均每段至少两个假名）** — 最强的理由是不依赖成本尺度。没有采用，是因为它几乎不区分：ちゃうん、ジャンちゃん这类错字段数本来就少。在 1800 的成本上限之外再加「段数不超过假名数一半（向上取整）」，探索时那组 26 条正例（含 ら面、ときょ 这类本不该给的）少了 3 条（19→16），错字只多挡下 11 个（155→144）。
- **门槛用罗马字独有的拼法（`tsu`、`kya`、双写辅音、`n'`）** — 最强的理由是对拼音零误伤。没有采用，是因为 `ohayou`、`arigatou`、`watashi`、`sushi` 这些最常打的日文都能整段切成拼音音节，一个都碰不到。
- **只要整段是完整罗马字（`is_romaji_complete`）就给一行** — 最强的理由是最简单，不用查模型就能定。没有采用，是因为绝大多数拼音本身就是完整罗马字（`nihao`→にはお、`women`→をめん），等于给每个拼音词都塞一行假名，列表被淹没。
- **像临时日文那样切换会话方案** — 最强的理由是现成：Shift+R 已经把唯一的引擎方案换成 `JapaneseRomaji`，候选和上屏全是日文方案自己的规则，不用新来源。没有采用，是因为同一时刻只能有一种语言的列表，做不到拼音照旧、日文只补一行；用户不知道下一个词是哪种语言时，切方案正是要省掉的那一步。
- **跨语言比分数决定谁领先** — 最强的理由是看起来最「智能」：日文路径成本低于拼音权重就让日文领先。没有采用，是因为日文是 mozc 的连接成本、拼音是词频权重，不在一个尺度上，混输英文早就因此定下「从不比较英文和拼音的权重」（#1040、#1063）。领先只看结构事实：拼音拆不拆得开、拼音有没有整段的词。
- **打开选项时自动下载日文资源包** — 最强的理由是 macOS、Android 精简包上开箱即用，不必另找下载入口。没有采用，是因为这是实验选项，拨一下开关就拉 66 MB 不合适；临时日文也刻意不触发下载（`needed_by_preferences` 只在方案是日文时要它），两个功能读同一份词典，入口一致。
- **在 `Session::snapshot` 里把日文行的 `answers_key` 置为假，代替在 `rerank_pick` 里挡** — 最强的理由是不碰网页引擎也用的 `ordering.rs`。没有采用，是因为 `answers_key` 是行本身的事实（选它确实结束组字），第一阶段为英文做过同样的取舍，两条规则放在同一处更好查。

## Consequences

- **收益**：
  - 开着选项时，`ohayou`、`arigatou`、`sugoi`、`watashi`、`konnnichiha` 空格直接上屏日文，`kaishi` 这类拼音有整段词的仍是中文首选、日文紧随其后，常用拼音（`nihao`、`women`、`xiexie`、`shenme`、`zaijian`、`wode`）和单音节（`hao`、`kai`、`you`、`shou`）不受影响。
  - 日文行不推进组字、不学习、不进个人上下文，句子模型和在线座位都不会把它挪走，繁体输出时不被转成繁体。
  - 选项默认关，脚本化 golden 一个没变；关着时除了多一次布尔判断没有额外开销。
- **代价与已知上限**：
  - 精确词条要三拍以上以后仍有噪音：三拍的读音在 mozc 词库里也常有词条，拼音词里 3.7%（`tamen` 的多面）、单音节里 20 个（`bian`、`jian` 一类）会多出一行日文（不领先）；拆不开的错字里 4.2% 给一行，其中 1.14% 领先，打字途中拆不开的前缀上 3.0% 会让日文短暂领先，这时按结束组字的标点会上屏日文。用户再反馈噪音时，下一步是要求最佳整句就是那个词条且成本在某个上限内（`tamen` 的多面每假名 2145，过不了「认得」的 1800）。
  - 拼音拆得开的两拍日文词（`sushi` 寿司）不再给一行，要打得用临时日文；拼音拆不开的两拍读音只在「认得」时给。
  - `nn` 一律是一个ん（见 [日文罗马字对齐主流输入法](../bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md)），所以 `konnichiha` 读成こんいちは、得不到こんにちは，要打 `konnnichiha`；它的最佳整句今イチは不算认得，不给日文行。
  - 双拼辅助码开着（产品默认）时，奇数长度的最后一键算辅助码，门槛挡住，`sugoi` 这类奇数长度的输入在双拼里拿不到日文行。
  - 开着选项时每个键多一次罗马字转换和一次精确词条查找，拼音拆不开或有精确词条时再跑一次整句搜索；66 MB 的模型在拼音会话里也被读进内存。第一个键若赶在预热之前，仍要在按键线程上等模型。
  - 开着选项的拼音用户进程里多了这份 66 MB 的日文模型（进程级缓存，见下面的模型缓存笔记），没开的用户不读。
  - 打字统计按会话方案记：`commit_context.scheme` 仍是全拼或双拼，日文行算作拼音输入。
  - 选日文行什么都不学：不调频、不进个人上下文，也不会因为常选它就让它更容易领先；要学得另加来源 13 专用的规则。
  - 只有 macOS 认来源 13 的显示和上屏不转繁体。Windows、Linux（IBus、Fcitx5）、iOS、Android、HarmonyOS 按方案号决定简转繁，开着繁体输出时日文行的显示和上屏都会被转（显示和上屏一致，但日文新字体会变成繁体字形）；这些宿主也不给它角标，macOS、Android、iOS 的置顶、固定、删除菜单仍会出现，引擎按 `is_editable_source` 拒绝。
  - 打开过选项的文档带 `mixed_input.japanese: true`，降级到不认这个键的旧版本时整份偏好被拒（`deny_unknown_fields`）；`skip_serializing_if` 把影响限在打开过它的用户，与 `mouse_wheel` 的取舍相同。
- 已检索的相关笔记：
  - [拼音拆不开且英文完全匹配时，混输英文占首选](2026-10-12-mixed-english-leads-unsegmentable-input.md)：部分重叠，本篇沿用它的音节判定、优先位置和「领先行后面是默认选择」，两篇互链。
  - [日语模型留在进程级缓存里，切到日文时后台预热](../architecture/2026-10-10-japanese-model-process-cache.md)：部分重叠，预热时机多了「日文混输开着时建会话」，事实处已就地补上并链到本篇。
  - [日文罗马字对齐主流输入法，完整读音的平假名留在第一页](../bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md)：部分重叠，`nn` 规则决定了 `konnichiha` 的结果，本篇不改它。
  - [拼音组字里直接出日期、时间、星期和农历](2026-10-10-inline-date-time-candidates.md)：无关，它否掉固定优先位的理由（简拼误伤）和本篇精确词条的噪音同类，供重访时参考。

## Verification

- `cargo test -p msime-engine`：`session::tests` 里的 `a_japanese_conversion_leads_or_follows_the_whole_chinese_word`、`short_exact_readings_stay_out_and_unknown_readings_do_not_lead`（两拍的 `kai`、`shou` 不混，拆不开、只有精确词条的 `sukoshi` 进优先位置）、`japanese_mixing_stays_out_unless_every_gate_passes`、`shuangpin_mixes_a_japanese_conversion_too`、`a_leading_japanese_conversion_follows_a_leading_english_word`、`japanese_rows_skip_duplicates_and_respect_the_online_seats`、`committing_a_japanese_row_ends_the_composition_and_learns_nothing`、`picking_the_chinese_row_behind_a_leading_japanese_row_is_the_default_pick`、`japanese_rows_carry_no_helpcode`、`japanese_mixing_warms_the_model_when_the_session_is_built`；`ime::queries` 的位置测试；`japanese::matrix::best_path_tests`；`japanese::provider` 的 `whole_reading_signals_need_a_complete_reading_and_the_model`（拍数与三拍规则）、`japanese::romaji::tests::morae_merge_small_kana_into_the_previous_one`、`the_best_path_is_the_first_provider_row`；`types::tests::candidate_sources_round_trip_and_mixed_japanese_matches_no_predicate`；`host::tests` 的选项默认值与映射。
- `cargo test -p msime-input-runtime`：`mixed_japanese_rows_keep_their_seat_like_english`、`committing_a_mixed_japanese_row_skips_script_conversion`、`the_reranker_leaves_mixed_japanese_rows_alone`。
- `cargo test -p msime-client-core`：`mixed_input_legacy_roundtrip_and_bounds`（四项旧对象可读，关着时不写 `japanese`）、`translation::tests::emoji_kaomoji_and_mixed_japanese_sources_get_no_gloss`。
- `cargo test -p msime-host-api`：`mixed_input_changes_defer_until_composition_ends`、`local_mode_resource_gates_preserve_unrelated_modes`、`the_wubi_lock_accepts_resources_without_japanese_and_gates_temporary_japanese_off`、`a_downloaded_japanese_pack_keeps_temporary_japanese_available`（模型在与不在、full 与五笔版），`dictionary::tests::a_lookup_keeps_the_users_japanese_mixing`（查询首行是来源 `Japanese` 的おはよう，关着时没有）、`translation_queries_never_clear_a_mixed_japanese_row_for_the_network`（`watashi` 的首行私来源 13，翻译查询里 `online_gloss` 为假）。`msime-engine` 的 `host::tests::gloss_keys_follow_the_bridge_rules` 断言来源 13 没有释义键。
- `cargo test -p msime-mcp-server`：`a_japanese_mixing_candidate_is_reported_with_the_japanese_origin`。
- 前端：`apps/desktop/tests/settings/mixed-input-section.test.tsx` 与 `settings.test.tsx` 的日文混输开关（缺省为关、单独切换、和其他混输开关一起保存、`temporaryJapanese` 为假时不列出），`input-scheme-edition.test.tsx` 断言完整版和拼音版列出、五笔版不列出。
- Windows：`platforms/windows/tests/input/candidate_completion_policy.cpp` 断言 13 结束组字。
- 真实模型：`MSIME_JAPANESE_MODEL=<msime-japanese.dat> cargo test -p msime-engine --lib real_model_calibrates_mixed_japanese_signals -- --nocapture`；出货词库：再加 `MSIME_EVAL_RESOURCES=<资源目录>` 跑 `real_resources_mix_japanese_conversions`。两者缺环境变量时跳过。
