# Agent Note: Windows 打字特效对齐 macOS：光标处的火花浮层、Power Mode 抖动、交给应用的键也出声

Status: implemented

## Problem

macOS 的打字特效面板（`platforms/macos/src/input/TypingEffectPanel.mm`）在光标处迸出火花。Power Mode 的火花更多、更快，还会抖一下候选窗。特效包的颜色一键换一个，火花数照包里的 `particles`。连击徽标升档时弹一下。候选窗不在屏幕上时（刚上屏、交给应用的键），闪光改画在光标所在的行，徽标画在光标旁。Windows 只有候选卡片的闪光，`TypingEffectPolicy.h` 自己写着「没有粒子浮层」。火花和 Power Mode 都只是闪得更亮，特效包只用第一个颜色。连击数挤在拼音行里，候选窗一收起就什么都看不到。共享设置页因此对 Windows 打开了 `typingEffectsFlashOnly`，样式说明不敢提火花。

按键音也少了一截。macOS 每个按键都出声，不管输入法处理没处理（`InputController.mm` 的 `reportKeyToLifecycle:`，接入生命周期核心前叫 `playKeySound:`），连击也数这些键。Windows 的 TIP 只把输入法接手的键转给 Server，没有组字时的空格、回车、退格、数字和方向键直接交给应用，从不经过 `FocusedSession::configured_key`。所以开着音效包时这些键没有声音，连击也不算它们。

## Decision

**光标处的浮层。** `src/candidate/TypingEffectOverlay.cpp` 是一个不激活、不进任务栏的置顶分层窗口，样式是 `WS_EX_LAYERED | WS_EX_TRANSPARENT`，`WM_NCHITTEST` 也回 `HTTRANSPARENT`，点击全部穿到下面的应用。每帧 Direct2D 画进一张 DIB（`ID2D1DCRenderTarget`，96 DPI，单位就是物理像素），再用 `UpdateLayeredWindow` 一次更新窗口的位置和内容。它画三样东西：

- 光标处的火花。起点是光标行的左端、行高中间，没有可用的光标时从卡片左上角往里 12 处。
- 候选窗不在时光标所在行的闪光。
- 「连击 ×N」徽标。有卡片时贴在卡片右上角上方，在候选窗外面；没有卡片时在光标右上方。徽标夹在显示器里，升档时按 1、1.35、0.95、1 弹一下，Power Mode 每键按 1、1.12、1 轻弹一下。

浮层排在候选窗下面（`SetWindowPos` 插在候选窗之后），火花不挡候选。所有数值照搬 macOS，写在纯头文件 `src/candidate/TypingEffectOverlayPolicy.h`：火花寿命 0.45±0.15 秒，初速 120±60（Power Mode 170±60），向上 ±36° 喷出，受 320 的重力，透明度每秒降 1.8，大小 16×(0.45±0.2)，每秒缩 0.6。迸发时长按键 60 毫秒、上屏 100 毫秒。没有特效包时颗数按 macOS 的发射率 220×强度×倍数每秒算，强度 50 时一键 9 颗，Power Mode 和上屏各加倍；特效包给了 `particles` 就每键这么多颗，上屏加倍，0 不画火花。收起时间：徽标在时 1.2 秒，否则取 0.7 秒和闪光长度里大的那个。

**候选窗仍是 `TypingEffectSignal` 唯一的消费者。** 它取到特效后自己画卡片闪光和 Power Mode 的抖动。抖动用渲染变换做：整张卡片横向按 0、+a、-a、+a/2、0 位移 0.12 秒，a 随强度从 0.5 到 2 个设备无关像素，落在阴影的透明边距里，窗口不挪，点击区域不变。其余部分组成 `TypingEffectPresentation` 交给 `set_typing_effect_presenter` 注入的浮层，内容有：特效、设置、特效包颜色、卡片的屏幕矩形、最近一次组字的锚点（前台没换时才给）、强调色（后来改成系统强调色，见[工具栏、提示与托盘对齐 macOS](2026-10-20-windows-toolbar-hud-tray-mac-geometry.md)）、「显示动画」和节电模式。浮层可用时卡片不再在拼音行里画连击数，火花样式也不再闪卡片（`typing_effect_card_flashes`），和 macOS 一样，只有闪光和 Power Mode 闪卡片。浮层建不出来或失败后，卡片退回从前的画法：每种样式都闪，连击数画在拼音行里。

**特效包的整套参数随按键发布。** `ServerSession::refresh_typing_effect_settings` 在读设置时多读出全部颜色（最多 4 个）和 `particles`，存成 `TypingEffectPalette`。`TypingEffectSignal::publish_palette` 把它打成两个 64 位字，和原来的设置字一样不加锁、只留最新。上屏的答案由宿主打上 `typing_effect_commit_mark`（第 24 位），界面线程据此画加倍的火花和 250 毫秒的闪光。这一位和新档位一样，在界面线程取走之前会带到下一个值上，上屏后紧跟的一个键不会吞掉上屏的那一下，和 macOS 合并未画特效的做法一致。

**交给应用的键走 Aux 管道。** TIP 在 `OnTestKeyDown` 判定不吃这个键的三个出口调用 `_NotePassthroughKeySound`，按 `common/KeySoundClass.h` 的 `passthrough_key_sound_class` 分类。它和 Server 的 `key_sound_class` 共用一张键表，下面这些不出声：自动重复、Ctrl/Alt/Windows 组合键、单独的修饰键、英文模式、停用的键盘、TSF 安全模式、面板注入的文字。同一次按下被探测多次时，按扫描码加消息时间去重。要出声的键进 `IPC/PassthroughKeySoundQueue.cpp` 的定长环形队列（8 个，不分配内存），由线程池发 `KeySound|<client>|<token>|<class>`（`AuxMessage.h`）并等 "OK"，排队超过 250 毫秒的键丢掉。Server 的 `AuxListener` 核对发送方进程号等于 client 的高 32 位，然后：

- 按键音或打字特效开着（`TypingFeedbackPreference.h` 的 `typing_feedback_wanted`，随偏好发布更新）时，回 "OK"，并通过 `SessionController::passthrough_key` 把任务排进输入队列，不等它执行。
- 都关着时不排，也不回 "OK"，TIP 停发 10 秒。

输入线程上 `FocusedSession::passthrough_key` 只认此刻以这个焦点令牌持有焦点、不在英文模式的会话，带 `MSIME_KEY_PASSTHROUGH` 交给生命周期核心（下一节）：和 Server 接手的键一样出按键音、计入连击，全屏应用在前台时不出声，但不开新的一轮，因为应用已经处理过这个键。

**声音和特效的时机交给生命周期核心。** 桌面生命周期核心（[2026-10-11-desktop-lifecycle-core](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md)）的第 2 阶段里，Server 的每个会话一建好就调 `msime_client_set_lifecycle(FEEDBACK | HOST_HOLDS_PIECES)`（`src/input/LifecyclePolicy.h`），旧的 `key_sound`、`commit_sound`、`music_set_active` 和 `typing_effect`（事件 0-4）对它变成空操作，Server 也不再调。TIP 在应用进程里的宿主会话从不选择加入，也从不调用 `msime_client_set_lifecycle`、`msime_client_key`、`msime_client_set_field_traits`、`msime_client_set_active`，`tsf/tests/input/lifecycle_boundary.cpp` 按源码守着这一条。

- **按键**：`FocusedSession::configured_key` 先比较一次字段属性，变了先报，再调 `msime_client_key`，然后才把键交给引擎。按键音因此排在引擎处理之前。返回的打字特效照旧经 `TypingEffectSignal` 交给候选窗。自动重复带 `MSIME_KEY_REPEAT`。Ctrl/Alt 组合键和单独的修饰键没有按键类别，不调 `msime_client_key`，也就不开新的一轮；头文件的轮次契约要求每个物理按下都报，但没有「不出声的键」这种编码，第 3 阶段按轮计 fuel 时要回来补。鼠标点选和翻页先报 `MSIME_KEY_POINTER`。
- **上屏**：上屏音由核心在派发时出，上屏特效取自派发回复的 `"feedback":{"typing_effect":N}`（`commit_feedback`），也在派发时发布；`record_commit` 只记统计，成就音是否静音看这次上屏前报给核心的字段属性。部分选词由 Server 暂存在前缀里（`HOST_HOLDS_PIECES`）：暂存的那一段还没进文档，核心不为它出上屏音，Server 也不画上屏特效（回复的 `committed_text` 为空，那次回复里为 0 的特效画出来会把连击徽标清零）；整段送出的那次上屏出一次声、画一次，和从前一样一次上屏一次提交音，只是时机从送达提前到派发。云候选、英文、快捷短语、表情、颜文字和整句（`CandidateCompletionPolicy.h`）是例外：选中后 Server 用 `MSIME_CANCEL` 丢掉剩下的读音、整段上屏，不暂存，可核心只看到「上屏了、读音还剩」，带着 `HOST_HOLDS_PIECES` 会把它当作暂存的一段，这次整段上屏就既不响也不画，还会用 0 把连击徽标清零；所以 `ServerSession::select` 在这次选词期间去掉这一位，选完恢复。Server 自己合成的上屏（Ctrl+Enter 和释义列的译文、以词定字退回到取首尾字）不是核心的上屏事件，不出声也不画；这些上屏（还有翻译页的选择）都改用命令 19 `MSIME_CANCEL_AFTER_HOST_COMMIT` 丢弃组字，生命周期里组字的结束原因记 HostCommit 而不是 Cancel；以词定字的回退还去掉那次派发回复里「没有上屏」的 feedback，免得合成的上屏被当成一次特效为 0 的上屏、把连击徽标清零。
- **TIP 已经写进文档的上屏**：韩文、注音、越南文、藏文（`scheme::AlwaysInlinePreedit`）、全拼和双拼整句改字（派发前 view 的 `conversion` 非空），以及没有候选时的本地回车（`ReplyPath::LocalCommit`，TIP 把读音原样插进文档），上屏由 TIP 从自己的宿主会话写出。Server 在这次派发前后打开 `COMMIT_OBSERVE_ONLY`：前两类按派发前的 view 在 `FocusedSession::configured_key` 里打开；本地回车要等路由选定才知道，由 `ReplyComposer::dispatch` 按路径（`LocalCommit`、`ConversionCommit`、`SyllableCommit`）打开。守卫 `CommitObserveOnly`（`src/ipc/ServerSession.h`）可以嵌套，外层已经打开时内层什么都不做。这些上屏只能观察，将来的脚本改写不了。声音上这一点也成立：上屏音只来自 Server 这一个会话的核心，TIP 的会话不选择加入，同一次上屏不会在两边各响一遍。
- **全屏和音乐**：「全屏应用在前台」不再由 Server 自己决定出不出声，而是作为 `FULLSCREEN` 字段属性交给核心：声音和音乐静音，特效和连击照常，和从前一致。背景音乐改由 `msime_client_set_active` 仲裁所有者：`prepare` 先算好此刻的属性再获得焦点，再 `set_active(true)`，`cancel` 时 `set_active(false)`，会话销毁时核心自己关掉音乐，偏好更新后仍持有焦点就再报一次 `set_active(true)`。全屏前台每个键都重新判断，进出全屏时音乐跟着变，不再只在获得焦点时判断一次。
- **获得焦点前确认属性**：脚本运行时只在获得焦点之后字段属性经过确认时才把 `focus_in` 交给脚本，确认是获得焦点之前的一次 `set_field_traits`（值相同也算）或者之后的第一个键。`ServerSession` 对属性去重，激活之后才报的话 `focus_in` 要等第一个键，几乎永远到不了脚本；所以 `activate` 和切回中文的 `set_input_enabled(true)` 在每次 `msime_client_focus(true)` 之前把当前属性原样再报一次（`confirm_field_traits_before_focus`），不去重。

**字段属性经 Main 管道从 TIP 报给 Server。** 新增 Main 事件 `FieldTraits`（17）和可选能力位 `FanyImeProtocol::FieldTraits`（`1u << 6`，不进默认宣告和必需集合）。协商到之后，TIP 在每个 ClientActivated、按键、状态快照和 FocusRestored 之前比较一次属性，变了才发，换了 Main 连接（每次协商成功）后重发；属性排在 ClientActivated 前面，Server 在 `prepare` 里先报属性再 `set_active(true)` 时用的就是这次焦点的属性，而不是这个客户端上次报的。重连和补发键这两条会发 ClientActivated 的路径在激活前也先记一次属性；写在 `SendToNamedpipe` 的最终检查之后、目标包之前，协商结果到那时才有。载荷 `FanyImeFieldTraits`：停用的键盘、空上下文（`_IsKeyboardDisabled()`，密码框就是这样到达 TIP 的）和安全桌面（`TF_TMAE_SECUREMODE`）是 `SECURE`；`TokenElevation` 报提权的客户端进程是 `ELEVATED`，令牌查不出来（`OpenProcessToken` 或 `GetTokenInformation` 失败）时也按提权报（`FanyImeFieldTraits::ProcessElevated`）：提权窗口里不运行脚本靠的就是这一位，少报会让它落空，而 `ELEVATED` 不让声音和特效静音，多报一次用户察觉不到；每个进程只查一次，缓存放在常量初始化的原子量里，因为 TSF DLL 用 `/Zc:threadSafeInit-` 编译。焦点换进密码框时 TIP 会发状态快照（`TIMER_FOCUS_STATUS_RESEND`），所以不用等一个永远不会到来的按键，音乐在进入密码框时就停。

Server 把这个事件当作属于客户端的通知：`FocusRouter` 不管它是否持有焦点都收下，不认领焦点、不带路由；`SessionPump` 直接交给 `InputState::set_field_traits`，记在客户端上，词库维护后重建的 `FocusedSession` 也拿得到。交给核心时 Server 加上 `KNOWN`，再并上自己看到的 `FULLSCREEN`（`server_field_traits`）。帧里只能出现 `SECURE` 和 `ELEVATED` 两位，别的都是非法帧。新 TIP 配旧 Server、旧 TIP 配新 Server 都协商不到这一位，TIP 不发（旧 Server 会把未知事件当非法帧断开连接），Server 的属性保持未知、不带 `KNOWN`，只有全屏前台照常生效。`ELEVATED` 只交给核心记作非普通字段（上屏不可改写、将来不跑脚本），不像 `SECURE` 那样静音：在管理员身份的终端里打字照常出声。

**动画开关和节电。** 「显示动画」（`SPI_GETCLIENTAREAANIMATION`）关掉时什么都不闪、不动、不弹，只留徽标，这是 Windows 卡片一直以来的规则。macOS 的「减弱动态效果」只去掉移动，闪光照常。开着节电模式时（`GetSystemPowerStatus` 的 `SystemStatusFlag`），火花和 Power Mode 退回闪光、卡片不抖，对应 macOS 的低电量模式。全屏应用在前台时浮层什么都不画。

**帧开销。** 浮层用自己的一把画刷，每笔改颜色和不透明度，不经 msimeui `DeviceResources` 只增不减的画刷缓存。火花向量一次预留 256 颗，发射和清理都不扩容。动画都停了、只剩静止的徽标时停掉帧计时器，换成一次性的收起计时器。候选卡片闪光的不透明度量化成 1/32 一档（`typing_effect_quantized_alpha`），以前每帧一个新透明度、缓存就多一把画刷，现在一次闪光最多几十把。共享设置页的 `typingEffectsFlashOnly` 只剩 HarmonyOS。

## Alternatives considered

- **用 DirectComposition 画浮层（`WS_EX_NOREDIRECTIONBITMAP` 加 `DeviceResources::EnsureForComposition`）**：候选窗、中英文提示、双拼键位图都这样画，能复用 msimeui，GPU 合成也最省。没有采用，原因有两个。点击穿透在 Windows 上要靠分层窗口的 `WS_EX_TRANSPARENT`，`WM_NCHITTEST` 回 `HTTRANSPARENT` 只对同一线程的窗口有效；浮层就盖在光标上，打字后一秒内挡住点击是用户直接碰得到的问题。另外 DirectComposition 在分层窗口上的行为没有把握。DIB 加 `UpdateLayeredWindow` 是 Windows 上分层窗口的标准做法，远程桌面和 Wine 也能用。浮层最大不过几百像素见方，每帧拷一次的代价可以接受。
- **把抖动做成挪动候选窗**：最像审计里写的 `SetWindowPos` 抖动。但抖动期间 `reposition()` 也可能挪窗口，两边会互相打架，留下错位；挪窗口还会让 Windows 重新算命中和置顶。用渲染变换在窗口里位移，抖完回到原位，什么状态都不留。
- **交给应用的键也走 Main 管道**：Main 管道有握手、有焦点租约，Server 拿到的会话最可靠。可它是同步的请求和回复，TIP 的按键线程要等 Server 答复，正是按键路径要避免的延迟。Aux 管道一条消息一个连接，在线程池上发，按键路径上只有一次入队。焦点令牌和进程号核对补上了认证。
- **字段属性跟着焦点租约走，每次激活重置**：属性看起来是「这一次焦点」的事，激活时清零最不容易留下旧值。没有采用：TIP 只在属性变了或换了连接时才发，激活时 Server 清零而 TIP 不知道，属性就一直是未知；而且密码框里没有按键，状态快照又可能落在 Server 认定的焦点切换之前，被当作非焦点消息丢掉。属性记在客户端上、不经焦点，TIP 每次换字段都报，旧值最多活到下一次快照或按键。
- **把字段属性塞进按键包（比如 `modifiers_down` 的空闲位）**：不加新事件，也不用协商。没有采用：密码框里根本没有按键到达 Server，音乐要在进入密码框时就停，靠按键包做不到；`modifiers_down` 的高位已经被 `UiLess`、`GameHost`、`CandidateActive`、`AutoRepeat` 占着，StatusSnapshot/FocusRestored 又把它当全角状态读。
- **`ELEVATED` 按 `SECURE` 处理，提权窗口里一律静音**：设计稿写的是「Server 按 SECURE 处理」，最省事也最保守。没有采用：那句话说的是脚本不在提权窗口里运行，核心的「普通字段」判断本来就把 `ELEVATED` 排除在外，已经做到了；把它也当成 `SECURE` 会让在管理员终端里打字的人失去按键音和音乐，换来的只是不泄露自己在自己的终端里打字的节奏。
- **Server 不回 "OK"、TIP 一直发**：最简单。但按键音和特效默认都关着，那样每个用户、每个交给应用的键都要连一次管道。由 Server 按偏好回答，功能关着时 TIP 停发 10 秒，开关打开后最晚 10 秒开始出声。

## Consequences

- **收益**：Windows 的火花、Power Mode、多色特效包、徽标弹跳和候选窗不在时的特效都和 macOS 一致；设置页的样式说明不再为 Windows 改写。没有组字时的空格、回车、退格、数字也出按键音、计入连击。数值集中在一个纯头文件里，主机上可以单测。
- **生命周期接入的收益与代价**：按键音挪到引擎处理之前，全屏判断和背景音乐都交给共享的核心，Windows 和其他桌面宿主走同一套规则；进入密码框时背景音乐停止。代价是每个按键多一次 `msime_client_view`（判断这次的上屏是不是 TIP 已经写进文档的），TIP 写进文档的那类派发前后多两次 `msime_client_set_lifecycle`；Server 合成的上屏（译文、释义列、以词定字的回退）不再出上屏音、不再闪上屏特效。`_IsKeyboardDisabled()` 在焦点不在任何可编辑区域、或者应用给非编辑区挂了空上下文时也为真，这时 TIP 同样报 `SECURE`，背景音乐会在离开文本框时暂停、回来时继续。全屏前台改成每个键都判断以后，同一次焦点里进入全屏，打第一个键音乐就停，退出全屏打第一个键再响（从前只在获得焦点时判断一次，按键音则一直是每个键判断）。这两项不在设计稿列出的五项可感知变化里，合并前要用户确认；不接受时，前一项只能连同密码框一起放弃（Chromium 这类应用的密码框和非编辑区挂的是同一种停用上下文，TIP 分不出来），后一项会让进入全屏后按键音也不再静音，因为两者用的是同一个属性位。
- **代价与已知上限**：光标位置先取前台线程的系统光标（`InputModeHudWindow::system_caret`，只认每显示器 DPI 感知的窗口）。取不到时用最近一次组字的锚点往上估 20 个设备无关像素一行。不建系统光标的应用里，交给应用的键的火花落在上次组字的位置，不在真正的光标处。macOS 的火花是加法混合的软圆点贴图，这里用两层不同不透明度的圆近似。全屏应用在前台时卡片闪光仍照 Windows 原来的行为画，没有像 macOS 那样一并关掉。交给应用的键在 Server 卡住时最多晚到 250 毫秒，再晚就丢。带 Ctrl、Alt 的组合键和单独的修饰键（Ctrl+退格恢复词段、Ctrl+Enter 上屏译文、Ctrl+左右移词段等）会派发引擎，却不调 `msime_client_key`，算进了上一个键的轮次，偏离头文件的轮次契约；现在听不出差别，第 3 阶段按轮计 fuel 之前要在头文件里加一个「开新的一轮、不出声不画」的按键编码，再让这些键先报它。以下几点只在 macOS 上做过 MinGW 语法检查和纯策略的本机测试，要在 Windows 真机上确认：浮层的点击穿透、层级、帧率和观感，交给应用的键的出声延迟。若真机上分层窗口每帧上传的开销明显，或者火花被候选窗以外的置顶窗口挡住，就要重访第一条备选。

- **以下几点只做过 MinGW 交叉编译和本机桩测试，要在 Windows 真机的图形会话里验收**：进入密码框时背景音乐停止、离开后恢复；以管理员身份运行的窗口上报 `ELEVATED`、照常出声；新 TIP 配旧 Server、旧 TIP 配新 Server 都能正常打字、不断开；整句改字、韩文、注音的上屏只响一次；全屏前台进出时音乐跟着变。

## Verification

- `platforms/windows/tests/candidate/typing_effect_overlay_policy.cpp`（`windows-typing-effect-overlay-policy`）：颗数、颜色轮换、轨迹范围、闪光、抖动、弹跳、摆放、调色板打包和上屏标记。
- `platforms/windows/tests/input/key_sound_policy.cpp`：Server 侧的键表改用共享表之后行为不变；交给 `msime_client_key` 的值（自动重复、PASSTHROUGH）、Server 报给核心的字段属性、回复里的上屏特效、哪些方案和状态算 TIP 已经写进文档。
- `platforms/windows/tests/input/lifecycle_call_order.cpp`（`windows-lifecycle-call-order`）：测试自己实现 Server 用到的 `msime_client_*`、记下每次调用，在 `FocusedSession` 的真实按键路径上核对顺序：建会话后立即选择加入；全屏判断由测试注入（`set_fullscreen_check`），属性在两次调用之间变化时先于按键、先于 `set_active(true)` 报给核心，每次 `focus(true)`（激活、切回中文）都紧挨在一次 `set_field_traits` 之后，按键先于引擎；PASSTHROUGH 不派发；韩文、整句改字和本地回车的上屏前后开关 `COMMIT_OBSERVE_ONLY`；以词定字的回退用命令 19，候选窗最后拿到的仍是按键特效；Ctrl+Enter 只有一条释义时同样用命令 19；空格上屏发布打了上屏标记的特效，部分选词暂存的那一段不发布、整段送出时才发布；整段上屏的候选来源选中后读音还剩一截时，这次选词前后去掉、恢复 `HOST_HOLDS_PIECES`，并照常发布上屏特效；点选先报 POINTER；失焦先交还音乐；旧的四个调用一次都不出现。再经 `InputState` 核对属性记在客户端上、旧票据报来的不算、词库维护后重建的会话一建好就拿到属性。去掉其中任何一处实现（派发里的守卫、两处先报属性、`committed_text` 判断、去掉 feedback、整段上屏时去掉 `HOST_HOLDS_PIECES`、重建时补报、票据核对），测试都会失败。CI 的 Windows 交叉编译只编不跑，这个测试和下面几个 Windows 测试要在 Wine 或 Windows 上执行。
- `platforms/windows/tsf/tests/input/lifecycle_boundary.cpp`（`msime-tsf-lifecycle-boundary`）：TIP 源码里不出现生命周期的四个 FFI；字段属性在按键、状态快照、补发键和重连激活之前记下，排在 ClientActivated 前面发，协商到能力位才发，换连接后重发。
- `crates/host-api/src/lifecycle/tests.rs` 的 `a_piece_the_host_holds_sounds_only_when_the_whole_phrase_goes_out`：`HOST_HOLDS_PIECES` 下部分选词那一段不出上屏音、特效为 0，事件照常发，整段送出时出一次；没打开这一位的会话照常出声。
- `shared/contracts/tests/windows_ipc_contract.cpp`、`platforms/windows/tests/core/main_frame.cpp`、`platforms/windows/tests/runtime/focus_router.cpp`、`platforms/windows/tests/runtime/session_pump.cpp`：`FieldTraits` 的事件号、能力位和三种新旧组合的协商，帧校验，不认领焦点，以及收下它不断开连接、带 `feedback` 的回复照样被合成。
- `platforms/windows/tests/input/typing_feedback_preference.cpp`：什么偏好下回 "OK"。
- `platforms/windows/tsf/tests/passthrough_key_sound.cpp`（`msime-tsf-passthrough-key-sound`）：TIP 侧哪些键出声；类别都在 `msime_client_key` 接受的 0-3 里；交给应用的键在哪里不出声，TIP 报的字段属性就在哪里是 `SECURE`。
- `platforms/windows/tests/runtime/aux_message.cpp`：`KeySound` 消息的往返和拒绝。
- `apps/desktop/tests/settings/plugins-section.test.tsx`：Windows 的样式说明提到火花，`typingEffectsFlashOnly` 只对 HarmonyOS 为真。
