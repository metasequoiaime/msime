# Agent Note: 全拼 14 键（Android、iOS、HarmonyOS）

Status: implemented

## Problem

移动三端的全拼只有两种触屏布局：26 键和九宫格。26 键在窄屏上每个键很小，九宫格把三四个字母压在一个数字键上，重码多（421 个全拼音节在电话键盘上只有 218 种码，133 种有重码，涉及 336 个音节）。14 键介于两者之间：QWERTY 的位置不变，相邻两个字母合成一个键（`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`），同样 421 个音节有 314 种码，86 种有重码，涉及 193 个音节。键比 26 键宽，记忆负担比九宫格小。

仓库里的九宫格解码（`crates/engine/src/nine_key.rs` 的 `NineKeySession`）原来按电话键盘写死：字母到数字的表 `KEYPAD`、数字上的字母 `DIGIT_LETTERS`、只收 `2`–`9` 的 `character`、按字节形状判断读音行的 `reading_for`、进程级单例 `spelling_table()`。`Session` 只把数字 `2`–`9` 交给它，硬件键盘的字母也走同一个 `character`。宿主三端都按「九键 / 不是九键」二分做门控（Android 的 `== QUANPIN_NINE_KEY_LAYOUT`、iOS 的 `== .nineKey`、鸿蒙的 `=== 'nine_key'`），多出一种布局时任何一处漏改，14 键就静默得到 26 键的行为。

三端必须表现一致：同一份行为规格，同一张键表，同一套消歧交互。

## Decision

Android、iOS、HarmonyOS 提供触屏方案「全拼 14 键」。解码复用九宫格会话，只换一张码表（`KeyGrid::FourteenKey`）；组码经新的 `grid_key` 入口进来，不走 `character`；宿主按 `View.key_grid` 决定 14 键的读音行、拼音选择条和三栏面板。三端按下表的同一份规格实现，键表由 `scripts/test-fourteen-key-table.py` 核对。

### 三端行为规格

| 项 | 规格 |
|---|---|
| 方案 | 触屏方案「全拼 14 键」，preferenceId 与 layout 都是 `fourteen_key`，引擎方案 quanpin，卡片字形「拼」、角标「14」，按 append-only 追加在 `TouchKeyboardScheme::ALL` 末尾 |
| 默认 | 不启用：不进 `DEFAULT_ENABLED`，也不进 `LEGACY_DEFAULT_ENABLED`；只有一个方案的版本照 `for_edition` 启用全部入口 |
| 设置 | 全拼的「中文键盘」是「26 键 / 14 键 / 9 键」三选一，选中即启用并切换；普通话的方案列表也按 26 键、14 键、9 键排列，选择器的方案顺序仍是 append-only 的 `ALL` |
| 适用方案 | 只给全拼；双拼、五笔、日文、注音都没有 14 键 |
| 提供范围 | 九键在哪里提供，14 键就在哪里提供：Android 全部形态、iPhone、iPad、鸿蒙手机和平板；鸿蒙 2in1 隐藏，同步过来的 `fourteen_key` 在 2in1 上画 26 键、不开网格 |
| 键面 | 4 行等高，总高度等于 26 键。第 1 行 `QW ER TY UI OP`；第 2 行 `AS DF GH JK L`，不缩进；第 3 行 `[符/分词] ZX CV BN M [⌫]`，两端边键宽度沿用各宿主 26 键 Shift/⌫ 的比例；底行就是各宿主的 26 键手机底行，逗号键连同它的长按常用标点也一样 |
| 键面文字 | 每键两个大字母，没有角标；无障碍标签「按键 Q W」，单字母键「字母 L」 |
| 组码 | 每组首字母的小写：`q e t u o a d g j l z c b m`。`a`–`z` 依次落在 `abcdedggujjlmbooqeatucqztz` |
| 左端键 | 照搬九键 1 键：组字中是「分词」，送 `'`；空闲时是「符」，打开符号面板。两种状态都记作键位 `SoftSymbol`。中文 14 键没有 Shift |
| 组字 | 点一下输入的是一组字母，不是某个字母。组码不写进编辑框（不论 iOS 的「行内预编辑」怎么设），读音行显示 `nine_key_reading` |
| 拼音选择条 | 放在读音行右侧，横向滚动，出现时读音最多占读音行的一半，不盖住候选行；在 123 层和三栏面板开着时照样画。「26 键数字键盘」选了九宫格时 123 层借九键的数字层，选择条仍在读音行，侧栏照常是标点（符号栏），不让给拼音。内容与九键左列相同：先完整音节，再列下一键上能作声母的大写字母；没有「原样上屏」那一项。点音节锁定，退格先撤销锁定 |
| 展开候选 | 九键的三栏面板，单字和笔画筛选照常可用 |
| 长按 | 弹出这一键的两个字母（L、M 不弹）；选中后与九键相同：先结束组字，再上屏这个字母 |
| 按键气泡 | 不画，同九键 |
| 原样上屏 | 组字中走原样上屏的路径（Android 的回车、123 层字符、各种面板）结束组字，从宿主高亮的那一行开始、其余各段取首选，与 `Finish` 相同；iOS 的回车和切换模式按 `CompositionBoundaryPolicy` 本来就结束组字 |
| 英文 | 英文模式画 26 键 QWERTY，切回中文恢复 14 键；英文模式不改写 `touch_keyboard_layout`；没有「英文 14」 |
| 英文混入 | 中文组字时，拼音读不通的组码照样混入英文词，由引擎按网格计算 |
| 123 | 打开与 26 键相同的设计层，跟随 `touch_twenty_six_key_number_layout` |
| 回落 26 键 | 密码框、URL 框、本地模式，同九键的 prefersFullFace 和门控；本地模式画 26 键时组字照常按「行内预编辑」标记 |
| 不组字的编辑框 | 数字、电话、日期这类没有引擎会话的编辑框里，14 键的点按不输入：一下是一组字母，照字面只能写这一组的首字母 |
| 关闭 | 滑行输入、分体、数字行、双拼键位提示、角标滑动 |
| 硬件键盘 | 物理字母照常走全拼 26 键路径；14 键组字进行中，字母返回 unhandled，同九键 |
| 统计 | 来源 `fourteenKey`（「全拼 14 键」），「输入方式」有自己的「14 键」一档；键位 id `FourteenQW` 到 `FourteenM` 共 14 个，不记到 26 键的 `KeyQ` 上，三端统计页都写成「14 键 QW」 |

### 引擎：`KeyGrid` 泛化九宫格

`nine_key.rs` 有 `pub enum KeyGrid { NineKey, FourteenKey }`，Copy 枚举加 match 取静态表，从 `lib.rs` 导出。它提供 `code_of`、`letters`、`is_code`、`encode`、`offers_raw_key`（九键真、14 键假）和 `spelling_table`（每种网格一个 `OnceLock`）。`NineKeySession` 有字段 `grid`，默认 NineKey；`encode`、`letters_spell_code`、`word_matches_digits`、`keyword_spells_digits`、`letters_for_digit`、`initials_for_digit`、`initials_codes`、`letter_prefixes`、`SpellingTable` 的构造、`key_choices`、`character` 都走 `self.grid`。`SyllablePrior` 与网格无关，两种网格共用。

引入 `KeyGrid` 的那一步是纯重构：九键的单测和 golden 一字不改，两份九键评测基线逐字节相同。

14 键的组码选每组首字母的小写，于是它们与左列的大写字母项、数字项和 `'` 都不冲突，`choice_kind` 不用改。`reading_for` 是正向判定：首行来自拼音路径（`canonical_pinyin` 非空，来源是词库、整句或生成的整句）才给读音；14 键首行是英文词时，读音是这个词覆盖已打键数的那段小写字母。Android 读音为空时会退回 preedit，而 14 键的 preedit 是 `bugao` 这种组码，会误导用户。九键下这个判定的结果与按字节形状判断等价，有单测钉住。

### `grid_key`：另开入口

`Session` 的网格是 `key_grid: Option<KeyGrid>`，由 `set_key_grid` 设置；`grid_key(letter)` 只在 FourteenKey、全拼、没有本地模式、不在专用英文里、26 键预编辑为空或九宫格正在组字、按键是 `a`–`z` 时受理，受理后由 `code_of` 归一，宿主送组里哪个字母都行，三端约定送首字母。`set_nine_key_enabled(b)` 是包装，英文 T9 和注音九键只在 NineKey 下设置。`character` 的数字分支只在 NineKey 下进九宫格。

input-runtime 的 trait 有 `set_key_grid`、`grid_key` 和 `Action::GridKey`；`Runtime::set_key_grid` 套用 `set_nine_key_enabled` 的校验（FourteenKey 只允许 quanpin，组字中切换报 `CompositionActive`）。host-api 有 `msime_client_set_key_grid`（0 关、1 九键、2 十四键）和 `msime_client_grid_key`，`msime_client_set_nine_key_mode` 保留为包装。

### `View.key_grid` 的语义

View 的 `key_grid` 是 `"none" | "nine_key" | "fourteen_key"`，`nine_key: bool` 仍只表示九键。runtime 有两道门看 `key_grid != none`：`ChooseNineKeySpelling` 的过期判断，和未处理的 `'` 不转成标点。数字不当选词键这一条仍只看九键。`nine_key_spellings`、`nine_key_reading`、`nine_key_single_character`、`nine_key_strokes` 两种网格共用，字段不改名。

宿主用 `key_grid` 决定读音行、拼音选择条、隐藏组字和三栏面板，不能再用 `nine_key`：14 键下 `nine_key` 是假，点拼音选择条会被当成过期请求全部拒绝。

`Runtime::set_nine_key_enabled` 跳过重复设置时比的是网格而不是 `nine_key`：14 键开着时关九键会真的关掉网格，宿主切回 26 键时调哪个入口结果都一样。host-api 记住宿主设下的网格（`Option<Option<KeyGrid>>`，`Some(None)` 是宿主关掉了网格），与九键的覆盖一样只保留到方案或 `touch_keyboard_layout` 改变为止；没有覆盖时按偏好的布局开网格，`fourteen_key` 只在全拼下开 14 键。Android 因此不调 `NativeClient.setKeyGrid`（入口已接好），建会话和重建会话时 host-api 按偏好开网格。

### 原样上屏与学词

九键的数字本身是可上屏的文字，14 键的组码不是：按「你好」的键是 `bugao`。`NineKeySession` 的 `CommitRaw` 在 14 键下（`!offers_raw_key()`）有候选时与 `finish(0)` 相同，结束组字、逐段取首选，锁定的音节照样算数；没有候选时只剩组码可上屏。宿主有多条路在组字中发原样上屏——Android 的回车和 `commitNineKeyLiteral`，鸿蒙在 123 层点字符、打开符号或表情面板——修在引擎里，三端一次对齐。

引擎的首位不一定是用户看到的首位：runtime 会按键盘模型重排（`sentence_association.neural_keyboard`）或把次选读音往后放，宿主还可能高亮着另一行。所以 runtime 在 14 键网格组字时把 `CommitRaw` 换成 `finish(engine_index(highlighted))`，与 `Finish` 和标点那两条路相同；iOS 本来就发 `Finish`，三端上屏的都是屏幕上高亮的那一行。九键的 `CommitRaw` 仍交给引擎上屏数字。引擎快照为此多了 `grid_composing`：14 键布局下硬件键盘打的是 26 键组字，它的原样上屏照旧是字母。

三端都开着 `phrase_preedit`。退格收回一次选词时，runtime 把这次选词吃掉的读音重新打回去；14 键的读音是组码字母，原来经 `character` 打回去就进了全拼 26 键，之后 14 键的键全都不处理。`PhraseSelection` 记下选词时正在组字的网格，14 键的组码改从 `grid_key` 打回去，分词键 `'` 和九键的数字仍从 `character` 进。

宿主层的回车学词策略（`commit_raw_with_policy`）不学九宫格组字时上屏的文字（`Session::grid_composing`）：那不是用户逐个打出的字母。否则 14 键的 `bugao` 全是字母，会被学进英文词库；九键的数字学不进去，每次回车都报「English word could not be learned.」。

### 同步分期

- Android 直接上传 `platform.android.keyboard_layout = "fourteen_key"`；服务端接受 32 字节以内的任意字符串，旧版客户端遇到未知值只跳过这一个键。
- 鸿蒙导入端的 LAYOUTS 有 `fourteen_key`，这个键的未知值从「整份拒收」改为跳过。本版导出时本机是 `fourteen_key` 就省略这个键；上传按 merge 覆盖云端快照，省略等于保留云端旧值，旧鸿蒙不会拒收。下一版再开放导出。
- iOS 云端只有布尔键 `platform.ios.nine_key`，这一版 14 键上传 `input.schema=quanpin`、`nine_key=false`，在别的设备上落成全拼 26 键；`platform.ios.keyboard_layout` 是 msime-cloud 仓库之后的改动。在本机「应用云端设置」时，云端是这组值而本机是 14 键就保留本机：原生 App 经 `IOSPreferencePlan.scheme(keeping:)`，Tauri 公共组件的 iOS 工程经 `IosPreferencePlan::requested_native`，同一条规则。云端是九键或别的方案时照常切换。
- client-core 先于任何宿主认识 `fourteen_key`：偏好严格解析，未知值会让整份偏好读不出来。

### Android 宿主

- `KeyboardScheme.QUANPIN_FOURTEEN_KEY` 追加在末尾，`optIn`；`fromPreferences` 有显式分支，通用循环同时排除 nine_key 和 fourteen_key。`KeyboardLayout.FOURTEEN_KEY_LAYOUT = 8`，`resolveTouchLayout` 先判 14 键再判九键；`View.key_grid` 是 `fourteen_key` 且没有本地模式时画 14 键，本地模式要逐个确定的字母，回落 26 键。
- 键表、组码、键面、无障碍标签、长按字母和键位 id 在无 Android 依赖的 `keyboard/FourteenKeyLayout.java`；`ImeLetterRows` 用 26 键的行高和边键权重画三行字母，底行就是 26 键的设计底行。点按调 `gridKey`，不走 `type()`，以免误入辅助码、微软双拼 `;` 和英文直出。
- 拼音选择条是 `SpellingPlacement.READING_ROW`：挂在读音行右侧的槽里，出现时读音收成自身宽度、最多半行。`showNineKeyHoldOptions(anchor, digit, letters)` 泛化成九键和 14 键共用。
- 数字行、分体、滑行、Shift、双拼提示和角标的门本来就只放 26 键和韩文，14 键不进；读音行、三栏面板、`drawsDesignLayer`、`twentySixKeyDigits` 和 `layerTitle`（「14键」）纳入 14 键。`englishNineKeyActive`、九键自己的数字层和引导、皮肤预览、热力图仍只认九键。
- 设置「26 键数字键盘」的说明改为同时适用于 26 键和 14 键。

### iOS 宿主

- 方案是 `ChineseInputScheme.fourteenKey`，追加在 `stroke` 之后，加入 `optInSchemes`；共享文档里写 `fourteen_key`，引擎方案是全拼。桥接层把 `nineKeyEnabled` 换成 `KeyGrid`（none、九键、14 键），重建会话时重放，切到别的方案时复位；按键经 `gridKey` 送组码。
- 键盘扩展的三排键在 `FourteenKeyLayout.swift`，用 `makeKey` 建，不进 `letterButtons` 和滑行的 `glideLetterKeys`，所以 Shift、双拼键位提示、角标滑动和滑行都碰不到它们。第三排两端的分词键和 ⌫ 宽度沿用 26 键的 ⇧：手机 44pt，iPad 按 26 键第三排 ⇧ 占一排的比例。
- 拼音选择条是读音行右侧的横向滚动条（`readingSpellingStrip`），显示时至少占读音行一半宽，读音更长时由读音截断让出；九键的拼音条仍在网格左侧的侧栏里。三栏展开面板与九键共用。
- iPad 与九键一样提供 14 键：不分体，不画数字行；底行用手机 26 键底行，因为 iPad 26 键底行的回车在第二排字母末尾，14 键没有那个位置。
- 切到 14 键之前按下、之后才到的 26 键字母被丢掉，不会开始一段 26 键组字。

### HarmonyOS 宿主

- NAPI 有 `setKeyGrid`（int32，先检查能放进一个字节，不截断）和 `gridKey`，登记在 ENTRY 表并在 `index.d.ts` 声明；`setNineKeyMode` 保留。`KeyboardScheme.ts` 有 `KeyGrid`（0 / 1 / 2，与 C ABI 一致）、`QUANPIN_FOURTEEN_KEY`、`usesFourteenKeyFace` 和 `keyGrid(layout, scheme, desktop)`，2in1 上 `keyGrid` 返回 NONE。
- `KeyboardSession` 的网格是 `keyGrid`，建会话、获得焦点、词库维护后重建三处都重发 `setKeyGrid`；`pressGridKey` 走 `gridKey`。`TypingStatisticsPolicy.source` 的第 4 个参数是 `KeyGrid` 数值，不再是九键布尔。
- `KeyboardView` 的 `fourteenKeyFace()` 排在 26 键字母行之前；`qwertyLettersShowing` 不含 14 键（没有滑行），`twentySixKeyLetterRows` 含 14 键（123 跟随偏好）。长按复用 `SURFACE_NINE_KEY_HOLD`，`openKeyHold(options)` 由九键和 14 键共用。英文模式下引擎的 14 键网格保持开着，引擎在专用英文里不受理 `grid_key`，字母走 26 键。

### 三端对齐

三个宿主分头实现后逐项对照，按多数一方对齐了四处：iOS 去掉了 14 键的按键气泡；鸿蒙的读音最多占读音行的一半（原来六成）；iOS 和鸿蒙的分词键在组字时也记 `SoftSymbol`（原来记成 `'` 的键位，Android 一直记 `SoftSymbol`）；鸿蒙在 123 层和三栏面板开着时照样画拼音选择条。分词键和「符」的读屏文字仍各随本宿主九键 1 键的说法，三端九键本来就不同。

`scripts/test-fourteen-key-table.py` 比对引擎的十四组和编码表、三端的三行键表、Android 的 `GROUP_CODES`，以及 client-core、Android、iOS、HarmonyOS 的 `Fourteen*` 键位 id。它按文件名被 `scripts/run-checks.sh` 发现，`verify-local.sh` 和 contracts 工作流都跑。

### 评测

`verify-local.sh` 有两个 14 键集合：`fourteen-key`（`quanpin-words-v1.tsv`，`--limit 3000`）和 `fourteen-key-sentences`（`sentences-nine-key-v1.tsv`），基线是 `resources/eval/baseline-fourteen-key*.json`。`convert_eval` 和 `rerank_latency` 有 `--grid nine|fourteen`，`--nine-key` 是 `--grid nine` 的别名。

决策点是在同一 limit、同一词库下，14 键的词级 top-1、top-5 和长句 top-1 都不低于九键，`rerank_latency --grid fourteen` 的 p95 不高于九键，才做宿主。实测（锁定资源 dict-v2.0.14 加 `sentence-model.safetensors`，同一次构建，常量沿用九键的，没有调）：

| 集合 | 九键 top-1 / top-5 | 14 键 top-1 / top-5 |
|---|---|---|
| `quanpin-words-v1` `--limit 3000`（实取 2791 条） | 0.575 / 0.851 | 0.660 / 0.906 |
| `sentences-nine-key-v1` 81 条 | 0.506 / 0.519 | 0.568 / 0.568 |

`rerank_latency` 在 `sentences-nine-key-v1` 上交替各跑两轮，挂重排模型的 p95 九键 22.34 / 22.29 ms、14 键 20.80 / 20.78 ms。三项都过了决策点。

## Alternatives considered

- **14 键复用 `character` 送组码。** 最强的理由是改动最小：九键就是这样接的，宿主、C ABI、runtime 都不用加入口。但硬件键盘的字母也走 `character`（Android `onKeyDown` 之后的分支），用 `character` 送组码会让物理 q 变成有歧义的 QW 组，而物理 w 仍是确切字母；九键能复用 `character`，是因为拼音里没有数字。另开 `grid_key` 后，组码只能由触屏键面送进来。
- **另写一个 14 键解码器。** 理由是互不干扰，九键的热路径一行不动。但九宫格的路径搜索、锁定、切分、整句种子、简拼、英文混入、学习都与「字母怎么变成码」无关，只依赖码表；另写一份就是两套要同步维护的解码器，以后的九键改进（如 [九键整句改由数字层统一词网格解码](../../proposed/feature/2026-10-09-nine-key-digit-lattice.md)）也要做两遍。`KeyGrid` 是 Copy 枚举加 match，不引入 trait。[九键双拼](../../proposed/feature/2026-10-10-nine-key-shuangpin.md) 同样需要按方案构造 `SpellingTable`，它可以在 `KeyGrid` 上再加一种网格。
- **鸿蒙导出时把 `fourteen_key` 写成 `twenty_six_key`。** 理由是旧鸿蒙客户端一定读得懂，不会整份拒收。但上传按 merge 把本机的键覆盖到云端快照上，写 `twenty_six_key` 会把同一账号的其他鸿蒙设备切回 26 键；省略这个键等于保留云端旧值，同样不会被旧版拒收。
- **拼音选择条沿用 Android 九键的挂法，叠在候选行上。** 理由是现成代码（`attachSpellings(candidateViewport, true)`），14 键键面没有九键那样的左列空间。但它会盖住候选，用户在选拼音和看候选之间只能二选一；放在读音行右侧、横向滚动，两者同时可见，三端也能放在同一位置。
- **首版就做逐位确定字母（长按选中的字母约束这一位，而不是结束组字再上屏）。** 理由是更接近用户想要的「这一位就是 w」，长按不打断组字。但这要把 `initial` 推广成逐位约束，影响锁定、切分、种子和简拼的每条路径，范围与 14 键本身相当；首版沿用九键的长按行为，逐位字母另行排期。
- **14 键的原样上屏写读音行的字母（`ni'hao` 去掉分隔就是 `nihao`）。** 理由是最接近 26 键「回车上屏打出的字母」，所见即所得，打英文词时也合适。但读音是引擎替用户挑的一种读法：同一串 `bugao` 首选是「不好」时读音是 `bu'hao`，未覆盖的部分仍是组码，用户没有逐个字母地打过它们；结束组字取首选与 iOS 对九键、14 键的回车处理一致，也与九键在面板、标点前结束组字的做法相同。
- **14 键也去掉底行逗号键的长按常用标点（规格原写「关闭快捷标点」）。** 理由是规格把它和滑行、分体并列为 14 键不需要的东西。但 Android 和 iOS 的 26 键手机底行里，逗号键本身就是常用标点键，关掉它等于拿掉逗号；规格同时要求底行与 26 键相同，两条冲突时保留底行，三端一致。
- **在引擎里把「你好」排到「不好」前面。** 理由是三端的手工脚本按「你好」首选写，用户第一次打 BN UI GH AS OP 多半想要「你好」。但锁定词库里「不好」的权重 500381 高于「你好」332885，排序只看权重与学习；为一个例子改常量会牵动所有四重码，评测里的 14 键 top-1 已经高于九键。用户点读音行的 ni 即锁定，选过一次后由学习调整。

## Consequences

- **收益**：
  - 14 键与九键共用一套解码，九键以后的改进 14 键自动得到。
  - 组码入口与硬件字母分开，外接键盘的字母始终是确定的全拼。
  - 三端的键表由检查脚本钉住，读音行、拼音选择条、长按、原样上屏、统计键位按同一份规格。
  - 14 键的词级和长句准确率都高于九键，延迟不高于九键。
- **代价与已知上限**：
  - 14 键歧义仍高（86 种重码，另有 sha 对 a'ga 这类跨音节歧义），常量沿用九键的。真实词库下 bu 和 ni 同组码，BN UI GH AS OP 的首选是「不好」，「你好」排第二；三端手工验收以「你好」在候选里、锁定 ni 后成为首选为准。
  - 候选行的 `pinyin` 字段在 14 键下是组码字母，凡是把它当真拼音用的地方都会悄悄出错；宿主不能展示候选的 `pinyin`。
  - 降级：选了 14 键后装回旧版，偏好严格解析整份读不出来。照现有的严格解析接受这个代价，写进发布说明；宿主绝不能先于 client-core 写 `fourteen_key`。
  - iOS 的 14 键不跨设备同步，别的设备落成全拼 26 键；本机应用云端时保留 14 键的规则也意味着，另一台设备上特意选的全拼 26 键不会覆盖这台的 14 键。`platform.ios.keyboard_layout` 落地后这两条都要重访。
  - 三端的非穷举判断（`== QUANPIN_NINE_KEY_LAYOUT`、`== .nineKey`、`=== 'nine_key'`）以后再加布局，还要逐处列清单。
  - 14 键 golden 没有 C++ 参考实现，预期由 Rust 自己生成，只防回归，正确性以单测和评测为准。
  - 皮肤预览、引导页和打字统计热力图对 14 键用户仍画 26 键或九键；14 键热力图键盘下一步再画（键位 id 已经记录）。
  - 主流输入法的 14 键交互细节没有在设备上核实，这里的规格以本仓库九键的行为为基准。

## Verification

- 评审后的修复：runtime 的 `retreating_a_fourteen_key_selection_types_the_codes_back_on_the_grid`、`fourteen_key_commit_raw_finishes_from_the_highlighted_row` 和 host-api 用真实引擎的 `retreating_a_fourteen_key_selection_keeps_composing_on_the_grid`（去掉修复时失败）；iOS 的 `testTheBorrowedDigitPadKeepsItsPunctuationWhileComposing` 和 `testInlinePreeditSkipsTheCodesButMarksALocalMode`（去掉修复时失败），`FourteenKeyKeyboardTests` 12 项与 `NineKeyKeyboardTests` 共 104 项在 iOS 27.0 模拟器上通过；Android 的三栏面板判断改为按布局（`NineKeyPanelPolicy.threeColumn(layout, …)`），`FourteenKeyLayoutSmoke` 用 `FOURTEEN_KEY_LAYOUT` 断言，`FourteenKeyDeviceSmoke` 加了展开三栏面板和借九键数字层两段；共享统计页的 `keyLabel` 与 `softKey` 认识 `Fourteen*`。 Android 在 API 35 模拟器上 `smoke.sh` 全部 16 组通过，API 28 上 `smoke.sh --core` 通过；鸿蒙的 `pressGridKey` 改动跑了 `tests/run.sh` 和 `scripts/test-harmony-*.py`，`hvigorw assembleHap` 这次没有重跑（本机没有暂存的原生库和 `sherpa_onnx.har`）。
- 共享层：`cargo test -p msime-engine -p msime-client-core -p msime-input-runtime -p msime-host-api` 全部通过，其中 `nine_key::tests::fourteen_key_*`、`fourteen_key_commit_raw_finishes_instead_of_writing_the_codes`、`host::tests::grid_raw_commit_is_not_learned_as_an_english_word` 钉住原样上屏和学词；`msime-desktop` 的 `applying_cloud_quanpin_keeps_a_local_fourteen_key_choice` 与 `shared/backend` 的 `testCloudQuanpinKeepsALocalFourteenKeyChoice` 钉住 iOS 应用云端的规则。`scripts/test-fourteen-key-table.py` 在四处一致时通过，人为改错某一端的一个键、一个键位 id 或引擎的编码表时失败。
- Android：`check-host.sh`（198 个 JVM smoke）、162 个 `scripts/test-android-*.py`；API 35 模拟器上 `smoke.sh` 全部 16 组通过，含 `FourteenKeyDeviceSmoke`（你好、shi/shu 锁定与撤销、分词得西安、长按 QW 选 w、英文与密码框画 26 键、123 是设计层）和 `KeyboardHeightDeviceSmoke` 的 14 键一轮；API 28 上 `smoke.sh --core` 通过，14 键用 `adb input tap` 实打。UiAutomation 的无障碍点击在 API 30 以下送不到输入法，所以 `FourteenKeyDeviceSmoke` 只在 API 35 跑。
- iOS：iOS 27.0 模拟器上 `MSIMEClientTests` 836 项、7 跳过、0 失败（链接的是含原样上屏修复的原生库），`FourteenKeyKeyboardTests` 10 项覆盖键表、你好、西安、shi/shu、长按、英文与 123、过期点按和 iPad；`OnboardingUITests.testChineseKeyboardOffersTwentySixFourteenAndNineKeys` 通过；`MSIMEApp` 编译通过。`KeyboardExtensionEditorUITests` 的 14 键用例在模拟器上因键盘没有加进系统键盘列表而跳过。
- HarmonyOS：`platforms/harmony/tests/run.sh` 0 失败，`scripts/test-harmony-*.py` 全部通过，`hvigorw assembleHap` 成功；手机模拟器上用临时 bundle 实打你好、西安、长按 QW 选 w、英文切换和三栏面板。2in1、密码框回落和设置页三档没有在设备上验证，只有逻辑测试。
