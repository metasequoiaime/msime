# Agent Note: 拼音后用部件全拼筛选候选

Status: implemented

## Problem

Issue #11804 请求在全拼输入后按反引号进入部件全拼辅助筛选，例如 `cheng` 后输入部件读音筛出目标字。报告环境为 macOS 27.2、水杉 0.60.1，目标是让普通全拼用户不必记传统辅助码。

仓库原有一条不同的能力：Shift+C 进入共享引擎的 `LocalInputMode::Chaizi`，随后按部件全拼查询编进引擎的 `chaizi.tsv`（`chaizi-chars.tsv` 提供名次、读音和繁体字形），可输入多个部件并用撇号分隔。它从空闲状态开始，候选是拆字反查结果，不能接在已经完成的普通拼音组字后，也没有把结果限制为当前拼音候选的筛选操作。

## Decision

全拼组字后按反引号打开一段「部件辅助段」，后面打部件的全拼，候选只留末字由这些部件组成的行。开关是共享偏好 `quanpin_helpcode.component_filter_backtick`（引擎 `SessionOptions::component_filter_backtick`），默认关，缺字段读作关；这是维护者在 #11804 上的决定，没有授权默认开。只做全拼，双拼的前缀规范化留到以后。

### 第 1 步：引擎里的部件筛选

- `local::chaizi` 第一次筛选时从 `chaizi.tsv` 建一份「字 → 所在行号」的反向索引，按字排序的 `(char, u32)` 数组，之后二分查找。一个字有几种拆法或部件别名就有几项（撑 有 `cai'zhang`、`shou'shang'shou`、`shou'zhang`、`ti'zhang`），任一种对上就算命中。
- `component_filter(candidates, parts)` 看每个候选的末字（多字词看末字），保留末字的某种拆法能按顺序对上辅助段的行，顺序不变，其余丢掉。辅助段去掉 `'` 后为空时原样返回；含小写字母和 `'` 以外的字符时什么也不留；末字不在拆字表里（含非汉字）的行丢掉。
- 匹配规则：把辅助段的字母切成若干段，每段等于键里的一个部件读音，用到的部件在键里的位置严格递增，可以跳过部件，不能颠倒，也不能重复用同一个部件；用户写下的 `'` 必须落在两段之间。切分由键里实际的部件决定，`xian` 既能对上部件 `xian`，也能对上 `xi'an`。实现是按辅助段位置的动态规划，每个位置只记「下一段最早能用第几个部件」，不会因重复部件（`ma'ma'ma`）指数回溯。
- 只认完整的部件读音，不做前缀补全：`cheng` 后打 `` `ch `` 不会先筛出 蛏，筛空时候选栏是空的，用户接着打完部件。

### 第 2 步：会话里的辅助段

- 触发条件（`InputSession::component_filter_backtick_ready`，`session/component_filter.rs`）：选项开着、方案是全拼（不含五笔混输和 `z` 临时拼音反查）、不在本地模式和专用英文里、有组字、没有整句改字和光标后编辑、光标在末尾、组字原文只有小写字母和 `'` 且是完整拼音（`is_complete_pinyin_input`）、反引号没被设成辅助码前导键。`spelling_symbols` 此时发布 `` ` ``，`handle_punctuation` 把它转给字符入口，`dispatch_character` 把它作为 `SchemeKey::Symbol(b'`')` 交给 `QuanpinScheme`；三处用同一个判断，发布的键就是接受的键。条件不满足时反引号照旧走标点路径（`punctuation.rs` 的「·」），选项关着时整条路径与以前完全相同。
- 反引号记进全拼输入串，一段组字只有一个，后面的字母和 `'` 跟着进串，退格逐字删回去，删掉反引号就回到普通组字。预编辑和 `editing_text` 是原串（`` cheng`chong ``），`raw_segmentation` 在切分后面原样接上 `` `chong ``。
- 剥离在请求构建这一个入口完成：`QuanpinScheme::build_request_from_raw_into` 在第一个反引号处把原串分开，`raw_input`、`raw_input_with_cases` 和由它们派生的 `normalized_input`、各种切分只有反引号前的拼音，原串放进 `raw_input_with_helpcode_marks`（双拼辅助码段已经这样用），部件段小写后放进新字段 `QueryRequest::component_filter_parts`。issue 评论里列的读音消费点（`quanpin/engine.rs` 的辅助码检测、`quanpin/scheme.rs` 的切分、`session/composition.rs` 的选词推进、`session/online.rs` 的云候选键、`pinyin/autocorrect.rs` 的纠错抑制键）都读这些字段，因此自动只看到拼音，不必逐处再剥。读原串（`raw_with_cases()`）的只有预编辑、编辑光标、字面输入判断和网址、超级简拼这些入口判断，它们对带反引号的串一律不成立。
- 筛选：`QuanpinEngine::query` 在原有查询（含大写辅助码分支）之后按 `component_filter_parts` 调 `component_filter`；单字母音节（`a`、`e`）先用 `complete_initial_candidates` 补全首屏，`expand_initial_candidates` 在筛选时不再追加未筛的行。会话的混排列表在混入快捷短语、表情等之后再筛一次，显示列表和学习用的未重排列表用同一条规则，`ranking_index` 仍对得上。
- `has_active_helpcode` 在有部件段时为真，于是英文、符号、联系人、日文与多语言混排、滑行和新的云候选查询都让位，与大写辅助码一致。反引号刚打下、段里还没有字母时不算，候选和云候选照常。
- 上屏和学习：选中筛出的行只上屏那一行，选走一部分时剩下的拼音不再带部件段（与「辅助码选中了这个词，剩余输入不再保留该辅助码」相同）。回车原样上屏和筛空后的结束组字走 `raw_commit_text`，在有部件段时只给反引号前的拼音。学词、调频和造词用的都是候选的读音和请求里的拼音，学习日志和词库里不会出现反引号或部件串。

### 第 3 步：宿主与设置

- 偏好：`HelpcodePreferences::component_filter_backtick`，`#[serde(default, skip_serializing_if = "is_false")]`，只在 `quanpin_helpcode` 里读，`shuangpin_helpcode` 里的同名键不起作用。host-api 在应用偏好变更（`apply_pending`，会重建会话）和首次建会话（`into_engine_options`）两处都直接读 `preferences.quanpin_helpcode.component_filter_backtick`（不经 `helpcode_for_scheme`），所以切到双拼再切回全拼时仍是同一个值。它不依赖 `enabled` 和辅助码表。
- 选项链：`EngineOptions::component_filter_backtick` → `SessionOptions::component_filter_backtick` → `ImeSession::set_component_filter_backtick` → `apply_request_options`（选项关着时清空 `component_filter_parts`，宿主改写进来的反引号只显示、不筛）。宿主经 host-api 的 C ABI 传偏好文档，不需要新的 C 函数或结构体字段。`crates/engine-capi`（给第三方的 C 库）的选项 JSON 没有加这个键：加键要把 ABI 升到 1.2，属于另一次发版决定。
- iOS：原生「辅助码」页的全拼一节加「反引号后按部件拼音筛选」开关（`HelpcodeSettingsView.swift`，标识 `quanpin_helpcode.componentFilterBacktick`），合并写入 `quanpin_helpcode` 对象，说明进「?」。和双拼的反引号触发一样，触屏键盘上的反引号直接上屏，要用外接键盘。
- macOS：AppKit 回退设置窗口冻结、共享 React 设置页只修缺陷，所以这次不加界面。`scripts/test-preferences-field-parity.py` 把它登记在 `HelpcodePreferences` 的 `RUST_ONLY` 里；`platforms/macos/tests/settings/preference_coverage.py` 认为 host-api 读了它，不需要豁免。macOS 用户在界面接入前手动打开：在共享偏好文档（设置应用的 `preferences.json`）的 `quanpin_helpcode` 对象里写 `"component_filter_backtick": true`，输入法在下次读到偏好时重建会话。
- 其余平台（Windows、Android、Linux、HarmonyOS）读的是同一份偏好和同一个引擎，打开字段即可用，设置入口随各自的原生设置跟进。

## Alternatives considered

- 直接复用 Shift+C 拆字模式并改入口键。它不能限制到当前拼音候选，且会改变现有拆字模式的入口语义，因此不满足需求。
- 只在 macOS 候选窗中按部件表过滤。其他平台会得到不同候选，宿主也会复制引擎字形和繁简规则，因此不采用。
- 把部件拼音编码写入现有辅助码表。传统辅助码是固定短码，部件全拼允许多音节和任意部件顺序，混用会破坏校验、编辑和同步语义，因此不采用。
- 先用通用的全拼音节切分器把辅助段切成音节，再逐个和部件比。它和「切成全拼音节」的描述最直接对应，但 `xian`、`fangan` 这类串有多种切法，切分器只给一种，会漏掉拆法写成 `xi'an` 的字；改为由键里的部件决定怎么切，就不存在选错切法的问题，因此不采用。
- 部件段不进输入串，在会话里另存一个字段。这样拼音原文天然干净，任何读 `raw_input` 的地方都不可能带进部件串；但预编辑、编辑光标、退格、宿主改写原文和 `editing_text` 都得各自再拼一次，宿主看到的组字也和引擎持有的原文不一致。双拼辅助码段已经有「原串进 `raw_input_with_helpcode_marks`、去段串进 `raw_input`」的现成形状，沿用它只需在请求构建处剥一次，因此不采用另存字段。
- 在 issue 评论列的每个读音消费点各自剥离（像大写辅助码那样到处调用 `strip_active_helpcodes`）。这是评论里描述的做法，但漏掉一处就会把部件串带进学词或云请求；在请求构建处一次剥掉之后，这些消费点不需要改，新的消费点也不会漏。
- 默认开。部件筛选对不会读的字很有用，但打开后全拼组字里的反引号不再出「·」，会改变现有用户的手感；维护者在 #11804 定为默认关。

## Consequences

- 选项关着时，反引号、候选、预编辑和上屏与以前逐字节相同（`session/tests/component_filter.rs` 的 `backtick_is_punctuation_while_the_option_is_off`）；`QueryRequest` 多一个字段，全拼请求每次构建多一次字节查找，没有额外分配。
- 打开后，全拼组字末尾的反引号不再出「·」；要打「·」先上屏或在拼音不完整时按。筛空时候选栏是空的，结束组字和回车只上屏反引号前的拼音。
- 部件只认完整读音、不做前缀补全，边打边筛时前几个字母会先筛空；以后要补全最后一个部件，改 `component_filter` 一处即可。
- `has_active_helpcode` 的含义扩大到部件段，读它的混排和云候选门禁一起生效；以后给它加新用途时要记得部件段也会让它为真。
- 验证：`msime-engine` 的会话测试覆盖进入、退格、标点入口、触发条件、上屏、学词、云候选和默认关；`quanpin::scheme` 的单测钉住请求字段；第 1 步的筛选单测仍在 `local/chaizi.rs`。iOS 开关只做了 Swift 语法检查，没有在本地编译 App，也没在设备上点过；macOS 没有界面，真实编辑器里的验收（外接键盘打 `` cheng`chong ``）还没做。
