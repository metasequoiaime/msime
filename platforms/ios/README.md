# iOS App 与键盘扩展

## 目录结构

App 业务位于 `App/Sources/<feature>/`，键盘扩展位于 `KeyboardExtension/Sources/<feature>/`，共享 SwiftUI/UIKit 位于 `SharedUI/<feature>/`；`KeyboardTests/`、`ServiceTests/`、`TransportTests/` 和 `UITests/` 分别覆盖键盘、服务、桥接和界面边界。Tauri 生成工程位于 `apps/desktop/src-tauri/gen/apple`，不在其中维护第二份键盘实现。

Xcode 工程由 XcodeGen 从 `project.yml` 生成，`project.yml` 是唯一权威来源；构建前先跑一次 `xcodegen generate`。生成的工程包含设置 App、`UIInputViewController` 键盘扩展和共享 Swift 适配层。输入算法与组合状态由 Rust Engine（`crates/engine`）管理；扩展只负责宿主事件、候选展示和文本提交。键盘扩展不直接使用桌面音频采集桥接。构建前需要先按下面的步骤把词库暂存到 `target/ios/EngineResources`。

`apps/desktop/src-tauri/gen/apple` 下的 Tauri 生成工程是共享设置界面的构建产物来源，使用 iOS 17 最低版本和同一 `group.app.msime.ios` App Group。它服务于原生宿主承载共享界面这一用途，**不作为 iOS 的产品 App 分发或启动**。原生入口只解析系统提供的共享容器 URL 并注入状态根；偏好校验、并发 revision 和首次 HostOptions 默认文档仍由 Rust 共享层负责。生成工程链接系统 SQLite，并从既有 `target/ios/EngineResources` 嵌入固定词库。

共享 Tauri 语音面板通过 `tauri-mobile-platform` 在 App 进程内使用 `AVAudioRecorder` 录制 16 kHz、单声道、PCM16 WAV，最长 60 秒；停止后把有界录音上传到当前 `PreferencesStore` 中选择的服务。OpenAI、SiliconFlow 和 Groq 使用 HTTPS multipart 批量转写；Doubao 使用 WSS、共享 `client-core` 鉴权策略和帧编解码 ABI，并按 Windows 的 200 ms PCM16 分帧发送。两种传输都禁止重定向并限制接口、模型、token、音频、消息、累计响应和识别文本大小；取消会停止录音或网络请求并删除临时文件。provider 凭据只在 Rust 与原生插件之间传递，不进入 WebView、日志或键盘扩展。

**iOS 的产品宿主是 `platforms/ios` 的原生 App（`MSIMEApp`）**，它承载应用生命周期、图标与设置界面，并嵌入 `MSIMEKeyboardExtension`。Tauri/React 在 iOS 上是**公共组件**——提供跨平台共享的功能与界面，由原生宿主按需承载——**不是产品本体，也不作为独立 App 启动**。扩展继续从 `platforms/ios` 编译唯一一份原生键盘、共享 UI、宿主桥接与平台服务源码，不依赖常驻桌面服务；App 与扩展各自打包已校验词库，并通过 App Group 共享状态。真机 target 使用锁定的 ML Kit Digital Ink 8.0.0，arm64 模拟器使用明确的无识别 fallback。

共享账户页的设置同步以 MSIME-Apple 远端 `develop@81e79abec7b53e7243fb8cbe82a42a4dde1e528f` 为固定来源，上传和应用输入方案、双拼方案、简繁、九键、按键音、触感及强度、词库学习、键盘皮肤、当前自定义皮肤，以及共享输入偏好中的调频方式、触发次数和线性步长（`input.frequency_mode`、`input.frequency_trigger_count`、`input.frequency_linear_step`）。Tauri 继续由 `BackendAccountSession` 持有 Keychain 会话；WebView 只接收有界标量设置，不接收 token。iOS 平台适配器在 App Group UserDefaults 与共享 `PreferencesStore` 间同步键盘可直接修改的状态，应用前完整校验，Rust 偏好保存失败时恢复原生快照；未知平台字段原样保留在云端。iOS 英文建议开关属于键盘扩展直接读取的 App Group 原生偏好，由移动键盘反馈接口维护，不作为账号云同步字段，避免下载旧云值覆盖设备上的原生选择。凭据、联网授权、输入内容、词库和打字统计不进入设置同步。

iOS 云剪贴板复用共享账号会话和 Tauri `CloudClipboardPanel`，只上传用户在面板中明确输入的文本，不读取系统剪贴板。列表、搜索、启停、添加和删除均由 `client-core` 校验后访问账号服务；复制动作通过 iOS 平台插件写入 `UIPasteboard`，限制为 4000 个 UTF-16 单元且拒绝 NUL。只有本地剪贴板历史已开启时，复制后的文本才会进入已有 App Group 历史。

键盘的剪贴板面板在本机历史旁边多了「云端」一栏，手机上在别的设备发到云剪贴板的内容可以直接上屏。打开面板时取一次列表，之后只在点刷新时再取；不轮询、不推送，面板关掉后才回来的结果直接丢弃。点一条经 `textDocumentProxy` 插入。本机历史每条的菜单里有「发到云剪贴板」，要等面板确认已登录才可点，发送前先读一次服务端的开关，关着时什么都不发（「云剪贴板未开启」）；不会因为复制而自动上传，也不读系统剪贴板。没有完全访问时显示开启提示、不发任何请求；未登录显示「登录水杉账号后可在设备间同步剪贴板」。密码、新密码和验证码输入框（与按键统计同一条判断）里没有「云端」这一栏，面板开着时输入框变成这类字段，云端内容也随即隐藏、不能插入。键盘能读到登录态，是因为 App 把账号会话存在 App Group 的钥匙串访问组 `group.app.msime.ios` 里，这个组 App 与扩展原本就有权限，不新增 entitlement，token 也不落到 App Group 文件里；旧版本存在 App 默认访问组的会话，在 App 下一次读取时移过去，此前键盘按未登录处理。服务端每次刷新都轮换 refresh token、并在旧 token 被再次使用时吊销整个会话，所以 App 与键盘对这份会话的每一次写入（登录、刷新、更新资料、退出）都经 App Group 里 `backend-account-refresh.lock` 的 `flock` 串行，持锁后先重读钥匙串：另一个进程已轮换过就直接采用，不再拿用过的 token 去刷新；刷新回来写入前再读一次，钥匙串已被清空或换成别的账号时丢弃这次结果，不会把已退出或已切换的会话写回去。共享的钥匙串是登录态的唯一依据，每次都重新读取，被另一个进程清空就按未登录处理。拿不到共享目录时不刷新。Tauri 公共组件的 iOS 工程把会话存在 App 默认访问组，它带的键盘因此显示未登录，这是已知限制。

本地剪贴板历史的搜索对应 Windows 剪贴板页上方的“搜索剪贴板”，同样是不分大小写的子串匹配，保持原有顺序。两处都能搜。键盘扩展没法往自己的搜索框里打字，所以键盘剪贴板页右上角的放大镜会换出一块自带的键盘（数字行加 26 个字母），剪贴板里常见的链接、验证码、电话和英文都能搜。搜索时「保存剪贴板」按钮和状态行收起、标题显示搜索词，好让手机高度的面板在字母键上方还能放下记录；「返回」先退出搜索，再关闭面板。要按中文搜，用 App 的“输入设置 → 剪贴板历史”：它和键盘通过同一个共享 ABI 读写 App Group 里的同一份历史，在这里固定、删除、清空或保存的记录，键盘下次打开剪贴板页时就能看到，反过来也一样。App 用 `PasteButton` 保存系统剪贴板，不会弹出粘贴授权提示；点一条即复制。iPhone 上右滑固定、左滑删除，每条显示三行；iPad 的宽度下每条显示六行和完整日期，固定和删除也可以用指针右键菜单。

“关于水杉 → 诊断日志”对应共享偏好中的 `diagnostic_log.server`，与 macOS、Linux 的宿主日志同一个字段，随设置同步。开启后，键盘扩展在 App Group 的偏好目录写入仅所有者可读的 `diagnostic.log`，记录键盘加载（是否有完全访问、手机还是 iPad）、出现与收起、共享设置是否应用、运行时初始化或词库恢复失败，以及系统内存警告——iOS 会结束占用内存过多的键盘扩展，“键盘突然消失”的反馈最需要这一行。每条记录只有事件名，截到 192 字节的可打印 ASCII，不含按键、输入文字、候选、凭据、路径或服务响应；文件超过 1 MiB 时保留一个 `.1` 副本。键盘没有完全访问时无法写入 App Group，此时静默不写，不影响输入。App 的这一页可以开关、分享和清空日志；iPad 在同一页直接显示最近的记录，iPhone 另开一页查看。Windows 专用的 `diagnostic_log.tsf` 不在 iOS 显示，原值保留以便跨平台同步。反馈页同样链到这一页，并和 Windows、macOS 的反馈页一样列出 QQ 交流群（点一下复制群号）和 Telegram 群组。

“输入设置”页的「默认中英文」读写共享文档的 `default_ime_mode`，与桌面端同一个字段。新打开的键盘从这个模式开始；按中/英键切换后，这次打开的键盘一直保持用户的选择，设置页的修改也不会在输入途中翻转模式。以前桥接层在建立会话时把它强制覆盖成 `chinese`，现在不再覆盖。桌面端用 `ime_mode_scope` 按应用记住中英文，但 iOS 不告诉键盘扩展正在哪个应用里输入，没有可以作为键的应用标识，所以 iOS 不使用这个字段（共享层的 `HostCapabilities::ime_mode_scope` 也把 iOS 排除在外）。网址、邮箱等字段带来的临时英文仍按原来的规则处理，离开字段后恢复。同一节的「沿用上次的中英文」是 iOS 上唯一能做的记忆：它对应桌面端 `ime_mode_scope` 的全局记忆，按应用的那一种在 iOS 上没有可用的键。打开后，新键盘从用户上次按中/英键（或 Shift）选的模式开始，还没切换过时仍从「默认中英文」开始；网址、邮箱字段临时切到的英文不记录。键盘进程在两次出现之间会被销毁，所以记录放在 App Group（`ImeModeMemoryPreference`），不进共享文档，也不随设置同步到其它设备；开关变化时清掉旧记录。

键盘扩展、原生统计页与共享 Tauri 统计页都从 App Group 的 `MSIME/typing-statistics.json` 读取聚合计数，所有读写都经 `msime_client_typing_statistics` 交给 `client-core` 的共享统计存储，与 macOS 同一条路径，并以同一锁文件串行更新。键盘为每次上屏带上本地日期和小时，共享存储据此记下每日活跃时长（两次上屏间隔不超过 10 秒计入）和每小时字数；原生统计页新增「节奏」标签，显示今日与平均速度、今日活跃、连续天数、日均、最多与最快的一天和今日时段分布，算法与共享统计页的 `activityMetrics` 一致，iPad 宽窗口一行放四格，手机两列。「趋势」标签下的「按日明细」对应 Windows 统计页的同名表格，列最近 30 个有记录的日子（日期、字数、中文、英文、数字、标点、其他、活跃、速度，「其他」收下其他文字、表情、符号和旧记录里未分类的部分，各列相加等于字数）；iPad 宽窗口照原样画九列，手机每天一行字数加一行分类小字。右上角可把全部保留的日子导出为带 BOM 的 UTF-8 CSV（活跃时长以分钟计），交给分享面板。原先 Swift 自带的读写逻辑会在每次写入时丢掉共享存储才认识的字段（活跃时长、时段、候选位置），保留期也存成了另一个键 `retentionDays`；每个进程第一次访问时会把它改写成共享的 `retention` 字段，再交给共享存储。与共享存储一致，新装时统计默认关闭，需在统计页右上角菜单里打开；已有统计文件里写着的开关状态保持不变。升级时若只存在旧 Swift App 写在 App Group 根目录的统计文件，会在双端加锁并验证后原子移动到共享状态目录；迁移和日常记录都只包含分类计数，不保存实际输入文本。统计打开且有完全访问权限时，键盘还为每个软键记按下次数：26 键字母记作 `KeyA`…`KeyZ`，九键格位按格上的数字记作 `Nine0`…`Nine9`（日文假名格同样按手机键位映射），符号层的键记到产生该字符的 ANSI 键（如「！」记 `Digit1`），123、符、中/英、地球、表情、语音记作 `Soft*`，没有对应键位的键不计；按住删除连删只算一次，密码框不计，统计关闭时连内存里也不攒。计数在内存里按本地日期攒批，满 256 次、跨日（先写旧日期）、每 30 秒、键盘收起或宿主转入后台时经 `record_keys` 在统计队列里写入共享存储的 `dailyKeys`，只保存每个键每天被按下的次数，不保存顺序、时间和内容；清空统计会一并删除，保留期也同样裁剪。统计页的「按键」标签按选中的日期或累计画出 26 键键盘热力图，有九键记录时加画九宫格，另列最常按的五个键和键盘图上没有位置的「其他键」。键盘扩展收不到硬件键盘的按键事件，所以 iOS 只统计软键。

键盘扩展通过共享宿主策略执行智能标点的三条规则。**重复标点转中文**：刚以 ASCII 上屏逗号、句点或冒号后两秒内再按同一个键，把它换成中文标点并结束这次手势；只在文档里确实还是第一次按下留下的那个字符时才触发，组字中、有候选、换了编辑器都不算。**空格转英文**：刚上屏一个中文标点后按空格，把它改回 ASCII 并吞掉空格——人是在改刚打出来的那个标点，不是打了标点再打空格；这一条默认关闭，因为它改写的是用户已经看着落下去的字符。两者的快照由键盘持有而不是放进会话：它们属于宿主的编辑器，而键盘收起时会话会被销毁重建，一个跨过那道缝还活着的手势是错的。editor generation 取 `documentIdentifier` 的前八个字节，不用 `hashValue`（Swift 的哈希按进程加种子）。第三条是**直出**：中文跟随模式且 Engine 空闲时，逗号、句点或冒号紧跟 ASCII 字母或数字**且共享偏好中对应的 `smart_punctuation_direct_letter` / `smart_punctuation_direct_digit` 已开启**时保留 ASCII——这两个开关默认关闭，未开启时仍走 Engine 的中文标点；锁定中文或英文优先；已有组合、日语、英文和本地模式仍交给 Engine。扩展只从 `UITextDocumentProxy.documentContextBeforeInput` 提取紧邻光标的一个 Unicode 标量，不保存或记录宿主文字；中文/日文键帽显示值会先映射回 Engine 的 ASCII 标点输入，缺失上下文安全回退到 Engine 标点。

App 的“输入设置 → 标点”页直接读写共享 `PreferencesStore`：智能标点及其四条子规则（重复转中文、空格转英文、数字后直出、字母后直出，总开关关闭时置灰）、成对标点自动补全和固定标点。这些开关没有 App Group 兼容键，页面通过 `MetasequoiaInputSessionBridge.loadSharedPreferences` / `updateSharedPreferences` 访问，不创建输入会话、不加载 Engine；写入与键盘共用同一份带 revision 的比较交换，写入失败时页面回读实际值并提示重试。键盘出现时 `reloadSharedPreferences` 只把这组标点开关（以及下面「候选与纠错」「辅助码」「本地输入模式」三页的 `quanpin`、`mixed_input`、`quanpin_helpcode`、`shuangpin_helpcode`、`local_modes` 五个对象，以及由下面「云候选」开关决定的 `cloud_candidates`）交给正在运行的会话（会话在组字中会排队到输入结束），输入方案等其他字段仍等下一个会话，避免键盘可见时方案被换掉。

成对标点自动补全打开时，Engine 上屏左半边（“ ‘ （ 《 〈 【）后键盘写入右半边并把光标放回中间；引号键每次都开一对新的，与 Windows 一致。右半边已经在光标右边，所以之后按它的键（`"` `'` `)` `]` `>`）只把光标移过去、不再写第二个，对应 Windows 的 `_TryStepOverPairedPunctuation`：否则（内容 再按 ） 会变成（内容）），而右引号根本打不出来。键盘记着自己补过的对（最内层在上，最多 16 层），只在没有组字时、在开这一对的同一个编辑器里、且光标右边确实还是那个右半边时才跨过去；宿主不报告光标后文字时以记录为准，这也和 Windows 相同。删除文档文字、拖动空格移动光标、换行都会清空记录；组字中按右括号照常连同候选一起上屏。

“输入设置 → 候选与纠错”页不显示全拼纠错开关；字母错位和相邻键误触均由 Engine 默认开启，已有明确保存的 `false` 仍生效。候选混输（`mixed_input` 的英文单词及触发字母数、emoji、颜文字）仍在此页设置。这些字段是嵌套对象，每次写入只合并一个子字段，其余子字段保持存储值。同一页的「以词定字」对应 `word_character.enabled`：桌面用 [ ] 或 - = 键只上屏候选的首字或末字，键盘扩展收不到硬件键（iPad 外接键盘也一样），所以 iOS 把它放进候选的长按菜单（候选栏和展开的候选面板都有），只对两个汉字以上的候选出现，`word_character.keys` 在 iOS 上不用。

同一页的「预编辑」对应共享设置里的 `candidate_preedit_style` 和 `shuangpin_preedit_uses_raw`。键盘扩展没有文档里的组字，候选栏左侧就是正在拼写的内容唯一看得见的地方，选「不显示」把这块位置让给候选；和 Android 一样，只隐藏正在拼的那一段：已选定的半个词（`phrase_prefix`）仍然显示，否则用户选过的字既不在文档里也不在屏幕上；本地输入模式的名称也保留，因为它说明当前在哪个模式。这个设置只改画出来的标题，组字状态不变，拼写还没有候选时候选栏也不会退回快捷栏，VoiceOver 仍读出完整内容；日语的假名读音不是拼音，不受它影响。「行内预编辑」对应 Windows 的 `tsf_preedit_style`：「原始按键」把按下的键原样写进输入框（双拼 `ui` 仍是 `ui`），「拼音分词」写候选栏同样的分词拼音，「不显示」只在候选栏显示。它是 iOS 自己的设置、默认关闭，因为共享设置里的默认值是 `raw`，照搬会让所有 iPhone 和 iPad 用户一升级就看到行内文字，而第三方 App 画键盘扩展标记文本的效果参差不齐；以前打开过的开关读作「拼音分词」，也就是它原来写进去的内容。日语总是写假名读音。「双拼显示原始按键」由 Engine 消费，关掉后双拼组字显示按键展开的拼音，例如 `ui` 显示为 `shi`；它和标点字段一样列在桥接层交给运行中会话的字段里，键盘进程不必重启，下次出现、空闲时就生效。

候选栏的配色是全局主题的一部分（`global_theme`，自定义主题是 `custom_theme`，App Group 镜像 `globalTheme`），在「设置 → 主题」里选，与电脑版同步；那一页的「外部皮肤与候选颜色」对应 `custom_theme.candidate_skin`、`custom_theme.candidate_colors` 和共享的 `candidate_theme`，以前在「候选栏」页，现在按全平台信息架构并入主题页的自定义主题。iOS 的候选栏是键盘的一部分，默认和按键一起用键盘主题的颜色（首选候选用强调色、不填底）；只有打开 iOS 独有的“使用桌面候选皮肤”开关（App Group `candidate_palette_follows_desktop`，不参与同步）后，候选栏才换成 client-core 为当前主题解析出的候选配色，也就是桌面候选窗画的那一套：首选候选用主题的高亮色，背景、文字、编号和描边取自主题；`system` 没有候选配色，开关打开也仍画原生候选栏。“明暗”选跟随系统时先看共享的 `theme`，再看系统外观。选外部皮肤或改颜色会切换到「自定义」主题，并以当前主题（或外部皮肤声明的底）为底。外部皮肤在桌面放进皮肤文件夹即可，iOS 的皮肤目录在 App Group 里，“文件”看不到，所以同一处有“导入皮肤”：从“文件”选一个含 `skin.toml` 的皮肤文件夹，经与 Tauri 壳相同的 Rust 导入（`msime_client_skin_import`，先校验文件夹名和清单，同名皮肤整个替换，最多 4096 个文件、256 MB）复制进去；皮肤列表只列支持横排的外部皮肤，只声明一种明暗的会标出来，选中的外部皮肤可以删除。App 用系统颜色选择器编辑文字、背景和首选高亮三种颜色，也能一键恢复皮肤颜色；iPad 并排预览浅色和深色，iPhone 只预览当前外观。主题、候选栏和输入三页顶部都有同一张候选预览卡，按当前主题、开关、候选字号和英文释义设置画出候选栏的样子。键盘下次出现时应用这些改动，不重建 Engine 会话。

同一页顶部「字号与字体」一节的两个字号直接写共享文档的 `candidate_font_size` 和 `candidate_preedit_font_size`（默认 18 和 15，与桌面候选窗同一对字段）。iOS 候选用 Dynamic Type 绘制，所以默认值就是原来的 `.body` 和 `.subheadline`，其他值按同一比例放大或缩小，系统文字大小仍在其上生效。手机候选栏宽度有限，候选字号限制在 12–24、编码字号 12–20；停靠的 iPad 键盘是 12–32 和 12–24，浮动的小键盘按手机处理。桌面同步来的更大字号会被夹到当前表面的上限，不改写文档。候选行和编码行随字号变高、键盘整体随之加高，字号变小时行高不低于默认值，触摸目标不缩小。这两个字段只影响键盘自己绘制，不交给会话；键盘每次出现重新读取共享文档后即生效，候选面板同样按这个字号绘制。

同一节的「中文字体」和「英文字体」写 `candidate_font_family` 与 `candidate_english_font`，与桌面候选窗同一组字段；补充字体 `candidate_fallback_fonts` 没有编辑界面（各端设置页都不提供），由桌面的字体预设写入并随文档同步。键盘按桌面的顺序拼出字体链：英文字体在前，负责拉丁字母；其后是中文字体与后备字体，按 `UIFontDescriptor` 的 cascade list 补齐前者缺的字形；链里都没有的字形交给系统字体。链中只保留本机装有的字体：桌面同步来的 Microsoft YaHei、共享默认值 Noto Sans SC 在 iPhone 上都不存在，因此没人改过的文档照旧用系统字体（苹方）。字体按字号与 Dynamic Type 缩放，只用于候选文字，编码行与注释仍用系统字体。选择列表列出本机字体、每一项用自身字体绘制；文档里本机没有的名字保留在列表顶端并标明「未安装」，而不是悄悄换成别的。`candidate_font_controls` 与 `candidate_english_font` 两个宿主能力位因此对 iOS 打开。

同一页的「云候选」是 iOS 自己的开关，存在 App Group（`candidate.cloudCandidates`），默认关闭：共享文档的 `cloud_candidates` 默认开，Windows 和 Android 照此联网，但 iOS 键盘对外承诺默认离线，所以桥接层在建立会话和每次重新读取共享文档时都用这个开关覆盖会话里的 `cloud_candidates`，覆盖值从不写回共享文档，桌面同步来的开启值也不会让 iPhone 自行联网。开启并允许完全访问后，键盘在组字停顿 0.35 秒时向会话取 `online_query`，按会话给出的 HTTPS 地址查询 Google 输入法服务（连接和总时长各 2 秒、回复上限 256 KiB、拒绝重定向、不带 Cookie 和缓存），把原始回复交回 `apply_cloud_response`，由共享层解析、判断组字是否仍是当时那一个，再渲染它返回的视图。共享文档启用了 AI 助手候选（`ai_assistant`）时沿用同一流程，它不受「云候选」开关控制、只看 AI 助手自己的开关，同样需要完全访问：云候选之后按会话生成的请求描述 POST，回复交给共享解析器，再以 source 1 应用；凭据只在描述里经过，不记录。

「每页候选数」对应桌面端的 `candidate_page_size`，同样是 iOS 自己的设置，存在 App Group（`candidate.pageSize`），范围 1–9，默认 9：共享文档的默认值 6 是按桌面候选窗口定的，而且分不清用户选的 6 和默认的 6，直接沿用会把现有 iPhone 的候选栏悄悄改短。桥接层像云候选一样把它覆盖进会话，并随每次重新读取共享文档交给正在运行的会话（组字进行中则等空闲再换）；候选栏的编号读的是会话实际拿到的值，所以符号键盘和 iPad 数字行的数字键与编号始终一致，其余候选在展开面板里。组字时按下超出本页编号的数字什么也不做，不会把数字写进输入框。宿主能力 `fixed_candidate_page_size` 仍报告 iOS 不使用共享文档里的页大小，共享设置页因此不显示那一项。

「AI 服务」页在「在键盘中启用 AI」打开后多出「候选栏 AI 候选」开关和候选数量（1–10，默认 3），对应桌面端 `[ai_assistant]` 的 `enabled` 和 `candidate_limit`。开启时页面把已经发布给键盘的那份配置（服务商、接口地址、模型）写进共享文档的 `ai_assistant`，密钥不写：桌面端把密钥放在 `ai_assistant.tokens` 里，iOS 的密钥只在与键盘共享的钥匙串里，写进可同步的共享文档就等于把它发到别的设备。键盘出现和重新读取共享文档时（`AICandidatePreference.credentialEndpoint`），只有共享文档的接口地址与键盘已保存配置的地址相同，才从钥匙串取出密钥，经 `msime_client_set_ai_credential` 只放进本次会话的内存，由它为当前服务商签名请求；地址不同、开关关闭或配置被删除时收回。共享文档里没有自定义提示词时，请求使用与 Windows 默认 `[ai_assistant].prompt` 相同的联想提示词（`DEFAULT_CANDIDATE_PROMPT`）。关闭「在键盘中启用 AI」或删除密钥会同时关闭候选栏 AI 候选。

同一处的「提示词」页对应桌面 AI 设置里的「自定义一 / 二 / 三」：分段选择器决定用哪一个（`ai_assistant.prompt_id`），编辑器写入对应的 `prompt_custom_1`…`prompt_custom_3`，点「保存」、切换槽位或离开页面时写入共享文档。留空（包括只有空白）时共享层发送内置提示词；「自定义一」为空时和桌面端一样先回退到旧的单一 `prompt` 字段。编辑器放在单独一页，是为了手机上有整屏的编辑空间。

「AI 服务」和「语音服务」页的「测试连接」对应桌面端的凭据测试（`api_credential_test.cpp`），检查的是页面上正在填写、尚未保存的地址、模型和密钥（密钥留空时用已保存的那个）：AI 发一条只要求回复 OK 的请求，文件转写服务上传一秒静音的 16 kHz WAV，只看 HTTP 状态是否为 2xx，不管返回内容；豆包实时语音走同一条 WebSocket 握手，静音得到空转写也算通过，因为能拿到转写结果就说明鉴权已被接受。豆包的「流式接口」与 Windows 设置页相同：整句流式（`bigmodel_nostream`，说完返回整句，官方推荐用于输入法）或双向流式（`bigmodel_async`，默认，返回增量结果）；两种接口的 `result` 一个是对象、一个是分段列表，iOS 和 Windows 一样两种都读。「边说边识别」对应 Windows 的 `stream_inline_preedit`（共享偏好 `voice_input.stream_inline_preedit`，默认开）：打开时 App 用 `AVAudioEngine` 采集 16 kHz PCM16，每攒够 200 ms 就发出一帧，豆包返回的中间结果实时显示在录音按钮下方，停止录音后发出末帧并等最终结果。Windows 把中间结果直接写进光标处的组字区；iOS 键盘扩展没有麦克风，录音在 App 里进行，所以中间结果显示在页面上，最终文本照旧经语音交接送回键盘。实时识别失败时整段录音仍然保留，可以改为按原方式发送。文件转写服务没有中间结果，不显示这个开关。

「语音服务」页的「识别语言」「录音提示音」和「识别后润色」读写共享偏好的 `voice_input`，与桌面同一组字段：`language`、`sound_enabled`、`start_sound`/`end_sound`（总开关打开时，下面分出「开始录音时」「结束录音时」两个开关，对应桌面的开始、结束录音提示音；总开关关着时两声都不响，两个分开关也就收起）、`polish_text`/`polish_enabled`、`polish_prompt_id` 和 `polish_prompt_custom_1..3`。预设提示词和自定义槽的回落规则不在 Swift 里另抄一份，App 经 `VoicePolishPrompt.cpp` 直接调用 macOS 与 Windows 共用的 `shared/voice/PolishPrompt.h`；识别文本同样包进 `<asr_text>` 发给模型。桌面单独配置润色服务和密钥（文本润色 API），iOS 默认复用「AI 服务」页已保存的服务与钥匙串密钥，免得同一把密钥填两遍；「润色服务」页可以改成单独设置，服务商预设与 AI 服务页同一份（含 Groq），接口、模型存在本机，密钥另存一份钥匙串条目，即使与 AI 服务同一主机也互不覆盖，都不写进可同步的共享偏好。关掉「单独设置」立即回到 AI 服务，已填的单独服务保留下次再用。内置的几种润色方式和桌面一样可以直接改提示词：改动写进 `voice_input.polish_prompt`，共享解析器用它代替所选方式的原文；换一种内置方式或点「恢复默认」就清掉它，从电脑同步来的修改也因此看得见、改得回。没有可用服务或润色失败时保留识别原文，结果区也可以随时改用原文。识别语言按共享规则只发语言部分（`zh-CN` 发 `zh`），「自动识别」、SiliconFlow 和豆包不发。录音时按钮下方有一条实时音量波形，对应 Windows 语音浮窗的输入电平：普通录音读 `AVAudioRecorder` 的电平，边说边识别读每段 PCM 的 RMS，每秒采样 20 次。录音会话会压住系统提示音，所以开始提示音放完才开始录音，结束提示音在停止录音之后播放，两者都伴随一次触感反馈。「录音时暂停其他声音」读写共享偏好的 `voice_input.mute_system_audio`：iOS 不能把其他 App 静音，能做的是让录音会话独占音频（`.record`），正在播放的音乐和视频因此停下，录完再由系统通知它们恢复；关着时改用 `.playAndRecord` 加 `.mixWithOthers`，其他声音照常播放，输出仍走扬声器而不是听筒。共享默认值在 iOS 上是关，与 HarmonyOS 相同：抢走别人的音频不该是默认行为。

“词库”页的「输入习惯」一节直接读写共享文档的 `learning`、`frequency`（调频方式、触发频次、线性步长）、`candidate_english_gloss`、`candidate_translations`、`translation_target_language` 和 `translation_secondary_language`，与 Windows 同一组字段和取值范围：调频方式多了「不调频」，触发频次和步长为 1 到 10，释义语言多了俄语。键盘每次出现都会用共享文档覆盖这些字段在 App Group 里的兼容副本，所以页面先写文档，写成功后再把结果镜像到 App Group，供键盘在下一次读取文档之前使用；只写 App Group 的旧做法会在键盘下次出现时被文档的值改回去。设置同步里的「词库学习」也改为写共享文档，并且在其他本机设置之前写入，写入失败时整次应用不改动任何本机设置。「候选与纠错」页的「英文单词提示」是 App Group 里的 `english.suggestions`，键盘在英文输入时直接读取，不在共享文档中。

“词库 → 翻译服务”页选择候选释义联网补充时用哪个服务，对应 Windows 的腾讯云机器翻译、小牛翻译和 DeepLX 兼容自定义接口，写入共享文档的 `tencent_tmt`、`niutrans`、`custom_translation` 和 `translation_account`，与其他宿主同一组字段。默认是「不使用在线翻译」，键盘不联网翻译。选择顺序与共享层一致：小牛翻译开着就用它，否则看自定义接口，否则腾讯云的两个密钥都可用时用腾讯云；这些都不适用时，只有用户在这一页显式选了「水杉账号」（`translation_account` 为 true）才把当前页的中文候选词发到 `api.msime.app`，带的是应用首次启动时注册的本机匿名账号（`MetasequoiaImeApp.init`，存于 App Group，键盘共用；键盘先于应用运行时在首次翻译时注册），账号不会作为兜底。旧版本在没有配置任何服务时默认走水杉账号，升级后这个字段缺省即为未选择：主释义语言或第二释义语言不是英语时，那一行释义会收起，英文释义只剩离线词库；已登录水杉账号的用户同样如此，要恢复需在这一页重新选「水杉账号」。腾讯云在共享默认值里是开着的，所以页面选别的服务（包括水杉账号和不使用在线翻译）时会显式写入关闭。用户选了某个服务但凭据不完整（空值、`<占位符>`、`FAKESECRET_` 示例值）时键盘不联网翻译，不会悄悄把文字发给没选的服务。请求由共享层构造和签名（`msime_client_{tencent,niutrans,custom}_translation_http_request`），回复也交回共享层解析；键盘只负责收发字节，腾讯云签过名的正文按原样发送、不重新序列化。腾讯云每批最多九个词，小牛翻译和自定义接口逐词请求，单词限 40 字，源语言固定中文。iOS 只允许 HTTPS，自定义接口填 HTTP 地址时不会被调用。键盘出现时重新读取这组设置，切换服务或凭据会清空已缓存的释义，包括还在路上的回复。页面的「测试翻译」用当前表单（未保存也可）翻译「你好」。iPhone 与 iPad 使用同一张表单，iPad 显示在设置分栏的详情区。

同一页的「自定义候选释义」对应 Windows 放进用户目录的 `custom_translations.txt`：每行「源词 Tab 译文」，`#` 开头为注释，同一源词以最后一次为准，优先于内置词库、学到的释义和在线翻译。iOS 用户够不着 App Group 容器，所以 App 直接编辑键盘 Engine 用户目录（`MSIME/user`）里的这个文件，也可以从“文件”导入一份 Windows 用的原文件。解析规则与共享设置页的 `custom-translations.ts` 一致，用来显示条数和无法识别的行；写入规则与桌面外壳的 `write_custom_translations_at` 一致：上限 1 MB、拒绝 NUL、原子替换，清空即删除文件。Engine 在建会话时读取它，所以键盘下次载入时生效。

“输入设置 → 辅助码”页分别设置双拼和全拼的辅助码：开关、方案（蓝天小雨点、自然码、首右2.0、首右plus、小鹤、加加）以及是否在候选栏显示辅助码，默认值与 Windows 相同（双拼蓝天小雨点并显示，全拼自然码不显示）。全拼或双拼组字时点一下 Shift，下一个字母作为辅助码交给 Engine，用于缩小候选；五笔、九宫格、日语和本地模式不使用辅助码。辅助码表不在词库发布里，由仓库自带在 `resources/helpcodes/`：`stage-resources.sh` 把六个方案的表按 Engine 读取的文件名（`crates/engine/src/assets.rs`）复制到 `EngineResources/helpcodes/`，资源校验只放行这一个目录，钉住的词库文件仍逐个校验。缺了这些表，Shift 字母会被当作辅助码吃掉却不缩小任何候选。候选栏显示的辅助码来自 Engine 给每个候选的注释，桌面版把它加括号接在词后，iOS 放在候选下方，与五笔编码提示一致，不带括号。

全拼和双拼组字时可以按音节编辑拼写，对应 Windows 组字里的 Ctrl+Backspace 和 Ctrl+←/→（共享 C ABI 的 `SegmentBackspace` / `SegmentMoveLeft` / `SegmentMoveRight` 三个命令，macOS 用的是同一组）。手机上没有 Ctrl，所以按住删除键时每一下退掉光标前的一个音节，拼到一半发现哪个音节错了，可以退回那里重拼，而不是整串丢掉；拼写删空后连按随即停止，不会继续删进文档。拖动空格移动组字光标时，慢拖仍按字母移动，快速一甩（横向速度不低于每秒 900 点）按音节跳。九键的数字串在选出拼音前还没有确定的音节，五笔编码不由音节组成，所以九键、五笔和其他方案保持原来的行为：按住删除键清空整个组字，拖动空格只按字符移动。Windows 组字里还有 Home、End 和 Delete，手机键盘没有这几个键，所以组字时轻点候选栏左侧的拼写，会弹出系统菜单“编辑拼写”：光标移到开头、光标移到末尾、删除光标后的字母（共享 C ABI 的 `MoveHome` / `MoveEnd` / `DeleteForward`）。当前位置做不了的项会置灰而不是消失，菜单形状不变；空闲时同一个位置仍是本地输入模式菜单，日语读音和本地输入模式不提供这个菜单。VoiceOver 把组字中的拼写读作按钮并提示“轻点编辑拼写”。

“输入设置 → 本地输入模式”页对应 Windows 的“实用功能”：快捷短语、日期与时间、Unicode 码点、表情、颜文字、超级简拼、英文补全和临时日语逐项开关，写入共享文档的 `local_modes`，默认全开。Windows 靠 Shift 加大写字母进入这些模式，触屏键盘没有这样的键，iOS 改为在空闲时点候选栏上的名称或工具面板的“本地输入”，从列表里选；关掉的模式 Engine 不再打开，所以键盘也把它从两处列表里去掉，全部关掉时入口置灰。

表情浏览器以远端默认分支固定来源 `MSIME-Apple@7de60fb5c5590f33e7f515db7e595a1d7e848ad1` 为交互基线，在空闲候选工具栏和“更多”面板提供入口，按 Unicode 固定顺序显示最近、笑脸、人物、动物、食物、旅行、活动、物品、符号和旗帜，每行八项，支持删除和返回。为适配共享客户端架构，iOS 不复制 Apple 扩展内的 SQLite 读取器，而是在串行后台队列通过共享 C ABI 分页读取已验证 `others.db`；每页、游标、文本和注释均有边界校验，过期分类结果不会覆盖当前页。选择后通过正常 `UITextDocumentProxy` 路径插入并记录最多 24 项的去重最近使用；打开面板前先由 Engine 完成已有组合，不保存或记录编辑器上下文。

符号面板（符号键）左侧先是手机上常用的常用、中文、英文、数字、网络五组，随后接上 Engine `others.db` 符号目录的全部大类（标点、数学、货币、箭头、形状、爱心、字母、游戏、文化、自然、人物、更多），与 Windows、macOS 和鸿蒙的符号面板是同一份目录；分类栏可以上下滚动。目录大类在打开面板时一次列出，某一类的符号在点到它时才在后台队列按游标分页读取，同一符号挂在该类多个子组下只显示一次，切换分类后迟到的结果会被丢弃。目录以后新增的大类按原名显示；资源未就绪或目录读取失败时只剩那五组，面板照常可用。用过的符号按最近使用的顺序排在最前面的“最近”里，最多 30 个（五列六行），对应 Windows 表情面板打开时先显示的最近使用；它和表情的“最近”分开记录，因为一个符号可能是 `@gmail.com` 这样的一串文字，放不进八列表情网格。没用过符号时不显示这一栏。

符号面板底栏的放大镜打开符号搜索，对应 Windows 符号面板的搜索框。和表情搜索一样，键盘扩展里没有文本框，所以分类栏换成面板自带的字母键盘，已输入的字母显示在放大镜按钮上；`others.db` 符号目录的每个大类都带英文、全拼和拼音首字母关键词，于是 `arrow`、`jiantou` 和 `jt` 都能找到箭头。结果跨所有大类、去重后在网格里显示，最多 255 个，由后台队列经共享 C ABI 查询，迟到的旧结果会被丢弃。点结果和浏览时点符号一样：插入、记入“最近”，面板没锁定时随即关闭。没有结果、或目录读取失败时网格上方会给出提示；返回键先退出搜索回到分类，再按一次才关闭面板。目录资源未就绪时不显示放大镜。

面板最后一个标签是“颜文字”，对应 Windows 的颜文字目录：同一个共享 C ABI 以 `kaomoji` 目录分页读取 `others.db`，1400 条共一个分组。颜文字是一行文字而不是一个图形，最长 59 个字符，塞不进表情的八列网格，所以这个标签按可用宽度排列：iPhone 两列，停靠的 iPad 键盘按每列约 170 点放到五六列；长度校验也只对颜文字放宽到 96 个标量。插入颜文字不写入“最近”，因为“最近”沿用八列表情网格。文字颜色跟随键盘皮肤，深色皮肤下仍然清晰。

表情面板标题栏的放大镜打开搜索，对应 Windows 表情面板的搜索框。键盘扩展里没有可以输入的文本框，而 Engine 按每个表情的拼音和英文关键词匹配，两者都只用字母，所以搜索时分类栏换成面板自带的字母键盘，输入的字母显示在标题位置，结果跨所有分类显示在上方网格里，点返回回到之前的分类。分类栏的位置换成“表情 / 颜文字”两个范围，对应 Windows 搜索结果里分开列出的 Emoji 和颜文字两组；从颜文字页打开搜索时先查颜文字，从其他分类打开先查表情，切换范围保留已输入的字母。颜文字的结果按颜文字页的宽列排版。Windows 面板把用过的表情、颜文字和符号记在同一份“最近”里；iOS 的“最近”是八列表情网格，放不下一行颜文字，所以用过的颜文字另记在“最近颜文字”里（最多 16 个，手机上两列八行），这一栏排在“颜文字”前面、按颜文字的宽列排版，没用过颜文字时不显示；表情结果照常写入“最近”。符号在键盘的符号面板里，不在这里搜索。

App「词库」里的个人词典对应 Windows 的用户词编辑：编辑器除编码和词条外还有权重（1 到 100,000,000，新词默认 100,000），列表每行显示当前权重；只改权重时仍以原词条为匹配依据排队，键盘在下次激活时交给 Engine 替换。导入除 JSON 外还有“纯中文”模式，对应 Windows 的 `importHans`：粘贴或选一个文本文件，每行一个汉字词，`#` 开头的行和空行跳过。拼音由共享 C ABI `msime_client_dictionary_hans_entries` 用随键盘扩展打包的 `msime.db` 只读标注，不写任何用户数据，所以 App 可以先预览再入队，不必在 App 里启动 Engine 会话；含非汉字的行整批拒绝，与 Windows 一致，重复词只保留一次，单次最多 128 条（与 JSON 导入同一上限）。

共享设置页的「导入词库文件」在 iOS 上同样可用，支持与桌面相同的标准、Windows、Rime 和纯中文四种格式。桌面走 Engine 直接写入，需要维护锁，而键盘打字时持有这把锁，所以 iOS 的 Tauri 宿主把 `import` 交给原生桥：共享 C ABI `msime_client_dictionary_import_entries` 用打包词库只读解析文件，返回可入队的词条和导入报告，原生桥再把词条放进键盘下次激活时应用的 App Group 队列。Engine 不接受的行（例如简拼，或音节数与字数不符）按行号计入“跳过”，重复词只入队一次，超出队列 128 条的部分记为“文件过长”，页面给出与桌面相同的导入说明，不会因为一行有问题就整批拒绝。Android 的队列导入走同一套解析和报告。iOS 实际随包的原生 App 在「导入个人词库」里有同样的“词库文件”来源：选词库类型（拼音、五笔、英文、快捷短语）和文件格式（标准、Windows、Rime），从“文件”选一个 UTF-8 文本文件，由同一个 C ABI 在后台解析，预览里列出将入队的词条，并用与桌面相同的措辞说明跳过的行、被截断的部分和两列顺序相反的文件，确认后入队。

个人词典页的搜索框带“我的词条 / 拼音 / 五笔 / 快捷短语 / 英文”几个范围，对应 Windows 词库管理器按词库查找并调整内置词。“我的词条”只查用户自己的词；选某个词库再输入编码时，键盘在那一整个词库里按编码前缀查找，内置词条也会列出，并标“内置”。内置词条的编码和词随词库安装，不能修改：点开只能调权重，滑动可以删除。这些改动和其他编辑走同一个 App Group 队列，队列里的词条带着 `source: bundled` 标记，键盘同步时交给 Engine 就地修改，并记入日志，词库升级后依然保留。如果编辑改动了内置词条的编码或词，Engine 会拒绝。从文件导入或在表单里新建的词一律按用户词校验，不能自称内置。

个人词典页的「导出」对应 Windows 按词库类型导出：选类型和格式（词在前的标准格式，或编码在前的 Windows 格式）后点「生成导出文件」。App 不能在键盘可能持有词库时打开 Engine，所以导出和编辑走同一个队列：请求记在 App Group 的 `sync.json` 里，键盘下次同步、并且排队的编辑都已应用后，在一次维护窗口里按每页 1000 行调用 Engine 的 `export`，把结果写到 `PersonalDictionary/export.txt`，页面随后以桌面的文件名（如 `水杉IME-拼音用户词库.txt`）交给分享面板。导出规则来自 Engine，与桌面相同：拼音只导多字词，并带上调过权重的内置词，其他类型只导用户词。键盘扩展的内存上限远低于 App，所以一次导出最多 8 MB、20 万行，超出时在最后一个完整行处截断，页面说明只导出了前面一部分。

Apple 客户端旧版 `english.mixedCandidates` 布尔值在创建首个共享输入会话前一次性迁移到 `mixed_input.english`。迁移使用共享偏好存储的 revision CAS；成功后删除旧键，冲突或写入失败则保留旧键供下次重试。共享配置中的触发阈值、Emoji 与颜文字字段原样保留，iOS 不建立第二套偏好源，也不复制 Engine 的英文候选算法。

全角输入与其余五个宿主同一条链路：共享偏好的 `character_width`（App「标点」页的「全角输入」，与桌面设置同一字段）是键盘会话的起始宽度，键盘经 `msime_client_set_character_width` 告诉运行时，此后运行时完成的每一次提交都已转成全角；键盘只再转换自己绕过运行时直接写入的符号、空格、快捷标点与英文字母，手写、剪贴板、AI、语音和表情保持原文。键盘收起时会销毁会话、再出现时重建，重建后的会话会被重新告知宽度。“更多”面板的“全角输入”开关只临时切换当前键盘；设置页重载时，只有 `character_width` 本身变了才覆盖它，保存其它设置不会清掉刚按下的切换。这个开关以前存在 App Group 的 `keyboard.input.fullWidth` 里，共享设置因此在 iOS 上不起作用；App 启动时把旧值并入共享偏好一次，随后删除旧键。

组字中按中/英键、Shift 切到英文或按回车，和 Windows 一样把已敲下的字母原样上屏（`commitRaw`），不再替用户选首个候选；回车键在组字期间显示「确认」（日语为「確定」）。九宫格和日语例外：九宫格的原始按键是数字，日语是在确认假名，这两者仍走 `finishComposition`。规则集中在 `CompositionBoundaryPolicy`。Windows 用 Ctrl+Shift+Alt+C 清除候选缓存（`msime_client_reset_cache`，重新查询而不用 Engine 缓存的候选）；iOS 不把硬件键交给第三方键盘，这组快捷键按不出来，所以“更多”面板底部的「维护」里有一张「清除候选缓存」卡，点了收起面板，候选栏提示「已清除候选缓存」。“更多”面板的「中文标点」开关对应 Windows 的中英文标点切换，只改当前键盘（经 `msime_client_set_chinese_punctuation`），从英文切回中文时恢复共享偏好的 `chinese_punctuation`，设置页只有这个字段变了才覆盖它；标点被锁定时开关置灰。锁定为中文时英文模式下的标点也交给运行时，出中文标点。Windows 在锁定中文时会强制打开标点开关，而 Engine 只要开关关着就不出中文标点，所以共享宿主层把「锁定中文」连同开关一起交给 Engine，所有平台都按这个语义处理。

韩语方案（「韩语 26 键」，共享偏好里是 `korean`）是同一个输入法里的又一个方案，按两套式（두벌식）键位输入：字母键的键帽换成对应的韩文字母（자모），键盘只把键帽下面的 ASCII 字母交给 Engine，音节由 Engine 的自动机拼成（`DubeolsikKeyLayout`）。Shift 在这里是键位自己的上档，Q W E R T O P 上档出 ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ 并以大写字母发送，其余键上档不变；它不切英文，也不打开本地模式。正在拼的音节总是作为标记文本写进输入框，不受「行内预编辑」影响，候选栏显示这个音节，除了下文的汉字列表韩语没有候选；下一键开始新音节时，上一个音节随同一次快照的 commit 上屏。空格、回车、数字和光标移动先把音节上屏，再做按键本来的事（插入空格或换行、执行输入框的发送动作），所以回车键在组字中仍显示输入框自己的名称；标点一律是半角 ASCII，全角开关、中文标点开关、繁体输出、候选释义和本地输入都不作用于韩语。按住删除键一次退一个字母，退完音节后继续删文档里的字；拖动空格先把音节上屏再移动光标。

韩语组字时候选行末尾出现「漢」按钮，对应韩国键盘的汉字键（键盘扩展收不到硬件键，iPad 外接键盘也一样，所以只有这个按钮）。点一下发送共享命令 16（`MSIME_CONVERT_HANJA`，桥接层的 `convertHanja()`），按 libhangul 汉字表的顺序在候选条上列出正在组字的那一个音节的汉字，训音（훈음）以释义行的小字显示在汉字下方的第一行，完整显示（候选按训音的宽度加宽，不截断），不受「候选释义」开关影响，韩语方案的候选条始终为这一行留出高度；打开候选释义时，翻译和释义（离线词典、自定义与学习到的释义、在线翻译，与中文候选走同一条路径）排在训音下面。VoiceOver 把训音读作「训音」、翻译读作「释义」。训音只用于显示，不会上屏，也不出现在长按菜单里；按键字母不显示；再点一次关闭列表，列表打开时按钮带底色。只有单独的辅音或元音（如 ㄱ）时没有汉字，Engine 不处理这一下，字母继续组字，候选栏提示「单个字母没有对应的汉字」。列表打开时：点选候选、空格和回车提交汉字（空格、回车提交排在最前的那个，不再插入空格或换行，回车键显示带强调色的「确认」），数字键选择本页对应项，退格只关闭列表、音节继续组字；字母关闭列表并继续组字；标点、切换中/英和切换方案关闭列表并把韩文上屏；宿主移动了光标或换了输入框时音节原样留在文档里，键盘连发两次取消，让 Engine 也丢掉它（第一次只关闭列表）。汉字候选不学习，没有长按菜单。只能转换正在组字的那一个音节，已经上屏的音节不转换。汉字表编进了 App 与键盘扩展共用的引擎库，它的 BSD-3-Clause 声明（`resources/licenses/libhangul-hanja-BSD-3-Clause.txt`）作为 App 的资源随包分发。引擎里粤语与注音方案所用数据的声明（`resources/licenses/rime-cantonese-CC-BY-4.0.txt` 与 `resources/licenses/libchewing-data-LGPL-2.1.txt`）同样作为 App 的资源随包分发；随包的 `cantonese.db` 与 `zhuyin.db` 旁边还各带一份同名数据的许可证文本。

粤拼、大千注音和越南语是另外三个方案（共享偏好里分别是 `cantonese`、`zhuyin`、`vietnamese`），全新安装默认不启用，在 App 的「输入方案」里打开后出现在键盘的方案选择里（「粤 26」「注 大千」「越 26」），中/英键分别显示「粤」「注」「越」。粤拼和注音读随包的语言词库：两个数据库放在键盘扩展里 EngineResources 的同级目录 `language-dictionaries/`，host-api 准备会话配置时自动找到它们，键盘不另传路径；缺少某个数据库时，键盘的方案选择不显示对应方案，已经选中它的共享偏好由 Engine 退回最近一次的中文方案。App 本身不带 EngineResources，判断不了词库是否在，所以 App 里的方案列表照常显示这两项，也不会改写已经选中的方案。粤拼与全拼一样是字母拼写加候选，拼写里可以移动光标，标点、「中文标点」开关与中文一致。越南语按 Telex（数字键为 VNI 声调）输入，是另一种语言而不是中文方案：从中文切到越南语再切回来，回到原来的中文方案。

注音方案换成大千键盘：四排键对应标准键盘的数字行和三排字母（含 `- ; , . /`），键帽画注音符号或声调（3 ˇ、4 ˋ、6 ˊ、7 ˙），按下时发送键帽下面的 ASCII 键，由 Engine 的注音编辑器拼音、选字（`ZhuyinKeyLayout`），输出固定为繁体，不经过简繁转换。空格在有待定音节时是一声，没有待定音节时打开候选列表；列表打开时点选候选或按空格选字，选中的字留在转换结果里继续组字，回车随时把整段转换结果上屏；数字 1–9 在 Engine 里是选本页的行，所以键盘先关闭列表再发送，按键始终输入键帽上的符号；退格和取消先关闭列表。没有组字时单独按声调键，Engine 不处理，键盘照硬件键盘的做法输入键帽下面的 ASCII 键（3、4、6、7）。符号面板里的标点先把转换结果上屏，再由键盘直接写入：「中文标点」打开（或标点锁定为中文）时写中文标点，关闭（或锁定为英文）时写 ASCII 标点，键帽随之切换；注音方案下「中文标点」开关与快捷键可用。注音与越南语都是就地组字：正在转换的文字或正在拼的词总是作为标记文本写进输入框，不受「行内预编辑」影响，拼写里没有可移动的光标，按住退格逐键删除并继续删到文档里；回车把它原样上屏（注音不再换行，越南语随后换行并执行输入框的动作），空格、数字、光标移动和切换方案先上屏再做按键本来的事。越南语没有候选，字母的 Shift 与 Caps Lock 是大小写，不切英文，标点和全角开关都按半角处理；宿主移动光标后键盘连发两次取消，第一次 Engine 只是恢复原始按键。统计页把粤拼和注音计入中文模式，越南语单独一项。

键盘按宿主给出的 trait 区分手机与平板形态（`KeyboardFormFactor`），不按机型判断：只有 regular 宽度的 iPad 才画平板键盘，iPad 的浮动键盘、Slide Over 与台前调度里的窄窗口是 compact 宽度，和系统键盘一样退回手机布局，停靠与浮动切换时随 size class 变化重新布局。平板键盘更高且横屏比竖屏高，第三排字母末尾带逗号和句号（中文模式显示中文标点，仍以 ASCII 交给 Engine）。平板键盘默认还有桌面键盘那样的数字行和 Tab 键（「键盘设置 → iPad → 数字行与 Tab 键」，App Group `keyboard.tablet.fullKeys`，这一节只在 iPad 上出现）：数字行在字母上方，打开时键盘加高一排而不压扁字母，和符号层的数字走同一条路径，组字时 1–9 选候选、没有组字时直接输入；Tab 在 Q 左边，宽一格半。桌面组字时 Tab 翻到下一页候选（共享的 `navigation.tab`，默认开），iOS 候选栏没有分页，只有背后的全部候选面板，所以组字且有候选时 Tab 打开这个面板；`navigation.tab` 关闭或没有组字时，结束组字并输入制表符。符号层、九键、手写和假名布局不显示数字行。iPad 没有 Taptic Engine，键盘「更多」面板、App 的输入设置和皮肤编辑器在非 iPhone 上不显示按键振动与振动强度；存储值不被改写，设置同步仍把它原样带给用户的 iPhone。

App 有设置、社区、统计、我的四个标签页。「设置」标签页分形态：iPad 在 regular 宽度下是侧栏加详情的分栏（`TabletSettingsView`），侧栏是状态卡、搜索和与手机相同的分组（`SettingsPage`），切换栏目时详情重建一条新的导航栈；iPhone（包括横屏时同为 regular 宽度的 Max 机型）与 iPad 的窄窗口是分组列表加单栈推入。

“主题”页最后的「高级 · 明暗」对应共享的 `screen_keyboard_theme`，以及手写、表情、语音面板各自的 `handwriting_theme`、`emoji_theme`、`voice_theme`，与电脑版同步。键盘皮肤的颜色都是动态色，所以键盘只需按这些设置覆盖 `overrideUserInterfaceStyle`，就能不管宿主 App 是浅色还是深色，都画出皮肤的浅色或深色形态。选“跟随系统”时先看共享的 `theme`，它也是 `system` 时不覆盖，保持原来跟随宿主外观的行为。面板与 Android 有一处按 iOS 调整：面板留在“跟随”且全局主题是 `system` 时跟随键盘而不是宿主，因为面板画在键盘里面，否则会在深色键盘里出现一块浅色面板，所以 App 里把它写作“跟随键盘”。iPad 的主题页把三个面板和键盘并列显示；iPhone 把面板收进“面板明暗”折叠项，多数人只会设置键盘本身。键盘下次出现时应用，不重建 Engine 会话。同一节顶部还有电脑版外观页的「主题模式」（共享的 `theme`，各处选“跟随”时的默认值）和「设置界面」（`settings_theme`）。iOS 上的设置界面就是这个 App，所以它决定 App 窗口的浅色或深色：选“跟随全局”时看主题模式，主题模式是“跟随系统”时不覆盖，跟随设备外观。改动立即生效，从别的设备同步过来的值在 App 回到前台时应用。

iOS 26 会默认在滚动视图边缘叠加渐隐和模糊。键盘内的候选、拼写、方案、皮肤、工具、表情、手写、AI、语音和回复面板统一通过共享 UIKit/SwiftUI 适配关闭该效果，避免短面板首尾内容被遮盖；iOS 25 及更早版本保持原行为。

繁体输出只在显示与上屏边界转换：候选显示、Engine 提交和手写结果经过 `SharedUI/preferences/ChineseTextConversion.swift`，Engine 的候选原文、候选身份和组合文本保持简体。转换调共享导出 `msime_client_simplified_to_traditional`，即 `crates/client-core/src/chinese_conversion.rs` 的 OpenCC s2t 词级转换，与 Windows、macOS、Linux、Android、HarmonyOS 逐字一致——「头发」出「頭髮」、「发展」出「發展」，`CFStringTransform` 这类逐字转换分不开这两个「发」。日语方案和 R 本地模式保留原文；C ABI 拒绝的输入（内嵌 NUL）保留原文，不丢字。回归测试是 `tests/dictionary/ChineseTextConversionTests.swift`。

“更多”工具面板使用显式分组模型，不从中文标题推断布局或开关语义。根页是表情、剪贴板、AI、语音、本地输入五个入口，加上直接摆在同一页的「设置」分组开关——繁体输出、按键音、按键振动、全角输入和振动强度；这些开关原先藏在「键盘设置」卡片后面，打开面板只看得到六张一样的入口卡，要再点一次才知道按键音开没开。本地输入仍然是二级页：八个模式是一份列表而不是一组开关，摊到根页会把首屏内容挤出键盘高度。

手写方案在真机构建中使用锁定的 ML Kit Digital Ink 8.0.0。模型下载会为键盘扩展创建的后台 URLSession 注入 App Group 共享容器；没有完全访问或共享容器不可用时明确失败，不把模型写入扩展私有临时目录。Apple Silicon 模拟器继续编译不依赖 ML Kit 的同界面 fallback，因为该 SDK 的 arm64 slice 是 device 平台而不是 simulator 平台。fallback 不冒充识别成功，也不沉默：它在自己的状态行上写明「此版本不含手写识别，请使用真机版本」——只画笔画什么都不说，和键盘坏了无从区分。真机构建通过 CocoaPods workspace 链接 SDK。

## 按键延迟

一次按键的成本由 `KeystrokeLatencyTests` 逐键计时，共享层那一侧由 `cargo run -p msime-host-api --example keystroke_latency <verified-resources>` 单独测。两者都报分布而不是均值：掉帧来自尾部，而均值会把它藏在同一个词里那些便宜的键后面。

真机实测（iPhone 17，Release，释义按出厂默认开着）：

| | 2026-09-21 之前 | 现在 |
| --- | --- | --- |
| 普通词字母键 p50 / p95 / max | 6.78 / 16.21 / 26.89 ms | 2.91 / 5.48 / 6.59 ms |
| 高频音节（`yi`）p50 / p95 / max | 21.46 / 30.73 / 51.97 ms | 5.23 / 5.52 / 10.76 ms |
| 空格 p50 | 2.38 ms | 1.50 ms |
| 一次 ABI 往返连同解析 p50 | 0.67 ms | 0.62 ms |

共享运行时不在这条预算里：走真实 ABI 的探针是 p50 0.47ms、p95 1.06ms，并且按组合长度看是平的。

三条改动依次是：把 `local_mode` 与 `nine_key_spellings` 放进快照（此前每键三十次 C ABI 往返，其中二十六次在遍历字母键的循环里）、按输入签名门控 `updateKeyboardLayout`、以及把释义请求去抖 120ms（此前每键遍历全部候选，`yi` 有几百个）。

**测这条路径时先确认释义开关是开的。** 这套测试里别的用例会把它关掉，而设置写在 App Group 里、跨测试运行持续存在；关着测出来的 `yi` 只有 3.3ms，和普通词没区别，问题完全隐形——上面那 21.46ms 就是这么漏掉过一轮的。两个延迟用例现在都显式恢复出厂默认。

## 开发构建

准备交叉编译目标：

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
rustup component add llvm-tools
```

Engine 是工作区内的纯 Rust crate `crates/engine`（`msime-engine`），随 `msime-host-api` 一起编进静态库，不需要取回源码、Boost 或任何依赖前缀；SQLite 由 `rusqlite` 的 `bundled` 特性编进同一个库。

Xcode 27 的 SwiftPM 会把静态库中的 `@_cdecl` 导出内部化；当前 `swift-rs` 构建桥会使用 `llvm-tools` 中的 `llvm-objcopy` 恢复应用 package 的符号。仓库同时固定到上游 PR #79 的提交 `a83e2b2f196e3fa9605cb21c7d3b82652205c279`，使传递嵌入的 SwiftRs runtime 导出在优化构建中保持公开。缺少该组件或移除补丁时，Tauri iOS Rust 动态库会在链接阶段报告 Swift 桥符号未定义。

词库必须来自仓库固定的 `resources/desktop-dictionary.lock.json`。安装器下载并校验发布文件，暂存脚本再次检查名称、长度与 SHA-256，只把允许的六个运行资源复制到 `target/ios/EngineResources`：

```sh
resource_dir="$(cargo run --quiet -p msime-client-core --example install_resources -- target/resources)"
platforms/ios/stage-resources.sh "$resource_dir"
```

第二个可选参数是非英语离线释义目录（默认 `target/offline-glosses`，由 `scripts/build_offline_glosses.py` 生成，见 [docs/third-party.md](../../docs/third-party.md#非英语离线释义resourcesoffline-glosseslockjson)）。其中的 `zh-<语言>.db` 与 NOTICE 暂存到 `target/ios/offline-glosses`，键盘扩展把它作为 EngineResources 的同级目录打包；候选释义语言选了已安装的语言时，该行不需要完全访问或网络即可显示。目录总会创建，没有词典时为空，只有英语走离线释义。

第三个可选参数是粤语与注音的语言词库目录（默认 `target/language-dictionaries`，由 `scripts/fetch_language_dictionaries.py` 按 `resources/language-dictionaries.lock.json` 下载，或由 `msime-dict-build languages` 生成）。其中的 `cantonese.db`、`zhuyin.db` 连同各自的许可证文本暂存到 `target/ios/language-dictionaries`，键盘扩展和测试宿主把它作为 EngineResources 的同级目录打包；数据库缺少许可证文本时脚本报错退出。目录总会创建，没有词库时为空，这时键盘不提供这两个方案。发版构建设 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1`，两个数据库没有都暂存就失败。`build-app.sh` 在暂存前先运行下载脚本；锁里还没有固定发布时它只打印一行跳过信息。

只构建 Rust 宿主库时运行：

```sh
platforms/ios/build-native.sh simulator
```

`device` 目标产出 `target/ios/device/libmsime_host_api.a`，`simulator` 目标产出 arm64 的 `target/ios/simulator/libmsime_host_api.a`。

一条命令完成资源暂存、键盘扩展 native 构建，并构建 iOS 的产品宿主 `MSIMEApp`（同时嵌入键盘扩展）。要改为单独构建 Tauri/React 这个公共组件，在同一条命令前加 `MSIME_IOS_TAURI_COMPONENT=1`：

```sh
platforms/ios/build-app.sh "$resource_dir" simulator
```

## 真机签名构建

产品宿主 `MSIMEApp` 的装机走 XcodeGen 工程加 CocoaPods workspace，不经过 Tauri CLI。`build-app.sh … device` 固定 `CODE_SIGNING_ALLOWED=NO`，只验证编译与打包，产物装不上真机；要装机就直接调 `xcodebuild` 并允许签名：

```sh
resource_dir="$(cargo run --quiet -p msime-client-core --example install_resources -- target/resources)"
python3 scripts/fetch_language_dictionaries.py
platforms/ios/stage-resources.sh "$resource_dir"
platforms/ios/build-native.sh device
python3 scripts/fetch_voice_runtime.py --platform ios
cd platforms/ios && xcodegen generate -s project.yml -p . && pod install --deployment && cd -
xcodebuild -workspace platforms/ios/MSIMEClient.xcworkspace -scheme MSIMEApp \
  -sdk iphoneos -configuration Release -destination 'generic/platform=iOS' \
  -derivedDataPath target/ios/derived-device -allowProvisioningUpdates \
  ARCHS=arm64 ONLY_ACTIVE_ARCH=YES DEVELOPMENT_TEAM=LXCL4Z68GU build
xcrun devicectl device install app --device <udid> \
  target/ios/derived-device/Build/Products/Release-iphoneos/MSIMEApp.app
```

`fetch_voice_runtime.py` 按 `resources/voice-runtime.lock.json` 下载并校验 sherpa-onnx 运行库到 `target/voice-runtime/ios`，`MSIMEApp` 内嵌它做本地语音识别；缺了它 `xcodebuild` 会报 `There is no XCFramework found at '…/SherpaOnnxC.xcframework'`。`-allowProvisioningUpdates` 是必须的：键盘扩展的描述文件要由 Xcode 联网刷新，新设备也在这一步注册进去。App 与 `MSIMEKeyboardExtension` 都以 team `LXCL4Z68GU` 的 Apple Development 证书签名，扩展侧带全部已校验词库，App bundle 约 241 MB。

下面这条是 Tauri/React 公共组件的签名构建，不是 iOS 的产品宿主装机路径：

```sh
cd apps/desktop/src-tauri/gen/apple && pod install --deployment && cd -
APPLE_DEVELOPMENT_TEAM=LXCL4Z68GU \
  pnpm --filter @msime/desktop tauri ios build --target aarch64 --ci
```

Tauri CLI 只把 `APPLE_DEVELOPMENT_TEAM` 应用到它自己的 App target，内嵌的 `MSIMEKeyboardExtension` 与 `MSIMESwiftRsRuntimeExports` 不会继承，签名构建会停在 `Signing for "MSIMEKeyboardExtension" requires a development team`。因此工程里为这两个 target 固定了 `DEVELOPMENT_TEAM`；`--no-sign` 构建不受影响（`CODE_SIGNING_ALLOWED=NO` 时该设置不参与）。需要换团队时在 `xcodebuild` 命令行覆盖同名设置。

首次签名构建前需要在 Xcode 的 Settings → Accounts 里登录该团队的 Apple ID：App 的开发描述文件（含 `group.app.msime.ios` App Group，且已包含目标设备）本机已有，但键盘扩展的 `app.msime.ios.keyboard` 需要由 Xcode 联网创建。没有登录账号时构建会报 `No Accounts: Add a new account in Accounts settings`，并退回到不含 App Groups 能力的通配描述文件。本机的 Xcode 登录该团队之后，`app.msime.ios` 与 `app.msime.ios.keyboard` 的开发描述文件都在本地且包含目标设备。这条路径在 iPhone 17 上走通：`BUILD SUCCEEDED`，`PlugIns/MSIMEKeyboardExtension.appex` 内嵌全部十个已校验运行资源，App 由该团队的 Apple Development 证书签名，`devicectl device install app` 成功，设备上 `devicectl device info apps` 能查到「水杉输入法 / app.msime.ios / 1.0.0」。装完在系统 设置 → 通用 → 键盘 → 键盘 里添加一次「水杉输入法」，扩展即可在任意编辑器里使用。

`devicectl device process launch` 需要设备处于解锁状态，锁屏时会被 `SBMainWorkspace` 以 `Locked` 拒绝（`FBSOpenApplicationErrorDomain error 7`）。

确认改动真的进了产物不要用 `strings`：键面提示是运行时从引擎的 profile 表拼出来的，二进制里没有 `ing uai` 这样的字面量；`@_silgen_name` 引用的 C ABI 名也在链接时解析掉了。用 `nm` 查符号——`MetasequoiaInputSessionBridge.shuangpinKeyHints` 下应当挂着一个 `withUnsafeBytes` 闭包，`msime_client_shuangpin_key_hints` 与 `msime_engine::shuangpin::hints::ShuangpinKeyHint` 应当出现在 Rust 侧的 mangled 符号里，而旧的 `makeShuangpinHints` 应当是 0 个。

Tauri 公共组件的真机产物把最后一个参数改为 `device`。真机构建会在 `apps/desktop/src-tauri/gen/apple` 执行锁定的 CocoaPods 安装，再调用 Tauri CLI；模拟器使用 `aarch64-sim` 并保留手写 fallback。

该目标产出 `apps/desktop/src-tauri/gen/apple/build/arm64/水杉输入法.ipa`：arm64 单架构，`Payload/水杉输入法.app` 内嵌 `PlugIns/MSIMEKeyboardExtension.appex`，扩展侧带锁定 ML Kit Digital Ink 的资源包，App 与扩展各自打包同一份已校验 EngineResources。

模拟器 bundle 默认不带签名，因此没有 App Group 授权：进程一启动就会在共享容器查找上拿到 `client is not entitled`。要在模拟器里真正安装并观察它，用 ad-hoc 签名把既有 entitlements 附上去（模拟器不校验 provisioning，这一步不需要任何开发者证书，也不改变真机的签名边界）：

```sh
app="apps/desktop/src-tauri/gen/apple/build/arm64-sim/水杉输入法.app"
codesign -f -s - --entitlements platforms/ios/KeyboardExtension/Resources/MSIMEKeyboardExtension.entitlements "$app/PlugIns/MSIMEKeyboardExtension.appex"
codesign -f -s - --entitlements apps/desktop/src-tauri/gen/apple/msime-desktop_iOS/msime-desktop_iOS.entitlements "$app"
xcrun simctl install booted "$app"
```

签名后 App Group 查找成功（日志里 `container_create_or_lookup…: success`），但 iOS 27 模拟器上进程仍会在启动时 SIGTRAP，release 与 debug 构建一致，`simctl erase` 后的干净设备上同样复现。崩溃报告里实测到的调用链是 `+[NSBundle bundleWithIdentifier:]` → `_CFBundleGetBundleWithIdentifier` → `_CFBundleEnsureBundleExistsForImagePath` → `__CFBundleCopyFrameworkURLForExecutablePath` → `CFRelease` 的空指针陷阱；应用侧的调用者帧未符号化。据此推断调用方为 `wry::platform_webview_version`：它是依赖树里唯一调用 `bundleWithIdentifier` 的位置，`tauri-runtime-wry` 在 `Wry::init` 中无条件执行 `wry::webview_version().is_ok()`，且 `com.apple.WebKit` 与该函数的错误字符串都能在产物二进制里找到；wry 0.57 的同一函数未改动。这条路径在仓库代码之外，未修改任何 vendored crate。它只影响 Tauri 这个公共组件：产品宿主 `MSIMEApp` 不加载 WebView，在同一台模拟器上正常启动，要在模拟器里看界面走下面那条路。

## 运行 iOS Swift 测试

`KeyboardTests/`、`ServiceTests/` 与 `TransportTests/` 通过 XcodeGen 生成工程的测试宿主在模拟器上运行。准备好 `target/ios/EngineResources` 与模拟器原生库之后：

```sh
xcodegen generate -s platforms/ios/project.yml -p platforms/ios
xcodebuild test -project platforms/ios/MSIMEClient.xcodeproj -scheme MSIMEClientTests \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro Max' \
  -derivedDataPath target/ios/derived-tests \
  CODE_SIGN_IDENTITY=- CODE_SIGNING_REQUIRED=NO CODE_SIGNING_ALLOWED=YES
```

必须允许签名。测试宿主带 App Group entitlement，被测键盘要靠它读共享偏好；用 `CODE_SIGNING_ALLOWED=NO` 构建会剥掉 entitlement，宿主在套件中途被杀，后面的用例全部不报告。模拟器上 `CODE_SIGN_IDENTITY=-` 即 ad-hoc 签名，不需要任何开发者证书。

当前结果为 **229 通过、1 跳过、0 失败**（`MSIMEKeyboardTests` 176 含 1 跳过、`MSIMESharedTests` 38、`MSIMEServiceTests` 16；Xcode 27 / iOS 27.0 模拟器）。**先 `xcodegen generate`**：提交在仓库里的工程会漏掉后加的源文件（实测漏过 `KeyboardAppLauncher.swift`，整套编译不过），所以它不是权威来源，`project.yml` 才是。

跑之前建一台干净模拟器再删掉，不要用手边那台：测试宿主带 App Group，读的是共享容器里的偏好，上一次运行留下的值会改变结果。

```sh
device=$(xcrun simctl create msime-parity-check \
  com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro-Max com.apple.CoreSimulator.SimRuntime.iOS-27-0)
xcrun simctl boot "$device"
# …在上面的 xcodebuild 命令里用 -destination "platform=iOS Simulator,id=$device"…
xcrun simctl delete "$device"
```
这个数字要跟着改动更新：该套件不接入 `verify-local.sh`，没有自动基线，所以这一行是它唯一的基线，写错了就没有别的东西会发现。唯一跳过的是 `CandidateTranslationTests.testCandidateLongPressOffersGlossInsertion` 的「开启释义」分支：固定词库发布里没有该候选的英文释义来源（`translation-glosses.db` 是用户编辑后的覆盖层），取不到释义时跳过而不是报成产品失败，一旦有释义就自动恢复断言。

该套件不接入 `scripts/verify-local.sh`：它需要模拟器和已暂存的词库资源，单次运行约十分钟。

## 界面测试与键盘扩展

`MSIMEClientUITests` 走 `xcodebuild test -scheme MSIMEClientUITests`，需要 `ARCHS=arm64 ONLY_ACTIVE_ARCH=YES`——原生库只有 arm64 切片，不加这两个设置时模拟器目标会按 x86_64 链接并在扩展上报未定义符号。

`KeyboardExtensionEditorUITests` 是唯一把键盘扩展当成系统键盘来用的一组用例：其余所有套件都直接在测试宿主里构造 `KeyboardViewController`，那条路覆盖不到只在运行期才存在的部分——系统是否真的加载这个扩展、扩展进程能否读到 App Group 共享容器、上屏文本是否真的到达别人的 `UITextDocumentProxy`。它要求键盘已在「设置」里启用；没启用时跳过而不是失败，并在跳过信息里带上当时键面上有什么。

**在 iOS 27 模拟器上，键盘能装上但切不过去。** 启用和切换是两件事，要分开看：

- **启用是成功的。** 往 `.GlobalPreferences` 写 `AppleKeyboards`（加上扩展 bundle id `app.msime.ios.keyboard`）并重启模拟器之后，设置 → 通用 → 键盘 → 键盘 里确实列着「水杉输入法 · 中文」。要确认这一点得走 Settings 把每一层的单元格文案打出来看，只凭「键盘环里没有它」会误判成写入无效。
- **切换是失败的。** XCUITest 到不了它。`app.buttons["Next keyboard"]` 点下去落在 shift 上（键面在 Q/q 之间来回，始终是同一个 `UIKeyboardLayoutStar`），长按它弹出的是单手键盘的「默认/右手/左手」菜单，里面一个键盘名字都没有——连已启用的简体拼音和英语都没有。`app.keyboards.buttons` 只有 `shift` / `emoji` / `Return` 三个。

把扩展排到 `AppleKeyboards` 数组第一位也不会让它成为默认键盘：新编辑框打开的仍是第一个**系统**键盘（实测是简体拼音），iOS 不会为第三方键盘做默认。所以模拟器上到不了「真实编辑器」这一级，差的就是那一下人手切换。

另外两条与启用无关，试过也无效，不必再走：`pluginkit -e use -i app.msime.ios.keyboard`（扩展本来就注册为 `com.apple.keyboard-service`，置 `+` 能跨重装保持，但和键盘环无关），以及 `App-prefs:General&path=Keyboard` 深链（命令返回成功，界面停在设置首页不跳转）。

真机上就是正常在 设置 → 键盘 里添加一次，之后这组用例会自己跑起来。

**当前结果为 39 执行、2 跳过、0 失败**（917 秒）。两条跳过是上面那组键盘用例，等键盘在设置里启用后才会真正执行。

写这组用例时踩过三类坑，都不是产品坏了，记在这里因为很容易再踩：

- **断言了不存在的导航栏标题。** 早先首页、我的、打字统计都用 `navigationTitle("")`，名字放在内容里；现在四个标签页的根都有大标题（设置、社区、统计、我的），可以用 `app.navigationBars["设置"]` 这类断言。但推入的设置页标题会随设计改名（「皮肤」改成了「主题」），判断「在哪一屏」时优先用选中的标签页加上只有那一屏才有的元素。
- **入口搬了家。** `voiceSettingsLink` 现在是设置根列表「键盘 / 语音输入 / 手写输入」一组里的一行（`SettingsPage.voice`），用 `reachSettingsLink` 走到。同名标识符还留在 `KeyboardSettingsView` 上，而那个页面已经没有任何地方实例化——它是 `ce4a76844` 把入口搬到首页时留下的孤儿，朝它伸手会让「搬家」看起来像「缺失」。同理，主题页的键盘预览、`customSkinEditorLink` 和 `skinCommunityLink` 在候选预览和主题网格下方，手机上要用 `tapRevealed` / `reveal` 滚到再点。
- **滚动没有真的发生。** SwiftUI 的 `Form` 是惰性列表，折线以下的行不在无障碍树里，所以「先 `waitForExistence` 再滚」永远等不到也永远不滚；而 `app.swipeUp()` 从屏幕中心起手，在 键盘设置 页那是键盘预览，那块把上下拖动解释成**改行间距**——表单不动，还顺手改掉了马上要读的设置。要边滚边看，并且用坐标拖拽把起点压在预览以下、标签栏以上。

`MSIMEApp` 是 iOS 的产品宿主，装机以它为准，也是 `build-app.sh` 的默认产物，不需要任何开关。要单独构建 Tauri/React 这个公共组件时用 `MSIME_IOS_TAURI_COMPONENT=1` 显式选择。

**上面那条 SIGTRAP 只挡 Tauri 宿主，不挡这个。** `MSIMEApp` 不加载 WebView，在 iOS 27 模拟器上界面能正常起来，键盘扩展也随它一起装进去，所以要在模拟器上看界面就走这条路：

```sh
platforms/ios/build-native.sh simulator
xcodebuild -project platforms/ios/MSIMEClient.xcodeproj -scheme MSIMEApp \
  -destination 'platform=iOS Simulator,name=<设备名>' \
  -derivedDataPath target/ios/derived-sim CODE_SIGNING_ALLOWED=NO build
xcrun simctl install booted target/ios/derived-sim/Build/Products/Debug-iphonesimulator/MSIMEApp.app
xcrun simctl launch booted app.msime.ios
```

需要 `target/ios/EngineResources` 已按上面的暂存步骤就位，否则运行时找不到词库。

共享 Tauri iOS 工程的 CocoaPods workspace 与 `Pods` 目录只为真机 ML Kit 构建生成，不提交到仓库；模拟器从干净的 XcodeGen 工程直接构建 fallback。真机目标需要先安装锁定的 CocoaPods 依赖，然后从 Tauri CLI 启动构建；CLI 会为 Xcode 中的 Rust 构建脚本建立本地控制通道，因此不要直接用独立的 `xcodebuild` 命令代替：

```sh
cd apps/desktop/src-tauri/gen/apple
pod install --deployment
cd ../../../../..
pnpm --filter @msime/desktop tauri ios build --target aarch64 --no-sign --ci
```
