# Agent Note: 全拼 14 键（Android、iOS、HarmonyOS）

Status: proposed

## Problem

移动三端的全拼只有两种触屏布局：26 键和九宫格。26 键在窄屏上每个键很小，九宫格把三四个字母压在一个数字键上，重码多（421 个全拼音节在电话键盘上只有 218 种码，133 种有重码，涉及 336 个音节）。14 键介于两者之间：QWERTY 的位置不变，相邻两个字母合成一个键（`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`），同样 421 个音节有 314 种码，86 种有重码，涉及 193 个音节。键比 26 键宽，记忆负担比九宫格小。

仓库里的九宫格解码（`crates/engine/src/nine_key.rs` 的 `NineKeySession`）按电话键盘写死：字母到数字的表 `KEYPAD`、数字上的字母 `DIGIT_LETTERS`、只收 `2`–`9` 的 `character`、按字节形状判断读音行的 `reading_for`、进程级单例 `spelling_table()`。`Session` 只把数字 `2`–`9` 交给它，硬件键盘的字母也走同一个 `character`。宿主三端都按「九键 / 不是九键」二分做门控（Android 的 `== QUANPIN_NINE_KEY_LAYOUT`、iOS 的 `== .nineKey`、鸿蒙的 `=== 'nine_key'`），多出一种布局时任何一处漏改，14 键就静默得到 26 键的行为。

三端必须表现一致：同一份行为规格，同一张键表，同一套消歧交互。

## Proposal

### 三端行为规格（对齐清单）

| 项 | 规格 |
|---|---|
| 方案 | 触屏方案「全拼 14 键」，preferenceId 与 layout 都是 `fourteen_key`，引擎方案 quanpin，卡片字形「拼」、角标「14」，按 append-only 追加在 `TouchKeyboardScheme::ALL` 末尾 |
| 默认 | 不启用：不进 `DEFAULT_ENABLED`，也不进 `LEGACY_DEFAULT_ENABLED` |
| 设置 | 全拼的布局从「26 键 / 9 键」改为「26 键 / 14 键 / 9 键」三选一，选中即启用并切换 |
| 适用方案 | 只给全拼；双拼、五笔、日文、注音都没有 14 键 |
| 提供范围 | 九键在哪里提供，14 键就在哪里提供：Android 全部形态、iPhone、iPad、鸿蒙手机和平板；鸿蒙 2in1 隐藏，同步过来的 `fourteen_key` 在 2in1 上画 26 键 |
| 键面 | 4 行等高，总高度等于 26 键。第 1 行 `QW ER TY UI OP`；第 2 行 `AS DF GH JK L`，不缩进；第 3 行 `[分词/符] ZX CV BN M [⌫]`，两端边键宽度沿用各宿主 26 键 Shift/⌫ 的比例；底行沿用 26 键手机底行 `123 , 空格 。 中 回车` |
| 键面文字 | 每键两个大字母，没有角标；无障碍标签「按键 Q W」，单字母键「字母 L」 |
| 组码 | 每组首字母的小写：`q e t u o a d g j l z c b m`。`a`–`z` 依次落在 `abcdedggujjlmbooqeatucqztz` |
| 左端键 | 照搬九键 1 键：组字中送 `'`，空闲时打开符号面板；中文 14 键没有 Shift |
| 组字 | 点一下输入的是一组字母，不是某个字母。组字不写进编辑框，读音行显示 `nine_key_reading` |
| 拼音选择条 | 放在读音行右侧，横向滚动，三端位置相同，不盖住候选行。内容与九键左列相同：先完整音节，再列下一键上能作声母的大写字母；没有「原样上屏」那一项。点音节锁定，退格先撤销锁定 |
| 展开候选 | 九键的三栏面板，单字和笔画筛选照常可用 |
| 长按 | 弹出这一键的两个字母（L、M 不弹）；选中后与九键相同：先结束组字，再上屏这个字母 |
| 英文 | 英文模式画 26 键 QWERTY，切回中文恢复 14 键；英文模式不改写 `touch_keyboard_layout`；没有「英文 14」 |
| 英文混入 | 中文组字时，拼音读不通的组码照样混入英文词，由引擎按网格计算 |
| 123 | 打开与 26 键相同的设计层，跟随 `touch_twenty_six_key_number_layout` |
| 回落 26 键 | 密码框、URL 框、本地模式，同九键的 prefersFullFace 和门控 |
| 关闭 | 滑行输入、分体、数字行、双拼键位提示、快捷标点、角标滑动 |
| 硬件键盘 | 物理字母照常走全拼 26 键路径；14 键组字进行中，字母返回 unhandled，同九键 |
| iPad 与 2in1 | iPad 提供，键面不分体；鸿蒙 2in1 不提供 |

### 引擎：`KeyGrid` 泛化九宫格

`nine_key.rs` 新增 `pub enum KeyGrid { NineKey, FourteenKey }`，Copy 枚举加 match 取静态表，从 `lib.rs` 导出。它提供 `code_of`、`letters`、`is_code`、`encode`、`offers_raw_key`（九键真、14 键假）和 `spelling_table`（每种网格一个 `OnceLock`）。`NineKeySession` 加字段 `grid`，默认 NineKey；`encode`、`letters_spell_code`、`word_matches_digits`、`keyword_spells_digits`、`letters_for_digit`、`initials_for_digit`、`initials_codes`、`letter_prefixes`、`SpellingTable` 的构造、`key_choices`、`character` 都改走 `self.grid`。`SyllablePrior` 与网格无关，两种网格共用。

这一步先作为纯重构只接九键：九键的单测和 golden 一字不改，两份九键评测基线逐字节相同，`rerank_latency --nine-key` 的 p95 在噪声范围内。

14 键的组码选每组首字母的小写，于是它们与左列的大写字母项、数字项和 `'` 都不冲突，`choice_kind` 不用改。`reading_for` 改为正向判定：首行来自拼音路径（`canonical_pinyin` 非空，来源是词库、整句或生成的整句）才给读音；14 键首行是英文词时，读音是这个词覆盖已打键数的那段小写字母。Android 读音为空时会退回 preedit，而 14 键的 preedit 是 `bugao` 这种组码，会误导用户。九键下这个判定的结果与按字节形状判断等价，用单测钉住。

### `grid_key`：另开入口

`Session` 的 `nine_key_enabled: bool` 改成 `key_grid: Option<KeyGrid>`，新增 `set_key_grid` 和 `grid_key(letter)`。`grid_key` 只在 FourteenKey、全拼、没有本地模式、不在专用英文里、26 键预编辑为空或九宫格正在组字、按键是 `a`–`z` 时受理，受理后由 `code_of` 归一，宿主送组里哪个字母都行，约定送首字母。`set_nine_key_enabled(b)` 保留为包装，英文 T9 和注音九键只在 NineKey 下设置。`character` 的数字分支只在 NineKey 下进九宫格。

input-runtime 的 trait 加 `set_key_grid`、`grid_key`，新增 `Action::GridKey(u8)`；`Runtime::set_key_grid` 套用 `set_nine_key_enabled` 的校验（FourteenKey 只允许 quanpin，组字中切换报 `CompositionActive`）。host-api 新增 `msime_client_set_key_grid`（0 关、1 九键、2 十四键）和 `msime_client_grid_key`，`msime_client_set_nine_key_mode` 保留为包装。

### `View.key_grid` 的语义

View 新增 `key_grid: "none" | "nine_key" | "fourteen_key"`，`nine_key: bool` 仍只表示九键。runtime 有两道门改看 `key_grid != none`：`ChooseNineKeySpelling` 的过期判断，和未处理的 `'` 不转成标点。数字不当选词键这一条仍只看九键。`nine_key_spellings`、`nine_key_reading`、`nine_key_single_character`、`nine_key_strokes` 两种网格共用，字段不改名。

宿主用 `key_grid` 决定读音行、拼音选择条、隐藏组字和三栏面板，不能再用 `nine_key`：14 键下 `nine_key` 是假，点拼音选择条会被当成过期请求全部拒绝。

### 同步分期

- Android 直接上传 `platform.android.keyboard_layout = "fourteen_key"`；服务端接受 32 字节以内的任意字符串，旧版客户端遇到未知值只跳过这一个键。
- 鸿蒙导入端的 LAYOUTS 加 `fourteen_key`，这个键的未知值从「整份拒收」改为跳过。本版导出时本机是 `fourteen_key` 就省略这个键；上传按 merge 覆盖云端快照，省略等于保留云端旧值，旧鸿蒙不会拒收。下一版再开放导出。
- iOS 云端只有布尔键 `platform.ios.nine_key`，这一版 14 键上传 false，在别的设备上落成全拼 26 键；`platform.ios.keyboard_layout` 是 msime-cloud 仓库之后的改动。
- client-core 必须先于任何宿主认识 `fourteen_key`：偏好严格解析，未知值会让整份偏好读不出来。

### 评测决策点

engine 加入 14 键网格后，`verify-local.sh` 加两个集合：`fourteen-key`（`quanpin-words-v1.tsv`，`--limit 3000`）和 `fourteen-key-sentences`（`sentences-nine-key-v1.tsv`），基线是 `resources/eval/baseline-fourteen-key*.json`。`convert_eval` 和 `rerank_latency` 加 `--grid nine|fourteen`，`--nine-key` 是 `--grid nine` 的别名。

在同一 limit、同一词库下，14 键的词级 top-1、top-5 和长句 top-1 都不低于九键基线，`rerank_latency --grid fourteen` 的 p95 不高于九键，才开始做宿主。达不到先调常量（按网格分值的常量挂成 `KeyGrid` 的方法），调了仍不行就停在引擎这一层。

## Alternatives considered

- **14 键复用 `character` 送组码。** 最强的理由是改动最小：九键就是这样接的，宿主、C ABI、runtime 都不用加入口。但硬件键盘的字母也走 `character`（Android `onKeyDown` 之后的分支），用 `character` 送组码会让物理 q 变成有歧义的 QW 组，而物理 w 仍是确切字母；九键能复用 `character`，是因为拼音里没有数字。另开 `grid_key` 后，组码只能由触屏键面送进来。
- **另写一个 14 键解码器。** 理由是互不干扰，九键的热路径一行不动。但九宫格的路径搜索、锁定、切分、整句种子、简拼、英文混入、学习都与「字母怎么变成码」无关，只依赖码表；另写一份就是两套要同步维护的解码器，以后的九键改进（如 [九键整句改由数字层统一词网格解码](2026-10-09-nine-key-digit-lattice.md)）也要做两遍。`KeyGrid` 是 Copy 枚举加 match，不引入 trait。[九键双拼](2026-10-10-nine-key-shuangpin.md) 同样需要按方案构造 `SpellingTable`，它可以在 `KeyGrid` 上再加一种网格。
- **鸿蒙导出时把 `fourteen_key` 写成 `twenty_six_key`。** 理由是旧鸿蒙客户端一定读得懂，不会整份拒收。但上传按 merge 把本机的键覆盖到云端快照上，写 `twenty_six_key` 会把同一账号的其他鸿蒙设备切回 26 键；省略这个键等于保留云端旧值，同样不会被旧版拒收。
- **拼音选择条沿用 Android 九键的挂法，叠在候选行上。** 理由是现成代码（`attachSpellings(candidateViewport, true)`），14 键键面没有九键那样的左列空间。但它会盖住候选，用户在选拼音和看候选之间只能二选一；放在读音行右侧、横向滚动，两者同时可见，三端也能放在同一位置。
- **首版就做逐位确定字母（长按选中的字母约束这一位，而不是结束组字再上屏）。** 理由是更接近用户想要的「这一位就是 w」，长按不打断组字。但这要把 `initial` 推广成逐位约束，影响锁定、切分、种子和简拼的每条路径，范围与 14 键本身相当；首版沿用九键的长按行为（先结束组字，再上屏这个字母），逐位字母另行排期。

## Acceptance criteria

- E1：九键单测和 golden 一字不改并全部通过；`baseline-nine-key.json`、`baseline-nine-key-sentences.json` 逐字节不变；`rerank_latency --nine-key` 的 p95 在噪声范围内。
- E2：14 键单测覆盖编码表（由分组表逐字母生成并等于 `abcdedggujjlmbooqeatucqztz`）、421 个音节的码数（九键 218、14 键 314）、典型重码、`key_choices`、你好、西安、锁定和撤销、简拼遇到 UI 键返回 None、英文混入、读音行；golden 增加 `fourteen_key_session.json`；评测达到上面的决策点。
- 宿主：三端按同一份手工脚本验收——「你好」是 BN UI GH AS OP；AS GH UI 时选择条里有 shi 和 shu，选后锁定、退格撤销；分词键得到「西安」；长按 QW 选 w；英文切到 26 键再切回仍是 14 键，密码框画 26 键；设置 26→14→9→26 来回切，每次都持久化；外接键盘字母走全拼；九键和 26 键各打一段长句，与改动前一致。
- 三端键表一致性脚本比对 Android、iOS、HarmonyOS 三份键表与引擎的组码，表不一致时失败。

## Risks

- 九键回归：热路径上的拼写表单例和 `encode` 都改了，由 E1 的逐字节门禁兜底。
- 14 键歧义仍高（86 种重码，另有 sha 对 a'ga 这类跨音节歧义），常量是按九键调的，以评测决策点为准。
- 候选行的 `pinyin` 字段在 14 键下是组码字母，凡是把它当真拼音用的地方都会悄悄出错；宿主不能展示候选的 `pinyin`。
- 降级：选了 14 键后装回旧版，偏好严格解析整份读不出来。照现有的严格解析接受这个代价，写进发布说明；宿主绝不能先于 client-core 写 `fourteen_key`。
- 三端的非穷举判断（`== QUANPIN_NINE_KEY_LAYOUT`、`== .nineKey`、`=== 'nine_key'`）漏改一处，14 键就静默得到 26 键行为，宿主改动要逐处列清单。
- 14 键 golden 没有 C++ 参考实现，预期由 Rust 自己生成，只防回归，正确性以单测和评测为准。
- 主流输入法的 14 键交互细节没有在设备上核实，这里的规格以本仓库九键的行为为基准。
