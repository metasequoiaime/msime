# iOS 功能迁移对照

macOS 的同类文档是 [macos-parity.md](macos-parity.md)，方法一致：先做存在性比对定位可疑区域，再按行为逐条下钻。本文记录用什么方法比过、发现了什么，便于以后复核而不是重新发明一套。

**MSIME-Apple 的 iOS 功能已全部迁移。** 依据是下面九条机械比对轴，加上按来源「为什么这样做」的注释逐条回验行为——后者是唯一能看见「两边符号都在、行为却不同」那一类的方法，三个查实的真实缺口全部由它找到，没有一个是存在性比对抓到的。

## 范围与基线

迁移的目标是 MSIME-Apple 的 iOS 完整功能，而不是能编译的子集。公共业务放共享层、公共管理界面放 Tauri；输入算法与组合状态仍归 C++ Engine；平台特性按 iOS 自身的机制适配，不照搬来源的实现形态。

比对在固定对象上进行，便于复核：

- 来源：`metasequoiaime/MSIME-Apple` 的默认分支 `develop`，提交 `b93f169839c442cfa7034f3130c3dfaac11b9467`。
- 目标：`metasequoiaime/msime` 的 `develop`。

来源入口是该检出的 `platforms/ios/`，以及 `shared/apple-bridge/` 与 `shared/backend/`。

## 九条比对轴

| 轴 | 规模 | 方法 | 未命中的去向 |
| --- | --- | --- | --- |
| 源文件名 | 192 | 来源 `platforms/ios` 与 `shared` 下的 Swift/ObjC/C++ 文件按文件名在目标检索 | 集中在 `shared/apple-bridge/` 与 `shared/backend/`：前者是 ObjC 桥接层，由 `crates/host-api` 的 C ABI 取代；后者是 Swift 后端客户端，由 `crates/client-core` 取代 |
| 中文文案 | 1666 | 抽取来源全部源文件的中文字面量，逐条精确检索 | 测试断言文案、措辞差异、Tauri 页以不同表述覆盖、落在注释里的引号 |
| 符号 | 584 | ObjC 方法、C/C++ 函数与 Swift 函数名，按原名与 snake_case 改名两种形式检索 | 61 个在被 C ABI 取代的 ObjC 桥接层（`InputSessionAdapter`、`MetasequoiaInputSessionBridge`、`CandidateTranslation`、`MSIMEBackendClient`）；其余是改名或重构等价物 |
| 可达控件标识 | 254 | 抽取来源全部 `accessibilityIdentifier` 字面量逐个检索 | 2 个：`emojiCategory-` 在目标用连字符，自定义皮肤重置由 Tauri 页覆盖 |
| 同名文件成员 | 116 对 | 逐对抽取 `func` / `var` / `let` 名做差集 | 16 个文件有差集，逐个核实全是重构等价物：候选注解并成 `KeyboardCandidateAnnotation`、「更多」面板从布尔开关换成显式页枚举（之后又随全平台设计改成按 Android 顺序排的扁平工具列表 `KeyboardTool`，以分页网格显示）、表情目录从内存表换成分页 ABI |
| 测试断言 | 来源 232 | 来源 `test*` 函数名逐个检索，按名未命中的逐簇对到改名或拆分后的用例 | 目标 `KeyboardTests`/`ServiceTests`/`tests`/`TransportTests` 现有 312 个 `test*`，整体是来源的超集 |
| 设置面控件文案 | 219 | 抽取来源 `platforms/ios/App` 与 `shared/backend-ui` 全部 `Toggle` / `Picker` / `Button` / `NavigationLink` / `Section` / `TextField` / `Slider` 的中文字面量 | 2 条：`从相册选一张`（Tauri 皮肤编辑器的「选择照片」，webview 的 file input 直接给出共享偏好要的 base64）、`显示释义`（输入设置里叫「显示英文释义」） |
| 界面测试 | 来源 38 / 目标 39 | 逐个用例对照 | 组织方式不同：来源按屏拆四个文件，目标合在 `platforms/ios/UITests/` 的流程用例里，覆盖面相当 |
| iOS 偏好同步字段 | 8 | 字段逐个对照 | 全部对上，目标为超集（多出三个调频字段） |

上表是迁移时的基线。之后目标按全平台设计稿改版，加了来源没有的功能，不在上表的比对范围内：App 跟随四季的应用主题（Rust `msime_client_resolve_app_theme`，选择存在 App Group 的 `general.app_theme`，暂不随设置同步（Android 会同步），键盘「跟随系统」皮肤与品牌标也跟着季节换色），以及键盘的功能菜单——品牌键打开的分页网格，取代来源的分组「更多」面板。

## 存在性比对看不见什么

存在性比对定位的是「有没有这个东西」。**符号在两边都有、行为却不同**的那一类，它一个都抓不到——三个查实的缺口全是读来源在该处写下的「为什么这样做」才看见的。

`shared/apple-bridge/ShuangpinKeymap.h` 的注释就是这类线索的样子：

> Per-key double-pinyin hint text derived from the engine's own profile, so a frontend never hardcodes a keymap that can drift from the scheme the session actually runs.

目标这边 `shuangpinKeyHints()` 的符号在、调用链在、`KeyboardViewController.updateSchemeButton()` 上方的注释甚至也写着「来自引擎自己的 profile……不会与按键实际产出漂移」——但当时的实现是一张手写的四方案 Swift 静态表。注释描述的是来源的做法，代码做的是它警告过的那件事。

## 三个查实并修掉的缺口

### 双拼键面提示不能是手写表

把引擎的 `ShuangpinProfile` 逐键展开与那张手写表对比，两处偏差：

- **内容**：小鹤双拼的 `K` 同时承载 `ing` 与 `uai`，手写表只列了 `ing`，于是打 `guai`（`g`+`k`）的那个键上没有任何 `uai` 的提示。其余三个方案内容一致——但一致是运气，不是机制。
- **格式**：来源的 `" / "` 表示「声母 / 韵母」的分界，同一侧的多个单位用空格分隔；手写表把 `" / "` 当成通用分隔符，于是 `V` 读作 `ui / zh / ü`（三个并列项），而来源读作 `zh / ui ü`（声母 `zh`，韵母 `ui` 和 `ü`）；手道方案的 `E` 甚至排成 `e / sh`，把韵母排到了声母前面。

修法是按目标的分层把它接回引擎，而不是修那张表：`crates/engine/src/shuangpin/hints.rs` 的 `shuangpin_key_hints(profile)` 从引擎自己的双拼方案表展开（当初是已删除的 `crates/engine-bridge` 从 C++ Engine 的 `GetShuangpinProfile` 展开），`crates/host-api` 以 `msime_client_shuangpin_key_hints` 发布，iOS 键盘读这个 ABI。未知方案名返回空表而不回落到默认方案——给键盘贴上一套它没在跑的方案，比不贴更糟。这条路径同时对 Android 与 HarmonyOS 的触摸键面可用。

来源的 `uses_shuangpin` 门控在目标侧由 `View.scheme` 承担：引擎无论什么方案都带着一个 profile 被构建，所以 `View.shuangpin_profile` 任何时候都非空，只有 scheme 才说明键面是不是在跑它。

### 日语模式列的约束写法

来源在 `JapaneseNineKeyView` 里用一整段注释解释它**换掉**了哪种写法：把每个模式键写成 `modes.heightAnchor × span/4` 加常数，算出来的数是对的，但它是个环——子键的高度引用父 stack，而 `.fill` 的 stack 高度又由子键决定。iOS 26 的求解器凑出了那个唯一解，iOS 27 没有：123 比 ^_^ 高 2.33pt，三个键加缝隙把列撑出 2pt。来源改成只在兄弟键之间表达：单格键彼此等高，跨两格的等于两格加中间那道缝，总高交给 stack 自己的 fill。

目标这边原本正是被换掉的那种写法，已按来源改过来。这条缺陷在本机模拟器上不复现——它是求解器相关的环，不是算术错误，来源自己的注释也写着「算出来是对的」——所以这一项是按来源的证据移植来源的修复。加强后的断言（单格键彼此严格等高、三个键加两道缝正好等于列高）留在目标里，真溢出时拦得住。

### 模拟器手写面板要说自己不识别

ML Kit Digital Ink 的 arm64 切片只给 device，所以模拟器这份 `HandwritingInputViewFallback` 编译的是同一个面板但没有识别。问题不在「不识别」——那是诚实的答案——而在**它不说**：fallback 曾经只画笔画，没有任何状态文字，人写完一个字什么都不出现，这和键盘坏了无从区分。

来源在同一处明确给出 `此版本不含手写识别，请使用真机版本`。真机那份面板本来就有一条状态行（`handwritingStatus`）专门放这类消息，fallback 照同一套加上，读起来与来源一致。「fallback 不冒充识别成功」与「fallback 要交代自己没有识别」现在都有强制检查看着。

## 断言层修正过的三处

按名对齐来源的用例之后，目标侧有三条断言是检查本身陈旧，不是产品坏了，已按其真实契约改写。第一条和本节后面关于折线的几段记录的是迁移时的状态：那张「键盘设置」卡和「设置」开关分组已随全平台设计改版，由品牌键打开的分页功能菜单取代，`testBrandOpensCompactToolsAndUpdatesFeedbackState` 也随之删除，现在由 `NineKeyKeyboardTests` 的 `testBrandTogglesTheFunctionMenuInTheKeyArea` 与 `testToolbarIsTheDesignsEqualGridOfTools` 接替，`MorePanelFoldTests` 改为检查分页网格按 Android 的顺序翻得到每一个工具（`testEveryToolIsReachableByPagingInTheDesignsOrder`）。

- `testBrandOpensCompactToolsAndUpdatesFeedbackState` 仍在根页找「键盘设置」卡。那一层是有意删掉的——实现里的注释写明了理由（六张一样的入口卡，要再点一次才知道按键音开没开），而同一个用例后面又直接从根页读 `moreCard-按键振动`，自相矛盾。
- `testSchemePickerUsesCurrentSkinPalette` 期望选中卡片的填充是 `accent` 的 0.10 透明度，实现是 0.12。0.12 来自来源，是有意对齐的，断言没跟上。
- `testAdditionalEngineSchemesAndLocalProviders` 要求临时英文模式下 `hello` 给出多于一条候选。固定词库发布的 `msime-english.db` 里以 `hello` 开头的词正好只有一个，所以它断的是词库内容而不是产品行为，改成断言补全确实以输入开头。

第一条修掉表层之后，同一个用例往下跑露出被它挡住的第二层：「设置」分组的开关卡 `maxY` 到 332 / 386，而面板高度是 306，原断言要求每张卡都在折线以上。这一条没有按字面修，因为它和它所针对的设计相矛盾：面板是滚动视图，实现的注释写明这些开关是**有意**从「键盘设置」二级页搬到根页的，理由是那一层把状态藏了起来，而「面板本来就会滚动，分组标题也已经能区分两类」。

来源在 `KeyboardViewController` 里写的理由也是同一件事：八个本地模式和一份设置列表曾经和工具挤在同一个滚动里，最后一项落在 292pt 面板下方 360pt 处，「**没有任何东西说它们在那儿**」，于是模式被移进了二级页。那条理由说的是**有没有那个「说」**，不是折线本身——一个会滚动的面板，只要折线以上有东西说明下面还有，就不是它修的那种毛病。

所以当时断言改成检查设计真正保证的事，并由 `platforms/ios/KeyboardTests/settings/MorePanelFoldTests.swift` 看着（改版前）：五张工具入口卡与「设置」分组标题全部在第一屏，五个开关全部落在可滚动内容之内（落在内容之外的卡片是滚不到的）。那个分组标题一旦被挤到折线以下，开关就退化成来源移走的那种无人知晓的列表，用例会红。

## 范围外顺带补齐的一件

`smart_punctuation_repeat` 与 `smart_punctuation_space_convert` 曾经在 iOS 上没有任何消费方：macOS 与 HarmonyOS 都实现了，共享设置页对 iOS 用户照样显示这两个开关，打开之后什么都不会发生。

这**不是** MSIME-Apple 的迁移缺口——来源仓库里没有智能标点这个功能，它是本仓按 Windows 基线加的。修法按架构分两片走：策略下沉到 `crates/client-core`（和 `punctuation.rs` 的路由同处，两条规则从既有实现读出来而不是重新发明），再由 iOS 通过 `msime_client_smart_punctuation_arm` / `msime_client_smart_punctuation_decide` 消费。快照由宿主持有而不放进会话：它们属于宿主的编辑器，而 iOS 在键盘收起时会销毁重建会话，一个跨过那道缝还活着的手势是错的。

macOS 与 HarmonyOS 各自保留着自己那一份实现；让它们改用共享策略属于它们自己的改动。
