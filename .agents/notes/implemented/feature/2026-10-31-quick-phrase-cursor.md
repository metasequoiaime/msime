# Agent Note: 快捷短语上屏后定位光标

Status: implemented

## Problem

#10260 希望编码短语 `title:""` 上屏后光标直接停在引号内。现有快捷短语只有文本，上屏契约不含光标目标，宿主只能把光标留在文本末尾。仅解析标记而没有宿主执行，或者只在 K 模式接入而漏掉普通混输，都不能满足要求。

## Decision

在快捷短语正文使用 `${cursor}` 指定一个光标位置，例如 `title:"${cursor}"`；第一个未转义标记控制位置，其余同名标记按字面保留。`$${cursor}` 输出字面的 `${cursor}`，不设置位置。没有标记时所有既有行为保持不变，不修改默认值、词库格式或短语持久化结构。

纯文本解析归 Engine 文档化 API。runtime 只对已接受的快捷短语候选（来源 `QuickPhrase`）展开；K 模式、混输、数字、点选、展开面板与高亮结束共用选择事实，原样提交和以词定字不触发。候选预览显示展开后的文本，身份、学习与原始词条仍沿用原值。上屏的可选 `commit_cursor_suffix` 字段保存光标后那段文本（可为空字符串，缺字段表示无定位请求），宿主按自己的文字单位定位，不在共享层猜 UTF-16 或按键步数。

host-api 对上屏做全角、空格处理时同步转换尾段；格式化前后都核对正文后缀，失配时丢弃位置请求。光标边界新增的中英文空格留在光标前，使光标仍紧邻原后缀首字符。脚本实际改写了提交正文时清掉旧定位，不对改写后的文本沿用过时目标。K 模式仍遵守现有的不可改写限制，普通混输短语按普通上屏的授权边界处理。AI 上文只记光标前的实际提交文字。

Android、iOS 与 macOS 原生宿主消费 `commit_cursor_suffix`，Android、iOS 与共享 React 短语表编辑器提供语法说明；Windows、Linux 与 HarmonyOS 宿主尚未完成，#10260 仍未完成，PR 必须保持草稿。macOS 使用读回后的空标记定位，不持有任意短语尾段；跨进程 IMK 与第三方编辑器仍需验收，Linux 的框架版本与转发方向键顺序也尚未验收。共享解析测试不构成任何平台的编辑器光标验收。

K 模式的真实引擎收到数字时，现有字符接口先结束原文，而不是选择候选；这一行为保持不变。数字选词只在引擎明确交还数字且没有产生提交时展开所选短语，不能覆盖引擎已经交出的原文。

## Android 的批次、读回与回声

`commitText(text, 1)` 完整提交后先结束批次，再同步 `getExtractedText` 核对光标前的完整提交文字、折叠选区和提取偏移。仅完整快照可以定位，增量片段、未知位置、截短或改写都保留原提交。正文与后缀先经过同一份宿主简繁转换，转换后失配不定位。`startOffset + selectionStart - suffix.length()` 给出 UTF-16 绝对位置，以 `setSelection` 定位，不按码位数发送方向键；空后缀沿用普通提交路径。

同次转换还有新组字时，定位先于新组字；两批之间不会读到尚未应用的文字。定位失败不报告整次提交失败，避免已上屏的文字被重试而重复。`SelectionEchoTracker` 在同步核对后的绝对位置重新定基，但保留较早的提交回声，让提交、定位及后续组字的异步回报都能认作自己的写入；未知读回不伪造位置。

设备用例在隔离会话显式请求混输能力，验证该响应也能定位；Android 产品仍按原有声明只开放既有的 K 模式入口，不借本次光标接入扩张混输能力或翻转默认值。

Android 的 `commitText` 参数不能表示正文内部的位置，而编辑器可以在 `endBatchEdit` 才应用写入。API 语义依据 [InputConnection](https://developer.android.com/reference/android/view/inputmethod/InputConnection#commitText(java.lang.CharSequence,int)) 和 [ExtractedText](https://developer.android.com/reference/android/view/inputmethod/ExtractedText)；后者的选区相对提取文字，`partialStartOffset != -1` 表示增量更新，不能当作完整快照。代价是多一次同步读回与跨进程定位，编辑器不支持读回时能力降级。

## iOS 的临时标记与提交选区

有效非空后缀使用 `setMarkedText` 一次交付完整正文和相对正文的 UTF-16 折叠选区，紧接 `unmarkText`，沿用原有行内组字替换通道。取消旧标记后另行插入正文会多出一次文档变更，刚结束组字时的选区回声可能还没更新；完整正文和选区一起交付可以避免拆开写入。既有选区、旧组字和同次下一段组字都由 UIKit 维护，新组字必须在临时标记已提交的位置开始。位置请求清掉成对标点栈，不把任意短语后缀误记成标点尾巴。

正文和后缀经过相同的宿主简繁转换，遵守 `script_conversion` 和日语来源的转换边界；转换后按 UTF-16 单元核对后缀。Swift 的规范等价字符串比较会把不同长度的预组合与分解形式视作相等，不能用于这个位置边界。缺字段、空后缀或失配沿用原有提交路径。`replacingCommit` 仅对相同 UTF-8 正文保留位置，真正改写正文就清除后缀。

公开 API 依据 [UITextDocumentProxy.setMarkedText](https://developer.apple.com/documentation/uikit/uitextdocumentproxy/setmarkedtext(_:selectedrange:))。初次 UIKit 探针在同一事件中既设置基础文档选区又立刻替换标记，观察到位置错误；把基础选区建立与待测提交隔开后，48 组探针包含同事件建立旧组字、一次／两次完整标记、Unicode 后缀和下一段组字，全部得到正确位置。产品路径没有新增延迟、重试或第二次标记；正式控制器回归分别覆盖同事件和已定稳的旧组字。进程内原生控件测试之外，跨进程系统扩展用例另行验证独立 UITextField 中的真实插入点。

iOS 产品维持原有 K 模式入口，没有新增普通混输能力声明或修改默认值。

## macOS 的已提交文档范围

普通 `insertText` 先完整替换旧组字，跨提交空格 observer 随后完成格式化；再经 `MSIMETextClientCommittedRange` 核对全文紧邻折叠光标，只在上下文可靠且转换仍有效时按 UTF-16 绝对范围定位。先在目标位置 `setMarkedText` 空文本，再 `insertText` 空文本结束临时标记，最后才应用同快照的新组字。客户端不提供上下文、拒绝范围、忽略范围或改写正文时不重试原提交，保留已交付文字。每次同步调用后检查派发代次和客户端身份，重入的新组字由较新的派发持有。定位另查按键代次，透传键未产生新转换时也不能沿用旧文档位置；普通预编辑保护保持原有转换代次边界。

正文和尾段一起执行宿主简繁转换，转换前后均核对后缀，转换后的巧合不能修复原本失配的请求。显式模板不触发自动补另一半标点；已有成对闭合符仍完整提交，定位时把它计入尾段。不把任意模板后缀塞进 `_pendingPairedClosing` 或标点栈，也不新增失焦时需刷写的短语状态。跨提交空格只在正文左边新增前缀，读回仍可核对原正文的结尾范围。

API 范围语义以 Xcode SDK 的 `HIToolbox.framework/Headers/IMKInputSession.h` 中 `IMKTextInput` 为准：`replacementRange` 相对文档开头，不支持 TSMDocumentAccess 的客户端可以忽略它。初次真实 AppKit 探针中，直接以空 `insertText` 指定范围的 16 组均没有移动光标；空标记加空提交的 16 组全部通过，包括 Unicode、选区、旧组字与下一次输入。这一实验只证明 AppKit 控件语义，不能证明跨进程 IMK 是否转发空标记。

## HarmonyOS 的异步布局与绝对选区

宿主消费 `commit_cursor_suffix`，正文与尾段同步简繁，转换前后均核对匹配。非空有效尾段走同步完整提交；空尾段、缺字段和失配沿用普通提交。`QuickPhraseCursor` 先保存光标前后各不超过 4096 个 UTF-16 单元的完整上下文，旧预编辑能从左文核对时扣除其替换范围，再交付一次完整正文。读回必须等于原左文加正文、原右文，且绝对光标等于完整左文长度，才能按 `selectByRangeSync({ start, end })` 发出折叠选区。完整前后文的代价是较长文档或限制上下文的编辑器只上屏、不定位；不猜截短快照的文档偏移。

同步 API 返回不等于布局完成。OpenHarmony 的 `InsertText` 把任务交给 UI 线程，`TextFieldPattern::AddInsertCommand` 还把写入排到布局队列；同步 `GetLeftTextOfCursor` 只读当下文字，并不排空布局命令。因此立即读回仍是旧文档时保留当前请求，等待自有写入的 `textChange`／折叠 `selectionChange` 回声后再核对和定位；定位也要读回前后文与目标绝对位置才能完成。`syncPreview` 在请求结束前不写下一段组字，候选视图仍更新，后续没有新提交的按键所产生的最新预编辑会在确认位置后写入。

所有写入与定位仍经 `editEditorSync` 记回声；不另开一套猜测选区的账本。失焦、会话重建、外部修改、其他编辑器写入或一秒超时立即清除请求，迟到回声不再跳回旧位置。超时或拒绝不重试正文，不删除已提交的文字。同步调用重入时较新的转换和客户端持有预编辑，旧调用不能清掉新标记、结束新请求或覆盖新视图。任意短语后缀不进入成对标点生命周期，产品混输声明与默认值保持。

API 与队列依据为 [IME Kit](https://raw.githubusercontent.com/openharmony/docs/master/en/application-dev/reference/apis-ime-kit/js-apis-inputmethodengine.md)、[UI 线程适配](https://raw.githubusercontent.com/openharmony/arkui_ace_engine/master/frameworks/core/components_ng/pattern/text_field/on_text_changed_listener_impl.cpp) 与 [TextFieldPattern](https://raw.githubusercontent.com/openharmony/arkui_ace_engine/OpenHarmony-6.0-Release/frameworks/core/components_ng/pattern/text_field/text_field_pattern.cpp)。纯逻辑回归分别覆盖「读回排空 UI 任务」和「读回仍看不到布局写入」两种编辑器；设备验收另记实际结果，不能把同步 API 名称或 Node 模型当作系统保证。

## Windows 的固定包长定位协议

协议预备层在 `quick_phrase_cursor_pipe.h` 定义可选的 `QuickPhraseCursor` 能力，保留 Main 的 416 字节、Worker 的 404 字节和正文 199 个 UTF-16 单元容量。`msg_type` 的最高位表示定位扩展，低 8 位保存原提交类型，8..23 位存尾段 UTF-16 单元数，24..30 位保留为零。只允许 Main 的 `Normal`／`CommitExactText` 和 Worker 的 `CommitCurCandidate`／`CommitCandidateAndContinue` 携带。零单元是显式末尾定位，不能当成缺字段，否则模板正文可能再次触发自动补成对标点。

解包必须同时验证双方已协商、提交类型、保留位、终止符、正文长度及代理项边界；损坏扩展不能去掉标志后当作普通提交。编码先核对真实正文后缀，错误元数据只丢定位。空 Worker 正文沿旧协议表示消费另一条回复，只有有效显式定位帧才能表示空提交；错误定位不能退回这个触发语义。`EngineResult` 仅在存在实际字符串提交且后缀匹配时保存可选字段，每次解码不沿用前一次结果。

`ReplyComposer` 从当前焦点租约接收定位能力，只向已协商的连接编码位置。键盘完整选词、标点提交、候选窗完整点选及五笔继续提交都保留尾段；TIP 已经上屏的本地字符／音节路径仍只回后续组字，不另投正文。部分选词和释义替换不继承旧位置。完整提交中有效的空尾段与空正文都保留显式定位，避免仅含 `${cursor}` 的模板被当作没有提交；旧连接维持原回复。候选窗选择在移动 `transition` 所有权前先读取 `next_view`，不依赖移动后的引用。

正文与尾段使用同一份 `commit_conversion` 简繁规则，转换前后都必须匹配。比如「發」正文配「发」后缀，即使转换后都成为「發」仍不能恢复原本失效的位置；「头${cursor}发」转换后正文是「頭髮」、单独尾段是「發」，后缀不再匹配时只提交文字。代价是这类上下文转换不定位，避免按错误尾段跳转。

Server 与 TIP 均尚未宣告该能力，因此生产连接不会收到扩展，Windows 能力仍未完成。后续必须接通 TSF 编辑会话、Worker 与 TIP 本地提交，并在同一次编辑会话核对实际全文后用 `ITfRange` 定位，不能靠方向键次数或在定位失败后重投正文。

回复合成器与纯 codec 套件加入跨宿主 runner。回复合成器使用真实共享宿主库，不能因缺少伴随链接单元而悄悄跳过。既有语音用例的非 BMP 字符改用显式 UTF-16 代理项作输入和预期，保持 Windows 的原始字节断言，并让 32 位 `wchar_t` 宿主也检查同一份线格式；不以平台排除隐藏失败。

协议与 JSON 解码回归分别用 macOS clang 和 MinGW x64／i686 编译；两套同源 Windows EXE 已在 Windows 11 原生 runner 执行，显式传播退出码，均返回 0。codec 覆盖固定线长、完整正文容量、Unicode、空尾段、能力交集、损坏元数据、无终止符与代理项边界，以及空正文的坏定位不能变成旧触发帧、Worker 计数前缀不能冒充正文后缀。JSON 覆盖缺失、空、错误类型、失配与无提交时的清理。该结果只证明协议和解码，不代表 TSF 编辑器位置或完整 Windows 宿主验收。

回复合成器的新增回归在 macOS 上以 `-Wall -Wextra -Werror` 编译并运行通过，最终夹具没有运行时配置回退警告。隔离插件表经真实 Engine／Host API／`ReplyComposer` 的 70 组入口矩阵覆盖空格、数字、回车、标点、点选、展开网格和旧连接，含 Unicode、显式空模板、末尾位置、上下文敏感简繁转换及已转换正文的 `script_conversion=false` 边界。纯回复回归另覆盖错误／缺失／失配位置、部分选词不传播、Worker 继续提交与 TIP 已上屏路径不重复交付。

此前协议切片的 `verify-local.sh --quick` 退出 0，包含 Windows 跨宿主 204 项、Linux 原生 113 项、HarmonyOS 设置页与 HAP、Rust／Android／Wasm 检查、macOS 原生宿主与 Apple bridge、x86_64／i686 管道协议编译。完整 Windows 原生项目依赖未配置，该阶段跳过，不计作产品构建。首轮 SSH 缺少 Node 和 DevEco SDK 环境而失败；补齐机器已有的 Node／pnpm、`NODE_HOME`、`DEVECO_SDK_HOME` 和 Java 21 后完整重跑通过，没有改测试断言或失败基线。

## 既有笔记审计

- [快捷短语混输](../../implemented/feature/2026-10-17-macos-quick-phrase-candidates.md)、[快捷短语偏好](../../implemented/feature/2026-10-22-quick-phrase-preferences.md)、[首页默认席位](../../implemented/feature/2026-10-25-quick-phrase-first-page-default.md)：部分重叠，保留能力、默认值、排名和学习决定，本篇只扩展选中后的文本交付。
- [光标后编辑](../../implemented/feature/2026-10-26-caret-suffix-candidate-edit.md)：无关，那里移动的是组字中的原文光标，不是短语上屏后的编辑器光标。
- [中英文间空格](../../implemented/feature/2026-10-12-chinese-english-spacing.md)、[脚本修改候选与上屏](../../implemented/architecture/2026-10-12-script-plugins-modify-commit-and-candidates.md)：部分重叠，本篇需保证格式化或插件改写不沿用过时位置，原有转换规则不变。
- [移动端默认数字页](../../implemented/feature/2026-10-15-mobile-default-number-pad.md)：部分重叠，保留 iOS 旧组字和字面正文一次替换的时序，本篇增加显式相对选区，不拆成删除旧组字与另行插入两步。

## Alternatives considered

- **新增数据库的光标偏移列**：可把位置与正文严格分离，不用转义。但现有用户词典、导入导出、短语表插件、账号同步和各设置页都只有正文，新增列会迫使全部数据路径升级，文本标记可直接沿原通道保存与同步。
- **Engine 的每个 `KeyResult` 增加光标整数**：所有引擎嵌入方都能直接消费，但多段提交、标点尾巴和宿主格式化都要逐处调整偏移，整数还容易混淆字节、标量、UTF-16 与字素。选择事实已经能确认短语来源，runtime 仅把共享解析结果交给宿主，后缀文本可在最终变换后再次核对。
- **每个平台各自解析正文**：不改共享提交结构，但转义、来源边界和候选预览会产生多份规则，K 模式和混输也容易遗漏。不采用。
- **Android 在原批次里设置光标，或用方向键左移后缀长度**：调用更少，且方向键能覆盖不支持选区接口的编辑器。但批次内读回可能是旧快照，方向键按字素、双向文字和换行移动，不能从 UTF-16 或码位长度推出步数。不采用；无法验证目标的编辑器保留文字和默认位置。
- **iOS 完整插入后调用 `adjustTextPosition(byCharacterOffset:)`**：可直接复用普通提交，不需要临时标记。但该 API 只以 character 描述移动量，没有给出将任意后缀的 UTF-16、Unicode 标量或字素数映射成跨编辑器步数的保证；双向文字还使方向移动更难核对。临时标记的相对选区直接指定正文内部边界，无需猜测移动次数。
- **macOS 长期持有任意模板尾段**：可以复用成对标点的 marked text 尾巴，不依赖绝对范围。但是任意尾段会跨越直接英文、语音、剪贴板、维护会话与失焦等所有上屏路径，任何遗漏都可能覆盖或丢失文字；不能把它冒充闭合标点。空标记方式在可靠文档范围内定位，全文已落地后无需额外尾段所有权。

- **HarmonyOS 同步插入后立即设置选区**：调用少、没有额外等待状态。但同步插入只把操作送进 UI／布局队列，选区可能早于文字落地，立即读回也可能还是旧文档；真实模拟器中的快速输入已观察到该时序。采用有界回声恢复，并阻止下一段预编辑抢先写入。
- **Windows 扩大回复结构或把定位写在正文前缀**：扩大结构能直接表示字段，正文前缀也不用占用消息类型位。但前者破坏固定管道 ABI，后者减少现有正文容量；消息类型的有界可选扩展保留两者，并由协商阻止旧端收到。
- **Windows 新增分帧定位消息**：定位能使用独立消息类型，也便于以后扩展更多元数据。但任意提交需要新增跨帧重组、焦点归属与失败回收状态；当前尾段上限已由正文容量界定，放在同一帧足够，不增加另一份待提交所有权。

## 共享层验证边界

- 共享回归覆盖普通短语、单标记、首尾位置、转义、重复标记、未知变量、中文／扩展字符／组合字素后缀，以及非快捷短语来源不解释标记。
- K 模式、普通混输、各种选择路径与带标点结束在真实运行时产生正确文本和尾段；原样提交、以词定字、失效候选不产生定位。
- 全角、空格、脚本改写后目标有效；AI 上文只接收光标前已展开文本。原样提交与后续字面标记不属于待删除的模板语法。
- Windows、Linux 宿主仍未完成；HarmonyOS 已接线，实际系统验收另列；Android 完整系统入口与各平台真机验收仍有缺口；共享层与已接入平台分片完成不能标记整个 issue 完成。
- 本地回归：Engine 快捷短语 25 项、runtime 全部 424 项、host-api 光标定位 3 项与自动空格 8 项通过；相关三个 crate 的全目标 clippy 无警告。笔记与差异门禁通过，quick 门禁及 PR 检查单独记录在 PR 验证段。
- Android 的 `check-host.sh` 通过 316 项 JVM 冒烟与 256 份设备源码编译；本分支同源 Rust／JNI 与原生 APK 构建、ELF ABI／16 KB 对齐检查通过。API 28、35 的 `CompositionBatchDeviceSmoke` 均通过：批次外读回、非零提取偏移、emoji／组合字符／双向文字、选区与组字替换、原生 LengthFilter、未知／增量／改写快照、拒绝／断连定位、迟到回声均有回归。隔离短语表经安装包真实 Engine、JNI、正式 `MSIMEInputService.apply` 到原生 EditText 的 K 模式及普通混输均定位成功，后续宿主提交写在目标位置。该项属于模拟器内宿主与真实控件验收，未声称覆盖其他应用、系统输入法完整入口或真机。
- iOS 同源模拟器静态库与原生 App／扩展构建通过。iOS 26.5 的 `InputBridgeResponseTests` 14 项、`QuickPhraseCursorKeyboardTests` 5 项、既有 `PairedPunctuationTests`／`InlineCompositionTests` 13 项全部通过，包括字段解码、正文改写清定位、规范等价但实际编码不同的边界、同事件旧组字替换、同快照的新组字、简繁转换与日语／已转换正文边界；隔离短语表经真实 Engine、Swift 桥、正式控制器到原生 UITextView 后，下一段组字仍写在后缀前。系统 UI 测试 1 项实际通过、未跳过：从正式设置页创建并启用模板，Settings 添加扩展与完全访问后，在独立测试应用的普通 UITextField 选词上屏，下一键 x 得到 `title:"x🌲é"`。该项覆盖模拟器的系统扩展入口与独立原生编辑器，未声称覆盖真机或第三方应用。简繁用例同时设置并恢复权威共享文档和 App Group 镜像，避免镜像被加载时的文档覆盖而误判产品转换。
- macOS 控制器到真实 `NSTextView` 和 `NSTextField` field editor 的 64 组回归通过，覆盖 UTF-16 后缀、选区、旧组字、同快照下一段组字和后续提交；另含简繁、跨提交空格、模板与已有成对尾巴组合、首尾位置和失配降级。隔离短语表经真实 Rust Engine、Apple 桥、正式控制器到原生编辑器，K 模式与显式启用的普通混输均定位成功，下一键仍在模板尾段前。共享文本客户端覆盖四个同步阶段的重入与未知／拒绝／忽略／改写上下文等五类降级。该层是进程内真实 AppKit 控件验收，尚未覆盖跨进程 IMK、第三方编辑器或安装验收。

- HarmonyOS 的 `tests/run.sh` 全套通过，新增 28 组 Unicode／选区矩阵及预编辑替换、简繁、拒绝／忽略／改写、异常、超长上下文、重入和分阶段布局回归。同源 `build-native.sh arm64-v8a`、共享设置页构建、`ohpm install` 与实际 `hvigorw assembleHap --no-daemon` 通过。
- HarmonyOS 6.0.2（API 22）独立 2in1 模拟器：隔离测试包只播种合成短语表、驱动按键和提供原生 TextArea；真实 Engine／NAPI／正式 `KeyboardSession.apply` 经系统输入客户端定位成功，`前正文${cursor}😀后` 的下一段 `n` 在尾段之前组字。普通等待与选词后同事件立即输入 `n` 两种场景均通过；后者实测提交后仍在等待布局，证明恢复路径实际执行。没有改普通混输声明；未覆盖手机、真机、第三方编辑器或物理硬件键路由。

## Consequences

跨进程 IMK 的空标记和 Linux 定位接口需要独立验收，不能以编译成功代替真实光标位置。本机与编译机主卷都不足 50 GB，Apple 与 quick 验证只复用既有产物目录；HarmonyOS 的同源冷构建放在空间充足的外置卷独立检出中，仍使用平台规定的产物目录。接口落地分片提交，所有宿主接入前保持草稿并明确未完成项。数据结构不增加数据库列，但旧宿主会忽略可选后缀字段，因此这份共享切片不能单独作为用户可用能力合并。
