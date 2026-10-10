# Agent Note: Windows 打字特效对齐 macOS：光标处的火花浮层、Power Mode 抖动、交给应用的键也出声

Status: implemented

## Problem

macOS 的打字特效面板（`platforms/macos/src/input/TypingEffectPanel.mm`）在光标处迸出火花。Power Mode 的火花更多、更快，还会抖一下候选窗。特效包的颜色一键换一个，火花数照包里的 `particles`。连击徽标升档时弹一下。候选窗不在屏幕上时（刚上屏、交给应用的键），闪光改画在光标所在的行，徽标画在光标旁。Windows 只有候选卡片的闪光，`TypingEffectPolicy.h` 自己写着「没有粒子浮层」。火花和 Power Mode 都只是闪得更亮，特效包只用第一个颜色。连击数挤在拼音行里，候选窗一收起就什么都看不到。共享设置页因此对 Windows 打开了 `typingEffectsFlashOnly`，样式说明不敢提火花。

按键音也少了一截。macOS 每个按键都出声，不管输入法处理没处理（`InputController.mm` 的 `playKeySound:`），连击也数这些键。Windows 的 TIP 只把输入法接手的键转给 Server，没有组字时的空格、回车、退格、数字和方向键直接交给应用，从不经过 `FocusedSession::configured_key`。所以开着音效包时这些键没有声音，连击也不算它们。

## Decision

**光标处的浮层。** `src/candidate/TypingEffectOverlay.cpp` 是一个不激活、不进任务栏的置顶分层窗口，样式是 `WS_EX_LAYERED | WS_EX_TRANSPARENT`，`WM_NCHITTEST` 也回 `HTTRANSPARENT`，点击全部穿到下面的应用。每帧 Direct2D 画进一张 DIB（`ID2D1DCRenderTarget`，96 DPI，单位就是物理像素），再用 `UpdateLayeredWindow` 一次更新窗口的位置和内容。它画三样东西：

- 光标处的火花。起点是光标行的左端、行高中间，没有可用的光标时从卡片左上角往里 12 处。
- 候选窗不在时光标所在行的闪光。
- 「连击 ×N」徽标。有卡片时贴在卡片右上角上方，在候选窗外面；没有卡片时在光标右上方。徽标夹在显示器里，升档时按 1、1.35、0.95、1 弹一下，Power Mode 每键按 1、1.12、1 轻弹一下。

浮层排在候选窗下面（`SetWindowPos` 插在候选窗之后），火花不挡候选。所有数值照搬 macOS，写在纯头文件 `src/candidate/TypingEffectOverlayPolicy.h`：火花寿命 0.45±0.15 秒，初速 120±60（Power Mode 170±60），向上 ±36° 喷出，受 320 的重力，透明度每秒降 1.8，大小 16×(0.45±0.2)，每秒缩 0.6。迸发时长按键 60 毫秒、上屏 100 毫秒。没有特效包时颗数按 macOS 的发射率 220×强度×倍数每秒算，强度 50 时一键 9 颗，Power Mode 和上屏各加倍；特效包给了 `particles` 就每键这么多颗，上屏加倍，0 不画火花。收起时间：徽标在时 1.2 秒，否则取 0.7 秒和闪光长度里大的那个。

**候选窗仍是 `TypingEffectSignal` 唯一的消费者。** 它取到特效后自己画卡片闪光和 Power Mode 的抖动。抖动用渲染变换做：整张卡片横向按 0、+a、-a、+a/2、0 位移 0.12 秒，a 随强度从 0.5 到 2 个设备无关像素，落在阴影的透明边距里，窗口不挪，点击区域不变。其余部分组成 `TypingEffectPresentation` 交给 `set_typing_effect_presenter` 注入的浮层，内容有：特效、设置、特效包颜色、卡片的屏幕矩形、最近一次组字的锚点（前台没换时才给）、主题强调色、「显示动画」和节电模式。浮层可用时卡片不再在拼音行里画连击数，火花样式也不再闪卡片（`typing_effect_card_flashes`），和 macOS 一样，只有闪光和 Power Mode 闪卡片。浮层建不出来或失败后，卡片退回从前的画法：每种样式都闪，连击数画在拼音行里。

**特效包的整套参数随按键发布。** `ServerSession::refresh_typing_effect_settings` 在读设置时多读出全部颜色（最多 4 个）和 `particles`，存成 `TypingEffectPalette`。`TypingEffectSignal::publish_palette` 把它打成两个 64 位字，和原来的设置字一样不加锁、只留最新。上屏的答案由宿主打上 `typing_effect_commit_mark`（第 24 位），界面线程据此画加倍的火花和 250 毫秒的闪光。这一位和新档位一样，在界面线程取走之前会带到下一个值上，上屏后紧跟的一个键不会吞掉上屏的那一下，和 macOS 合并未画特效的做法一致。

**交给应用的键走 Aux 管道。** TIP 在 `OnTestKeyDown` 判定不吃这个键的三个出口调用 `_NotePassthroughKeySound`，按 `common/KeySoundClass.h` 的 `passthrough_key_sound_class` 分类。它和 Server 的 `key_sound_class` 共用一张键表，下面这些不出声：自动重复、Ctrl/Alt/Windows 组合键、单独的修饰键、英文模式、停用的键盘、TSF 安全模式、面板注入的文字。同一次按下被探测多次时，按扫描码加消息时间去重。要出声的键进 `IPC/PassthroughKeySoundQueue.cpp` 的定长环形队列（8 个，不分配内存），由线程池发 `KeySound|<client>|<token>|<class>`（`AuxMessage.h`）并等 "OK"，排队超过 250 毫秒的键丢掉。Server 的 `AuxListener` 核对发送方进程号等于 client 的高 32 位，然后：

- 按键音或打字特效开着（`TypingFeedbackPreference.h` 的 `typing_feedback_wanted`，随偏好发布更新）时，回 "OK"，并通过 `SessionController::passthrough_key` 把任务排进输入队列，不等它执行。
- 都关着时不排，也不回 "OK"，TIP 停发 10 秒。

输入线程上 `FocusedSession::passthrough_key` 只认此刻以这个焦点令牌持有焦点、不在英文模式的会话，和 Server 接手的键走同一个 `sound_key`：全屏应用在前台时不出声，连击照数。

**动画开关和节电。** 「显示动画」（`SPI_GETCLIENTAREAANIMATION`）关掉时什么都不闪、不动、不弹，只留徽标，这是 Windows 卡片一直以来的规则。macOS 的「减弱动态效果」只去掉移动，闪光照常。开着节电模式时（`GetSystemPowerStatus` 的 `SystemStatusFlag`），火花和 Power Mode 退回闪光、卡片不抖，对应 macOS 的低电量模式。全屏应用在前台时浮层什么都不画。

**帧开销。** 浮层用自己的一把画刷，每笔改颜色和不透明度，不经 msimeui `DeviceResources` 只增不减的画刷缓存。火花向量一次预留 256 颗，发射和清理都不扩容。动画都停了、只剩静止的徽标时停掉帧计时器，换成一次性的收起计时器。候选卡片闪光的不透明度量化成 1/32 一档（`typing_effect_quantized_alpha`），以前每帧一个新透明度、缓存就多一把画刷，现在一次闪光最多几十把。共享设置页的 `typingEffectsFlashOnly` 只剩 HarmonyOS。

## Alternatives considered

- **用 DirectComposition 画浮层（`WS_EX_NOREDIRECTIONBITMAP` 加 `DeviceResources::EnsureForComposition`）**：候选窗、中英文提示、双拼键位图都这样画，能复用 msimeui，GPU 合成也最省。没有采用，原因有两个。点击穿透在 Windows 上要靠分层窗口的 `WS_EX_TRANSPARENT`，`WM_NCHITTEST` 回 `HTTRANSPARENT` 只对同一线程的窗口有效；浮层就盖在光标上，打字后一秒内挡住点击是用户直接碰得到的问题。另外 DirectComposition 在分层窗口上的行为没有把握。DIB 加 `UpdateLayeredWindow` 是 Windows 上分层窗口的标准做法，远程桌面和 Wine 也能用。浮层最大不过几百像素见方，每帧拷一次的代价可以接受。
- **把抖动做成挪动候选窗**：最像审计里写的 `SetWindowPos` 抖动。但抖动期间 `reposition()` 也可能挪窗口，两边会互相打架，留下错位；挪窗口还会让 Windows 重新算命中和置顶。用渲染变换在窗口里位移，抖完回到原位，什么状态都不留。
- **交给应用的键也走 Main 管道**：Main 管道有握手、有焦点租约，Server 拿到的会话最可靠。可它是同步的请求和回复，TIP 的按键线程要等 Server 答复，正是按键路径要避免的延迟。Aux 管道一条消息一个连接，在线程池上发，按键路径上只有一次入队。焦点令牌和进程号核对补上了认证。
- **Server 不回 "OK"、TIP 一直发**：最简单。但按键音和特效默认都关着，那样每个用户、每个交给应用的键都要连一次管道。由 Server 按偏好回答，功能关着时 TIP 停发 10 秒，开关打开后最晚 10 秒开始出声。

## Consequences

- **收益**：Windows 的火花、Power Mode、多色特效包、徽标弹跳和候选窗不在时的特效都和 macOS 一致；设置页的样式说明不再为 Windows 改写。没有组字时的空格、回车、退格、数字也出按键音、计入连击。数值集中在一个纯头文件里，主机上可以单测。
- **代价与已知上限**：光标位置先取前台线程的系统光标（`InputModeHudWindow::system_caret`，只认每显示器 DPI 感知的窗口）。取不到时用最近一次组字的锚点往上估 20 个设备无关像素一行。不建系统光标的应用里，交给应用的键的火花落在上次组字的位置，不在真正的光标处。macOS 的火花是加法混合的软圆点贴图，这里用两层不同不透明度的圆近似。全屏应用在前台时卡片闪光仍照 Windows 原来的行为画，没有像 macOS 那样一并关掉。交给应用的键在 Server 卡住时最多晚到 250 毫秒，再晚就丢。以下几点只在 macOS 上做过 MinGW 语法检查和纯策略的本机测试，要在 Windows 真机上确认：浮层的点击穿透、层级、帧率和观感，交给应用的键的出声延迟。若真机上分层窗口每帧上传的开销明显，或者火花被候选窗以外的置顶窗口挡住，就要重访第一条备选。

## Verification

- `platforms/windows/tests/candidate/typing_effect_overlay_policy.cpp`（`windows-typing-effect-overlay-policy`）：颗数、颜色轮换、轨迹范围、闪光、抖动、弹跳、摆放、调色板打包和上屏标记。
- `platforms/windows/tests/input/key_sound_policy.cpp`：Server 侧的键表改用共享表之后行为不变。
- `platforms/windows/tests/input/typing_feedback_preference.cpp`：什么偏好下回 "OK"。
- `platforms/windows/tsf/tests/passthrough_key_sound.cpp`（`msime-tsf-passthrough-key-sound`）：TIP 侧哪些键出声。
- `platforms/windows/tests/runtime/aux_message.cpp`：`KeySound` 消息的往返和拒绝。
- `apps/desktop/tests/settings/plugins-section.test.tsx`：Windows 的样式说明提到火花，`typingEffectsFlashOnly` 只对 HarmonyOS 为真。
