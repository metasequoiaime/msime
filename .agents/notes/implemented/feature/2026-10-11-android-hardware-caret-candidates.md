# Agent Note: Android 物理键盘时候选栏跟随光标

Status: implemented

## Problem

#6792 第 (1) 项：平板接实体键盘打字时，候选栏贴在屏幕最底部，离正在输入的地方很远；报告人想要桌面端那样的效果，候选出现在输入光标旁边。维护者把这个 issue 拆开，这次只做这一项，并定为可选开关、默认关。

现状是有意为之：[外接键盘的候选条模式](2026-10-08-android-hardware-keyboard-candidate-bar.md) 把键区收起，只留一行停在底部的候选条；[浮动键盘](2026-10-08-android-floating-keyboard.md) 在这个模式里不生效（`FloatingKeyboardPolicy.active`）。宿主从来没有向编辑器要过光标位置，全仓库没有 `requestCursorUpdates`、`onUpdateCursorAnchorInfo` 或 `CursorAnchorInfo`。

## Decision

- 开关是本地设置 `platform.android.hardware_caret_candidates`（`AndroidLocalSettings.HARDWARE_CARET_CANDIDATES`，布尔，默认关，不同步），入口是设置「键盘」页「布局」里的「物理键盘时候选栏跟随光标」。它和共享偏好 `candidate_follow_cursor` 是两回事：后者是桌面候选窗在组字途中跟不跟着光标挪（默认开，Android 不提供），前者决定候选条模式里候选栏放不放到光标旁。
- 判断和几何集中在无 Android 依赖的 `keyboard/CaretCandidatePolicy`：生效条件（开关、键区收起、编辑器答应报告位置、这一次没有退回底部），退回底部与回到光标旁的规则，候选栏显不显示，以及水平与竖直位置。
- 光标位置：开关打开、走引擎的输入框、配置报告接着实体键盘或已在候选条模式时，每次 `render` 开头的 `synchronizeCursorUpdates` 对当前 InputConnection 调一次 `requestCursorUpdates(CURSOR_UPDATE_MONITOR | CURSOR_UPDATE_IMMEDIATE)`，不再需要时以 0 撤回；换输入框和离开输入框时清掉。`onUpdateCursorAnchorInfo` 有组字时左边取组字第一个字（候选栏与组字起点对齐，打字时不往右滑），上下取整段组字和插入点所占的行，否则取插入点，经 `getMatrix()` 换成屏幕坐标。组字折到下一行时（笔画字形、超过一行宽的拼音），只按第一个字的外框摆会把候选栏放在第一行下方、正好压在折下去的组字和插入点上；Windows 的 TSF 宿主也是按整段组字的范围（`GetTextExt` 整个组字区间）摆候选窗。旋转等配置变化时再要一次 `IMMEDIATE`。
- 等第一次报告：配置不报告键盘的设备上，实体键盘第一次按下才进入候选条模式、才去要位置，而这一下同时开始组字。要过位置后的 `ANCHOR_WAIT_MS`（300 ms）内不退回底部，候选栏先不显示，位置一到就出现在光标旁。
- 退回底部：编辑器拒绝报告（`requestCursorUpdates` 返回假）时这个输入框整个照旧是停在底部的候选条。答应了却在要显示候选栏时等不来位置，这一次退回底部，直到空闲时收到位置才回到光标旁（`dockedAfterAnchor`），不在组字途中把候选栏和应用的排版一起挪走。
- 窗口复用浮动键盘那一套：`overlayDrawn()` 为真（浮动键盘或跟随光标）时输入法窗口与 `inputArea` 铺满、根视图透明、键盘列不留系统栏内边距、`onComputeInsets` 不让应用让出高度，可触摸区域只有键盘外框。外框与浮动面板同宽（`FloatingKeyboardPolicy.widthDp`）、圆角，没有拖动条；位置由 `positionCaretCandidates` 用平移决定。候选栏不显示时外框是 `INVISIBLE`（仍有大小，位置可以先算好），可触摸区域为空，触摸全部落到应用上（等第一次报告期间、窗口刚被叫出来还没收起的那一刻）。
- 位置：左边与光标对齐，放在光标所在行下方 4 dp；下方放不下而上方放得下时翻到光标上方；两边都放不下时放在空间大的一侧；始终夹在系统栏之间。外框和根视图每次重新布局都重新摆，所以候选栏换内容变高、旋转、分屏后仍在窗口里。
- 空闲时什么也不画，这时没有工具栏上的展开键。打开展开候选、内联高度条等要占键区的面板时 `hardwareKeysCollapsed()` 为假，键盘照旧停靠在底部展开。
- 空闲时收起整个输入法窗口（`CaretCandidatePolicy.hidesWindow`，`synchronizeCaretWindow` 里 `requestHideSelf`）。窗口只是什么也不画、却还开着的话，系统仍当键盘在显示：`InputMethodService.handleBack` 拿第一下返回键去收起这个看不见的窗口（API 33 起是输入法登记在应用那边的 `OnBackInvokedCallback`，应用拿不到这一下），应用要按第二下才返回；导航栏也一直是收起键盘的箭头和切换输入法键，应用读到的 `WindowInsets` 里键盘也算显示着。候选栏又有内容时（实体键盘开始组字、联想、英文建议）用 `requestShowSelf` 叫回来，只叫回自己收起的窗口（`caretWindowHidden`），用户按返回键收起的照旧由 `noteHardwareTyping` 在下一次组字时叫出来。
- 自己的收起不是用户收起键盘：收起要等应用那边的键盘动画，落地（`onFinishInputView(false)`）比请求晚约半秒，其间用户可能已经又开始组字；应用同时要求显示时系统还会把一次收起分几次落地，窗口也可能先被 `onWindowShown` 叫回来。所以请求后 `HIDE_LANDING_MS`（2 s）之内落地的收起都认作自己的（`caretHidePending` 加时刻，离开输入框时清），不做 `finishInputViewPresentation`，也不调系统默认实现（它会 `finishComposingText`），否则这段组字会被上屏、引擎里的组字也丢掉；落地后按此刻的状态决定要不要叫回窗口。应用一见键盘收起就要求再显示时，空闲收起至少隔 `WINDOW_REHIDE_MS`（500 ms），不和它每帧来回打转。

## Alternatives considered

- **默认打开** — 报告人想要的就是这个效果，桌面端也一直如此；但它改变了已有实体键盘用户的界面，空闲时还拿掉了工具栏和展开键，有的应用不报告位置时两种样子来回换。维护者的决定是可选、默认关。
- **另开一个 `PopupWindow` 画候选，输入法窗口仍停靠在底部显示工具栏** — Trime 用过这种做法，空闲时工具栏和展开键都还在，应用的排版也从不变化；但候选行、读音行、翻页和释义都长在输入视图的同一列里，要么把它们在两个窗口之间挪来挪去，要么另画一份，底部那一条工具栏还要一直占着屏幕，和「不要贴在底部」的诉求相反。复用浮动键盘已经验证过的铺满窗口做法，改动集中在窗口布局和摆放上。
- **空闲时把工具栏停在底部、组字时才挪到光标旁** — 保留展开键；但每开始和结束一次组字，输入法窗口就在「占底部一条」和「不占高度」之间切换一次，应用跟着重新排版，光标位置随之变化，候选栏追着跳。
- **一拿不到位置就固定退回底部、不等第一次报告** — 简单；但配置不报告键盘的设备上，第一次实体按键总是拿不到位置，每个输入框的第一段组字都会贴在底部，再在空闲时跳回光标旁。
- **窗口照旧开着，返回键在 `onKeyDown` / `onKeyUp` 里空闲时返回假，交给应用** — 不必每段组字都收起、叫出一次窗口，也就没有收起落地与组字赛跑的问题；但 API 33 起应用打开了预测返回（`targetSdk` 36 默认打开）时，返回走的是输入法登记在应用那边的回调，`compatHandleBack` 调 `onKeyDown` 时不看返回值，返回假只会让这一下彻底消失，比原来还糟；导航栏的收起箭头和应用读到的键盘可见状态也都不变。
- **组字时跟着插入点而不是组字起点** — 拿到的就是当前光标；但候选栏会随每个字母往右滑，桌面端（Windows 候选窗）也是按组字起点摆的。
- **设备套件在编辑器获得焦点后固定休眠 4 秒再注键** — 能避开当前最后一次 3 秒的显示请求，改动只在一处；但编辑器可能因窗口焦点变化重新排定请求，固定休眠无法证明最后一轮已发出，也不能证明空闲输入法窗口已收起。

## Consequences

- 收益：开关打开后，实体键盘打字时候选就在光标旁，屏幕底部不再被候选条占着；不报告位置的编辑器照旧可用。
- 代价：空闲时没有工具栏，回到软键盘要关掉开关或拔掉键盘；组字时候选栏盖住光标下方（或上方）一块应用内容；第一次报告最多等 300 ms，这段时间组字已经在进行但看不到候选。每段组字开始和结束都要叫出、收起一次输入法窗口（应用那边各有一次零高度的键盘动画），叫出要等一次跨进程往返，候选栏比窗口一直开着时晚一点出现。空闲收起之后 2 s 内用户在组字途中按返回键收起键盘的话，这一下会被认作自己的收起，组字不上屏，窗口还会被叫回来。窗口铺满依赖系统 `input_method.xml` 的结构，与浮动键盘相同。
- 平台对等：HarmonyOS 的 2in1 本来就是跟随光标的候选窗（`cursorContextChange` + `placeCandidateWindow`）；手机和平板接实体键盘时宿主照常路由按键，但还没有「收起键区只留候选条」这个模式，也就没有可以挂这个开关的地方，等那边先有候选条模式再谈对等。iOS 的键盘扩展收不到实体按键，接外接键盘时打字走的是系统输入法和系统的候选界面，这个开关在 iOS 上无从谈起。
- 验证：`tests/keyboard/CaretCandidatePolicySmoke.java` 钉住生效、退回、显示和位置规则，`AndroidLocalSettingsSmoke` 钉住设置项默认关、不同步，`SettingsSearchIndexSmoke` 钉住搜索得到这一行，均由 `platforms/android/check-host.sh` 运行。设备套件 `tests/device/keyboard/CaretCandidatesDeviceSmoke.java`（`smoke.sh` 跑）在 API 31 起用系统 `uinput` 接上一块 USB 键盘，核对组字时候选在输入框旁、空闲时不画、点击落到应用上、不报告位置的输入框（`EditorActivity` 的 `msime-test-no-cursor-anchor`）退回底部、窄的多行输入框（`msime-test-wrapping`）里折行的组字下方放候选栏，以及空闲时输入法窗口从窗口列表里消失、按一下返回键合成编辑器就离开。
- 已验证（2026-10-11，本机模拟器）：API 35 上 `smoke.sh` 全部套件通过，含新套件；另用宿主侧 `uinput` 键盘手动看过组字时候选在组字起点下方、空闲时输入法窗口可触摸区域为空（`dumpsys input` 里是 `<empty>`）、自由窗口（freeform）里候选栏被夹回屏幕内。API 28（`hw.keyboard=yes` 的 AVD，`adb emu event text` 打字）上手动看过同样的位置、不报告位置的输入框退回底部、横屏、以及把 `wm density` 调大后靠近底边时翻到光标上方；`smoke.sh --core` 通过。设置页的新开关在 API 28 和 35 上都点过。以上设备运行都带着 `fix/issue-6781` 的 8a2f502526：develop 上 #6832 引入的 `ImeNineKeyPanel.dismiss` 空指针会让键盘视图建好前的 `onStartInput` 崩溃，不带它时两台模拟器上的冒烟都过不去，与这次改动无关。
- 返回键与折行（2026-10-11，本机 API 35 模拟器，同样带着 8a2f502526）：`smoke.sh` 全部套件通过；`CaretCandidatesDeviceSmoke` 连跑 12 次全过（第一版只在 `onWindowShown` 之前认自己的收起，连跑时约三成失败在收起落地把组字上屏，加上时限后才稳定）。把 `hidesWindow` 改成恒假、去掉窗口检查再跑，套件失败在 `one back press leaves the editor`，即改动前按一下返回键应用不返回；把合成编辑器整个应用打开 `enableOnBackInvokedCallback` 再跑也通过，logcat 里能看到输入法的 `ImeOnBackInvokedCallback` 随窗口收起被撤下。API 28 上 `smoke.sh --core` 通过（新套件要 API 31 的 `uinput`，那里不跑）。
- 设备套件启动就绪（2026-10-12）：`EditorActivity` 在输入框获得焦点后按 250、750、1500、3000 毫秒连续要求显示输入法；原套件一见焦点就注入 `nihao`，与这些请求交错。CI 曾在 `caret strip while composing` 读到空的输入法可访问节点，同一提交重跑通过；此前的设备记录也提到机器负载高时丢首键。合成编辑器把最后一轮显示请求已发出的状态放在输入框的 `stateDescription`，套件等待该状态后才注键；首键才使宿主进入候选条模式，首键前允许完整软键盘窗口显示。只调整测试夹具与验收时序，不改变宿主行为。
- 普通触摸入口同样遵循就绪契约：节点 `extras` 将当前显示请求代次是否结束公开给 API 28 以上的套件，`DeviceSmoke.tap` 在合成输入框实际获得焦点、最后一轮请求结束后才返回；API 30 以上原有的 `stateDescription` 保留。触摸与焦点缓存的 CI 证据见[设备 CI 回归修复](../../implemented/bug-fix/2026-10-22-android-device-ci-regressions.md)。
- 未验证：真平板接蓝牙键盘、各家定制系统；Chrome、WebView、Compose 等第三方编辑器是否报告 `CursorAnchorInfo`、报告得准不准；分屏（只试了 freeform）。
