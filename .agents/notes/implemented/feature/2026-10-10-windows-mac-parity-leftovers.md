# Agent Note: Windows 对齐 macOS 的遗留项：屏幕键盘快捷键兜底、按方案的任务栏图标、日文释义罗马字，置顶切换暂缓

Status: implemented

## Problem

第一轮 Windows 对齐 macOS 留下四件事。Ctrl+Shift+Win+K 只找共享应用，没装或起不来时什么也不做；工具栏和托盘的键盘入口这时已经退回 `osk.exe`（[工具栏](2026-10-10-windows-floating-toolbar-mac-parity.md)）。任务栏的语言栏图标只有中、英、日、한、大写五种，双拼、五笔、粤拼、注音、越南文、藏文、笔画都显示「中」；macOS 输入菜单里每个方案有自己的一个大字（`platforms/macos/src/input/InputModeIdentifiers.h`）。候选释义的日文行在 Windows 上不标罗马字，macOS 用系统分词器标（[候选窗](2026-10-10-windows-candidate-window-mac-parity.md) 当时放弃了 IFELanguage）。候选右键菜单的「置顶」在 Windows 上不能取消，macOS 有「取消置顶」。

## Decision

**快捷键和工具栏、托盘走同一条路。** `server_main.cpp` 的 `MaintenanceAction::OpenScreenKeyboard` 调已有的 `open_screen_keyboard`：先找共享应用，起不来时 `open_system_screen_keyboard()` 打开 `osk.exe`。三个入口的行为因此一致。

**任务栏图标按方案画一个字。** 语言栏的模式按钮由 `tsf/LanguageBar/ModeIconPolicy.h` 的 `mode_icon(open, disabled, capsLock, mode)` 选图标，顺序是：按钮禁用时画「英」，Caps Lock 开着时画大写图标，键盘关着时画「英」，其余按模式画中、双、五、日、한、粤、注、越、藏、笔。藏文画「藏」，与 Windows 托盘和悬浮工具栏一致，不用 macOS 的「ཀ」。全角、标点两个按钮仍按开关两张图选。

TIP 原本分不出双拼和五笔：`InputModeChanged` 帧给全拼、双拼、五笔发同一个码 `'0'`。`common/InputSchemeTraits.h` 给它们各加一个码，`InputMode::Shuangpin` 是 `'8'`，`InputMode::Wubi` 是 `'9'`，Server 用 `input_mode(scheme)` 照常算出并发送。`mode_scheme` 把这两个码仍映射成 `Quanpin`，所以 TIP 的键入不变（`Global::InputModeScheme` 照旧）；码只存进新的 `Global::InputModeIndicator`，只有 `CLangBarItemButton::GetIcon` 读它。TIP 激活时先按偏好里正在运行的方案填一次，Server 的帧到了再覆盖。悬浮工具栏的 `toolbar_icon` 也认这两个码，直接画「双」「五」。

十四个 ICO（`tsf/assets/{shuangpin,wubi,cantonese,zhuyin,vietnamese,tibetan,stroke}-{light,dark}.ico`）由 `platforms/windows/scripts/render_tsf_mode_icons.swift` 生成，在 macOS 上运行，同样的输入得到逐字节相同的输出。字形取自思源黑体 SC Regular 2.005（SIL Open Font License 1.1，adobe-fonts/source-han-sans 的 `2.005R` 发布里的 `09_SourceHanSansSC.zip`，脚本核对 OTF 的 SHA-256），按墨迹框把长边撑到图块的 15/16，摆法与 `render_menu_icon.swift` 一样。最初用的是 macOS 菜单图标同款的 PingFang SC，审查时改掉了：PingFang 是 Apple 随系统授权的字体，它的字形渲染成图标放进 Windows 安装包分发，授权上站不住；OFL 字体渲染出的图像可以随任何产品分发。字重用 Regular：任务栏上并排的中、英、日是上游原图，笔画细，同一个指示器换方案时字不该忽粗忽细。尺寸与 jp、kr 相同，16 到 48 每隔 4 再加 64；每个尺寸是 32 位 BMP 条目加 AND 掩码。浅色任务栏用黑字，深色用白字。资源号 37 到 50（`tsf/Header/resource.h`），导出检查 `tsf/tests/exports/verify.cmake` 要求的图标组从 15 个变成 29 个，`tsf/assets/README.md` 记来源。

**日文释义行的罗马字来自微软日语输入法。** `src/candidate/JapaneseReader.cpp` 在翻译工作线程上用 IFELanguage（ProgID `MSIME.Japan`）的 `GetJMorphResult(FELANG_REQ_REV, …)` 分词，并取每个词的平假名读音。反查时转换结果 `pwchOutput`（`wDispPos`/`cchDisp`）是平假名读音，原文在 `pwchComp`（与 `pwchRead` 同一个联合，`wCompPos`/`cchComp` 同理），所以写法取原文、读音取输出（`japanese_reverse_words`）；最初的实现把两者取反了，含汉字的行一律读不出来，只有纯假名的行靠退路读出，因为这条路在 Windows 上没有跑过，评审才发现。`src/candidate/JapaneseRomaji.h` 把读音按平文式转成罗马字，词与词之间一个空格。助词 は、へ、を 和 こんにちは、こんばんは 按发音读。有一个词读不出来，整行就不标，和 macOS 的 `MSIMEJapaneseRomaji` 一样。

接口声明照抄 Windows SDK 的 `msime.h`，因为 MinGW 不带这个头文件。该头文件整个用 `#pragma pack(1)`，布局由 `static_assert` 钉住。COM 在工作线程上按单线程套间初始化，线程已经是多线程套间时沿用。`TranslationWorker::run` 在工作线程的栈上持有一个 `JapaneseReader`，经一个 `thread_local` 指针交给本线程的读音查询，所以创建、使用、关闭都在同一线程上；结果按词缓存，最多 1024 条。关闭由读音器在 `run` 返回时的栈析构完成，不用 `thread_local` 对象：那种析构在线程退出回调里持着加载器锁运行（MSVC 和 MinGW 都是），在那里 `CoUninitialize` 或让 COM 卸载日语输入法的 DLL，可能让 Server 退出时卡在等翻译线程上。最初的写法是 `thread_local JapaneseReader` 加 `run` 返回前显式 `close()`，门禁在 Wine 下跑 `windows-translation-worker` 时它在线程退出时跳到空地址崩溃（MinGW 构建的 `thread_local` 析构），改成栈上实例后通过。

系统里没有这个组件时不报错：纯假名的释义仍按假名直接读，含汉字的行不标读音。`CandidateGlossReadings.h` 的 `gloss_pronunciation_lines` 多一个可选的 `japanese` 读法；不传时行为和以前一样。共享设置页「显示读音」不再按宿主区分说明（`expression-page.tsx` 不再传 `candidatePronunciationRomaji={!windowsPlatform}`，`CandidatePronunciationSection` 的 `romaji` 属性和那句只讲音标的说明也一并删掉，没有宿主再缺罗马音），原生设置窗口的同一行也改成「英文释义给音标，日文释义给罗马音」。

**「取消置顶」这次不做，先定共享机制。** 不做的原因是证据指向 Engine，而改 Engine 要做产品决定。

macOS 的「取消置顶」只撤掉宿主自己按编码记的顺序表（NSUserDefaults `MSIMEClientPinnedCandidates`，`InputController.mm` 的 `MSIMETogglePinnedCandidate`）。Engine 的置顶是一次性抬高词频，加上 `pinned_candidates` 表里一条记录（`crates/engine/src/session/candidates.rs` 的 `pin_candidate`），两样都不会撤回。macOS 能在宿主里重排，是因为数字键按候选窗上画出来的位置取候选的 `generation`/`index` 再选（`InputController.mm` 的 `MSIMERenderedCandidateIdentity`）。Windows 的数字键则原样发给 Engine（`ReplyComposer::dispatch`），按 Engine 的顺序选。照搬宿主重排，画出来的第 1 位和按 1 选中的就不是同一个词，所以 Windows 不能照抄。

建议的共享机制如下：

1. Engine 的 `pinned_candidates` 改成每个上下文可以存多条、带先后顺序（现在每个 `context_key` 只有一条，`positions.rs` 的 `record_pinned_candidate` 是 upsert）。
2. 排序时把置顶词按顺序排在最前，与固定排位用同一个时机（`apply_fixed_positions_with_state`）。
3. 视图的每个候选带 `pinned`。
4. 加一个取消置顶的候选动作，经 host-api 和会话协议暴露。

这样所有宿主都不必自己重排，数字键和画面也自然一致。macOS 再把 NSUserDefaults 里的表一次性重放进 Engine。

要用户拍板的是取消置顶要不要连同词频的抬高一起撤回。macOS 现在不撤，取消后那个词通常仍排在前面。

## Alternatives considered

- **按方案注册多个 TSF 语言配置文件，让系统的输入法切换器直接选方案** — 这最接近 macOS「每个方案一个输入模式、从系统菜单选」的形状。但每个配置文件都要在安装时注册，还要处理配置文件之间的切换与方案偏好的同步，这比画图标大一个量级；系统切换器里一下多出十一项，也和 macOS 的按需启用（opt-in）不是一回事。所以这次只做指示，切换仍走托盘和工具栏。
- **另开一条帧把确切的方案号发给 TIP，不动 `InputModeChanged`** — 输入模式码保持原义，不碰一条已有的线上约定。但旧 Server 配新 DLL 时这条帧永远不来，图标需要另一套缺省；而新码 `'8'`、`'9'` 在旧 DLL 那边按约定读成中文，行为与以前完全相同，兼容成本更低。
- **日文只用 `IFELanguage::GetPhonetic` 取整串读音** — 这不用声明 `MORRSLT`，一个 BSTR 进、一个 BSTR 出，不担心结构布局。但它没有词边界：今日は天気がいいですね 会读成 kyouhatenkigaiidesune，助词 は 读成 ha，正好教错音。macOS 特意逐词读、按发音读助词，所以这里用带词描述的 `GetJMorphResult`，布局按 SDK 头文件钉住。
- **用系统自带 ICU（`icu.dll` 的 `ubrk_*`）分词，再用假名表转换** — Windows 10 1703 以后的系统都带 ICU，所以这条路不依赖日语输入法。但 ICU 只能分词，给不出汉字的读音，含汉字的词仍要 IFELanguage，等于多接一个系统库而不解决读音。
- **把 macOS 的宿主置顶表照搬到 Windows，在 Server 里重排候选** — 这只改 Windows，不动 Engine。但 Windows 的数字键按 Engine 顺序选词，重排后画面和选词会对不上；要对上就得在 `ReplyComposer` 里把数字键改成按画出的位置选，还要同步 TSF 候选列表 UI 元素和删除快捷键的位置，等于在 Windows 再造一个 macOS 的宿主层，而 Engine 级的方案对所有宿主都省事。

## Consequences

- **收益**：
  - 三个屏幕键盘入口在没有共享应用时都能打开系统屏幕键盘。
  - 任务栏指示器和 macOS 输入菜单一样，一眼能看出是哪个方案。
  - 装了日语输入法的 Windows 上，日文释义带罗马字，读法与 macOS 一致：逐词、助词按发音、读不全不标。
- **代价**：
  - `InputModeChanged` 多了两个码，以后读这个码的代码不能再假定「中文模式只有 `'0'`」；要按方案键入的地方继续经 `mode_scheme`，不要直接比较码。
  - 罗马字在 Windows 上依赖系统的日语输入法组件。没有它时含汉字的日文行不标，设置页的说明不区分这种情况。
  - 长音写成重复元音（koohii），不加长音符号；macOS 系统分词器的写法没有在本机核对过，两边在长音上可能不同。
  - IFELanguage 在 Server 进程里起 COM，首次使用有加载开销，所以只在打开「显示读音」并且出现日文释义行时才发生。
  - 重新生成十四个图标要在 macOS 上运行脚本（CoreText），并且要先下载那份固定版本的思源黑体 OTF 作为参数传入；换了版本的字体会被脚本拒绝。
  - 置顶切换仍缺，Windows 的「置顶」仍是单向的。

## Verification

主机上能跑的纯策略测试如下：

- `tsf/tests/input/mode_icon_policy.cpp`：每个模式的图标，以及 Caps Lock、英文、禁用的先后。
- `tests/input/input_scheme_traits.cpp`：新码的往返、`mode_scheme` 仍是全拼、旧码和未知码读成中文。
- `tests/runtime/tsf_config_frames.cpp`：帧里发 `'8'`、`'9'`。
- `tests/ui/toolbar_icons.cpp`：模式码直接画双、五。
- `tests/candidate/japanese_romaji.cpp`：平文式、拗音、促音、拨音、长音、外来音、逐词与助词、读不全不标，以及按反查结果的方向拆词（写法取原文、读音取输出）。Windows 真机上仍要用「今日は天気がいいですね」确认读出 kyou wa tenki ga ii desu ne。
- `tests/candidate/candidate_gloss_readings.cpp`：日文行接上罗马字读法。
- `apps/desktop/tests/settings/settings.test.tsx`：Windows 的「显示读音」说明。

ICO 由脚本重跑两次比对字节相同，并渲染成 PNG 与 cn、jp 原图并排检查过。

以下要在 Windows 真机上确认：

- `MSIME.Japan` 在没装日语输入法的系统上确实不可创建，并且失败路径不卡住翻译线程。
- `GetJMorphResult` 对释义短语的分词粒度，以及动词活用是否拆得过细（例如 食べ / ます）。
- 任务栏在 100% 到 200% 缩放下取到的图标尺寸。
- Ctrl+Shift+Win+K 在没有 MSIME.exe 时打开屏幕键盘。
