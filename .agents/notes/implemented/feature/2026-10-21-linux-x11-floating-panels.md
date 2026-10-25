# Agent Note: Linux 在 X11 上画悬浮工具栏和双拼键位提示

Status: implemented

## Problem

macOS 有两块 Linux 没有的屏幕浮窗：

- 悬浮工具栏（`platforms/macos/src/core/FloatingToolbarPanel.mm`）：一条可拖动、不抢焦点的工具条，左端是标志（或握把），右边一排按钮直接显示当前状态（中/双/五……英/En/A、。/.、半/全、简/繁），点一下切换或打开面板，停住出提示，10 秒没输入自动隐藏，按键唤醒，位置记住。Linux 只有输入法菜单里的「工具栏」子菜单，当前状态只是菜单里的勾，屏幕上看不到；GNOME Shell 下 IBus 的输入源菜单连这个子菜单都没有。
- 双拼键位提示（`platforms/macos/src/settings/ShuangpinKeymapPanel.mm`）：双拼组字时在候选窗旁画当前方案的三排键帽，高亮刚按下的键。Linux 两个前端都没有，`host_surface.rs` 对 Linux 关着 `shuangpin_keymap_hint`，设置页连开关都不给。

## Decision

本篇关于“有 `DISPLAY` 即可使用”的 Xwayland 前提由 [satellite 浮层回退](../bug-fix/2026-10-25-linux-satellite-overlay-fallback.md) 收窄：xwayland-satellite 不具备独立 root overlay 语义。原窗口、绘制和菜单架构保留；不得再把下面的兼容性推断当作 satellite 桌面验收。

两块浮窗都在 X11 上自绘，IBus 宿主和 Fcitx5 插件共用 `platforms/linux/src/overlay/` 里的同一份实现，行为、几何、文案照 macOS，视觉保留 Linux 的配色（产品决定：系统主题在 Linux 上用 Adwaita 的配色，皮肤画自己的颜色）。

### 分层

- 纯计算（单测直接包含）：`ShuangpinKeymapModel.h`（何时显示、三排键帽、零声母、几何、摆放与翻转、候选窗高度估计）、`FloatingToolbarModel.h`（读偏好、按钮与字、提示文案、两种菜单、几何、命中、摆放、位置文件）、`FloatingPanelColors.h`（从候选调色板推出两块浮窗的颜色）。
- 画法：`ShuangpinKeymapPainter.h`、`FloatingToolbarPainter.h`（工具栏、菜单、提示，图标是 cairo 路径，对应 SF Symbols）、`OverlayPaint.h`（与切换徽章共用圆角和阴影画法）。
- 窗口：`OverlayX11Window` 是一块 override-redirect、带 32 位 visual 的窗口，每次画时按合成器是否在场决定透明还是按 alpha 裁外形（与候选窗原生画布同一种退路）。`ShuangpinKeymapX11Surface` 点击穿透；`FloatingToolbarX11Surface` 接受指针、从不接受键盘焦点，菜单打开时抓住指针，点到外面就收起。
- 控制：`FloatingToolbar` 管属主、显示规则、空闲隐藏、全屏、位置记忆，显示后端经 `FloatingToolbarSurface` 注入（单测用假的）。事件循环经 `OverlayLoop` 注入：IBus 用 GLib 主循环，Fcitx5 用 `fcitx::EventLoop`。

### 悬浮工具栏

- 一个宿主进程一条，跟着当前属主走：IBus 的属主是引擎对象（每个输入上下文一个），Fcitx5 的属主是输入上下文。获得焦点或切换到本输入法时接手（IBus 自己的 `fake` 上下文不接手），失焦不收起，换输入法（IBus 的 `disable`、Fcitx5 的 `deactivate` 非失焦事件）或属主销毁时收起并放开。
- 显示条件的形状是 macOS 的 `MetasequoiaFloatingToolbarShouldShow`：开关开着、有属主、前台窗口不是全屏（`_NET_ACTIVE_WINDOW` 的 `_NET_WM_STATE_FULLSCREEN`，根窗口的 `_NET_ACTIVE_WINDOW` 或前台窗口自己的 `_NET_WM_STATE` 一变就重查）。全屏这一条是有意与 macOS 不同、跟 Windows 走：macOS 现在总是传「不是全屏」，面板带 FullScreenAuxiliary 盖在全屏应用上；X11 的 override-redirect 窗口会盖住全屏的视频和游戏，所以 Linux 收起。`_NET_ACTIVE_WINDOW` 一变还把还在屏幕上的工具栏 `XRaiseWindow` 一次：有的窗口管理器用 XRaiseWindow 抬起被点中的窗口框，会把它压到 override-redirect 窗口之上。10 秒没有按键也没有点它就隐藏，下一次按键或点击回来；同一个属主换输入框不唤醒，新的激活或换属主才唤醒，与 `activateForDelegate:visible:` 相同。
- 几何照搬 macOS 的常量（按钮字号加 8、间距 2、标志 34 或握把 10 加 4、分隔线 1.2、高字号加 20、圆角 10），按 `scale_percent` 和 `font_size` 缩放，再乘桌面缩放；默认一行 255 × 44，与 `kToolbarWidth`、`kToolbarHeight` 相同。
- 按钮动作走「工具栏」子菜单里同一项的路径：IBus 调 `property_activate` 的对应属性，Fcitx5 激活状态区里的同一个 `fcitx::Action`。中/英按钮在「En」（中文输入开着、英文候选模式）时退出英文候选模式回到中文，其余时候切换中文输入，与 macOS 的 `floatingToolbarDidRequestToggleInputMode:` 相同（`floating_toolbar_input_mode_leaves_english_candidates`）。打开面板、官网和「隐藏悬浮状态栏」不需要输入框，IBus 直接做。IBus 的属性路径只认获得焦点的引擎，所以属主失焦后要输入框的按钮和「宣传」画成淡色、点了不做事（`Options::actions_need_focus`），回到输入框恢复；Fcitx5 把失焦的上下文留给状态区菜单，按钮照样生效。设置按钮右键是 macOS 齿轮菜单的内容，去掉 Linux 没有的「检查更新」，「表情与符号」换成水杉表情面板。
- 位置记在 `$XDG_STATE_HOME/<客户端目录>/floating-toolbar-position.json`，原子替换写入，与 Windows 的 `floating_toolbar_position.json` 一样只属于这台机器，不进同步的共享偏好；恢复时收进工作区四边 12 以内，没记过时放在焦点窗口所在显示器工作区右下角、离边 20。
- 全屏监听跟随当前前台窗口：读取 `_NET_ACTIVE_WINDOW` 后，撤销旧窗口的属性订阅，在新窗口订阅 `PropertyChangeMask`，再读取 `_NET_WM_STATE`。先订阅后读取，避免漏掉中间的全屏切换；旧窗口或新窗口可能已经销毁，订阅、撤销和查询共同置于现有的 Xlib 错误保护内。没有前台窗口时撤销订阅，根窗口不作为应用窗口订阅；断线重连时清空记住的窗口编号。状态通知复用控制器现有的显示判断，不重置空闲计时。后端以 `connected()` 报告连接是否有效，断线时控制器丢弃旧全屏判断，避免隐藏中的工具栏无法进入重连路径；成功重连后先在一次零延迟回调里重查全屏、恢复订阅，再决定显示，在这之前不绘制。回调异步执行，避免 `create()` 尚未交还后端时访问控制器，以及在 `present()` 内同步递归；关闭连接时撤销这次回调。Xlib 的订阅按连接独立，不改应用自身的事件掩码，见 [XSelectInput](https://www.x.org/releases/current/doc/man/man3/XSelectInput.3.xhtml)；窗口管理器维护的全屏状态见 [EWMH `_NET_WM_STATE`](https://specifications.freedesktop.org/wm/latest/ar01s05.html)。

- 后端只有 X11。Wayland 会话有 `DISPLAY`（Xwayland）时也用它：工具栏不跟随光标，只要一个屏幕位置，Xwayland 的根坐标在 GNOME、KDE、wlroots 上都对得上桌面布局。那里看不到 Wayland 原生窗口是否全屏，所以 `hide_on_focus_out`：失焦就收起，获得焦点再出现。没有 X 服务器或构建缺 x11/xext/xrandr/cairo-xlib/pangocairo 时没有窗口，「工具栏」子菜单照旧。
- Fcitx5 默认每个上下文各记各的输入法，焦点移到用别的输入法的上下文时插件只收到旧上下文的失焦；`panel_focus_watch_` 在任何上下文获得焦点时看它的输入法，不是本输入法就收起工具栏并放开属主，等同 macOS 的 `deactivateForInputSourceSwitch`。
- 工具栏和键位图的 X 连接经 `open_overlay_display` 打开，装上这条连接自己的 `XSetIOErrorExitHandler`（libX11 1.7+，CMake 按 pkg-config 的 x11 版本定义 `MSIME_X11_IO_ERROR_EXIT_HANDLER`），再加一个进程级的 I/O 错误处理，只对这些连接打印一行并返回、别的连接照旧交给之前的处理（Xlib 默认的 I/O 错误处理自己就 `exit(1)`，走不到每条连接的退出处理，Xvfb 测试里先这样失败过）：Xwayland 崩溃或重启时不再让整个 IBus 引擎或 fcitx5 守护进程退出，只记下断开，在下一轮主循环里关掉连接，下一次要显示时重新连。legacy 包的 libX11 1.6.7 没有这个接口，那里照旧。

### 双拼键位提示

- 条件与 macOS 的 `updateKeymapPanel` 相同：有焦点、中文输入开着、`shuangpin_keymap_hint` 打开、双拼方案、没有局部模式、不在英文候选模式、正在组字；自定义方案不画。键位表经 `msime_client_shuangpin_key_hints` / `msime_client_shuangpin_zero_initials` 读 Engine 自己的表，按方案缓存。
- 只画给报根窗口坐标光标的客户端：Fcitx5 看输入上下文的 display 是否 `x11:`，IBus 看会话里没有 `WAYLAND_DISPLAY`，与切换徽章的判据相同。光标矩形高为 0 时不画（`MSIMEValidCaret`）。
- 摆放照 `MSIMEShuangpinKeymapPanelFrame`：左边对齐光标，放在光标下方、让开候选窗再隔 8，下面放不下就翻到光标上方，收进工作区四边 16。Fcitx5 自己画候选窗（原生画布，[X11 上的默认候选窗](../architecture/2026-10-21-linux-x11-native-candidate-default.md)）时用画布里卡片的真实矩形（`NativeCanvasX11::frame`、`shuangpin_keymap_measured_clearance`）：画布窗口四周还有合成器下的投影边距、装饰图伸出的部分和外留白，按整个窗口算会让键位图离看得见的卡片远出一截，候选窗在光标下方而窗口顶边因投影或装饰带高过光标底边时还会误判成翻到了光标上方，所以 `frame` 按 `shape`、`place` 同一套平移报卡片（`layout.panel`）的矩形，退场动画中算已收起；候选窗由 IBus 面板或 Fcitx5 经典界面画时宿主拿不到它的矩形，候选窗高度按候选字号、横竖排、条数和拼音行估计，与 macOS 在算精确几何之前的基线公式相同。经典界面的字体是插件按同一个候选字号偏好写进去的，所以估计用的字号与它画的一致。
- 卡片 620 × 203、13 圆角；macOS 的标题行没有固定高度，被约束撑到 33，这里直接写成 33。高亮键与 macOS 的 `MSIMEKeymapAccent` 一样填主题的强调色（`accent` 槽位）、白字；系统主题解析出的 `accent` 是 Adwaita 写在卡片上的强调字色（深色下是浅蓝），这时改用 GNOME 的强调底色 #3584E4（产品决定：系统主题在 Linux 上用 Adwaita 的配色）。

### 能力位与设置页

`host_surface.rs` 对 Linux 打开 `shuangpin_keymap_hint` 和 `floating_toolbar_appearance`。键位图只画给 X11 客户端，所以 Linux 上「输入时显示双拼键位提示」的说明照实写出 Wayland 桌面上 IBus 不显示、Fcitx5 只为经 Xwayland 运行的应用显示。共享设置页的悬浮工具栏页对 Linux 的判据改成 `linuxPlatform && !showToolbarAppearance`：报告外观能力的 Linux 宿主（现在就是）得到与 macOS 相同的预览、开关、尺寸和颜色链接，预览下另说明 X11、Xwayland 和退回子菜单；只有菜单的 Linux 文案仍保留给不报这项能力的宿主。

## Alternatives considered

- **把 Windows 的 `ShuangpinKeymapLayout.h` 挪到 `platforms/common` 两边共用** — 规则只有一份，Windows 和 Linux 不会漂移。这次没有做：那份头文件带着 Windows 的调色板和阴影类型，挪过去要同时改 Windows 的构建和测试，而 Windows 的键位图正在另一条并行的改动里；Linux 这份写成同样照 macOS 的纯函数，测试钉住相同的数值，合并可以单独做。
- **Wayland 用 layer-shell 画工具栏（切换徽章和语音浮层已经这样做）** — KDE、wlroots 上不经 Xwayland，缩放更准。没有做：layer-shell 只能锚在输出边上，拖动要改成调边距，指针、配置往返和缓冲都要再写一套；GNOME 没有 layer-shell，经 Xwayland 的 X11 窗口在三种合成器上都能显示、能拖。将来要做时 `FloatingToolbarSurface` 已经是注入点。
- **Wayland 上也用 layer-shell 画键位图** — 原生客户端也有提示。没有做：layer-shell 拿不到光标的屏幕坐标，键位图只能放在屏幕边上，离正在输入的那一行很远，而 macOS 的键位图是贴着候选窗的；不画比画在错的地方好。
- **工具栏跟着每个引擎对象（IBus）或每个输入上下文各建一条** — 不需要属主交接。没有采用：理由与切换徽章的进程级表面相同，每个获得过焦点的输入框各占一条 X 连接，快速换输入框还会同时留着几条工具栏；macOS 也只有一个 `sharedPanel`。
- **按钮动作在浮窗里另写一份（直接改偏好、直接调 host-api）** — 不依赖菜单那条路径的焦点检查。没有采用：同一个开关两份实现迟早行为不同，「工具栏」子菜单已经走好了保存、会话同步和失败重试。
- **只监听根窗口的前台窗口切换** — 只需要一次根窗口订阅，不必管理应用窗口的生命周期。没有采用：F11 或视频播放器在同一窗口进出全屏时不必改变 `_NET_ACTIVE_WINDOW`，工具栏会继续盖在全屏内容上，或退出全屏后一直不恢复。直接监听状态属性，避免为这一项定时轮询 X 服务器。
- **全屏时照 macOS 留着工具栏** — 与 macOS 完全一致。没有采用：macOS 的全屏应用在自己的桌面空间里，工具栏是辅助面板；X11 的 override-redirect 窗口直接盖在全屏视频和游戏上，Windows 也在全屏时收起。
- **IBus 失焦后绕过焦点检查直接改状态** — 按钮在失焦后照 macOS 生效。没有采用：中文输入的切换会对会话调用 `focus_session(true)`，失焦的引擎改输入状态会让会话以为自己有焦点，其余几项要在失焦的引擎上重画；画成不能用是照实，回到输入框就恢复。
- **读皮肤包的 `[toolbar]` 段（ToolbarSkinTokens）** — 皮肤在工具栏上也画自己单独的配色和圆角。这次没有做：共享层发给宿主的皮肤目录（`host_candidate_catalog`）为控制文档大小不带工具栏段，要改那份跨宿主的契约；Linux 的工具栏先用候选配色和固定的 10 圆角，与 macOS 在带 `[toolbar]` 段的皮肤下颜色会有差别。
- **设置页按 `!showToolbarAppearance` 判断要不要显示菜单版文案（[2026-10-09](../bug-fix/2026-10-09-linux-toolbar-settings-menu-copy.md) 否掉的那个）** — 现在用的是 `linuxPlatform && !showToolbarAppearance`，两个条件一起：菜单版文案仍只给 Linux，那篇担心的「别的宿主套上 Linux 文案」不会发生。
- **在 Xvfb 断言前固定睡一段时间** — 只需增加一行，能缓解多数机器上的时序差异。没有采用：无法知道另一条连接何时完成请求，负载变化仍会误报，并让正常路径每次都额外等待；按状态轮询同时保留失败上限和原有几何、可见性断言。

## Consequences

- **收益**：
  - X11（以及 GNOME、KDE、wlroots 的 Xwayland）上有了与 macOS 同样大小、同样按钮和状态字、同样空闲隐藏和位置记忆的悬浮工具栏；GNOME Shell 下 IBus 第一次有了工具栏入口。
  - X11 客户端的双拼用户有了键位提示，设置页给出开关。
  - 两块浮窗的规则、几何和颜色都是纯函数，各有单测；X11 窗口在 Xvfb 里真的建出来测位置、点击、提示、两种菜单、拖动和位置记忆；IBus 的接线在 Xvfb 里测获得焦点显示、失焦保留、换输入法收起。
- **代价与已知上限**：
  - 工具栏默认开着（共享默认值），所以升级后 X11 用户第一次会在屏幕右下角看到它；不想要可以在菜单里取消「显示工具栏」或点「隐藏悬浮状态栏」。GNOME Shell 下 IBus 的输入源菜单没有「工具栏」子菜单，也没有「显示工具栏」开关：那里只能在设置的「悬浮工具栏」页关掉，或用设置按钮右键菜单里的「隐藏悬浮状态栏」；重新打开只能回设置页，设置页的说明照这个写。
  - Wayland 原生客户端没有键位图；Wayland 会话里的工具栏经 Xwayland，失焦就收起，KDE 分数缩放下 Xwayland 窗口可能发虚。没有 Xwayland 的 Wayland 会话只有「工具栏」子菜单。
  - IBus 面板和 Fcitx5 经典界面画的候选窗，键位图让开它靠估计，主题加了大装饰或者面板字体被另行改大时可能被盖住一点；Fcitx5 原生画布用卡片的真实矩形。
  - 菜单和提示不响应键盘（菜单抓指针，不抢键盘焦点），Escape 不会收起菜单；点到外面或换窗口才收起。
  - IBus 上属主失焦后切换类按钮画成不能用，要回到输入框才能用；macOS 上它们失焦后仍然生效。打开面板、官网、设置和隐藏照常。
  - 带 `[toolbar]` 段的皮肤（工厂皮肤里有 wechat、fluent、microsoft、graphite、canvas、autumn_osmanthus、willow_green、wechat_scroll）在 Linux 上工具栏用候选配色，与 macOS 不同。
  - 工具栏在第一次有输入框获得焦点时连 `DISPLAY` 并一直连着，按需启动 Xwayland 的合成器会因此启动 Xwayland。切换徽章、语音浮层和原生画布各自的 Xlib 连接没有装断开处理，仍是 Xlib 的默认行为。
  - 不维护 `_NET_ACTIVE_WINDOW` 的窗口管理器上没有重新抬起工具栏的时机。
  - 键位图的规则与 Windows 的 `ShuangpinKeymapLayout.h` 是两份，改其中一份时要对照另一份，直到合并。

## Verification

全屏回归保持 `_NET_ACTIVE_WINDOW` 不变，只设置或删除当前窗口的 `_NET_WM_STATE_FULLSCREEN`，通过事件循环和实际窗口可见状态等待，验证进入时隐藏、退出时恢复。旧实现先在此处因「同一前台窗口进入全屏后工具栏仍然显示」失败，修改监听后通过；窗口销毁后原测试继续清空前台窗口，覆盖撤销已销毁窗口的订阅。断线测试保留同一前台窗口：在断线期间进入全屏，恢复连接后继续隐藏，退出全屏后恢复；还覆盖隐藏于全屏期间断线、断线期间退出全屏后用下一次输入恢复。只有属性订阅的中间实现先因「重连后工具栏盖在全屏窗口上」失败，补上重连后的状态刷新再通过。

键位图 Xvfb 测试通过独立连接观察窗口，按目标几何或不可见状态轮询，达到条件立即继续，2 秒仍不满足才失败。浮窗自己的 `XFlush` 只保证请求发出，观察连接上的 `XSync` 不能确认另一条连接的请求已被服务端执行；立即断言隐藏会在同一构建上交替通过和失败。测试等待实际状态，生产浮窗维持异步绘制与隐藏。

- `bash platforms/linux/build-container.sh`：`linux-floating-panels-model`（规则、几何、颜色和控制逻辑，假的事件循环和后端；含「En」回到中文、要输入框的按钮、失焦停用、强调色和量出的候选窗高度）、`linux-floating-panels-render`（cairo、pango 实画键位图、工具栏、菜单、提示并检查像素，含停用的按钮和菜单项）、`linux-floating-panels-x11`（Xvfb：另测前台窗口换了以后工具栏回到最上面、连接断开后进程不退出并重新连上、失焦时停用的按钮点了不做事、键位图按量出的高度让开）和 `ibus-mode-hint-x11-toolbar`（Xvfb），`build-container.sh` 要求它们注册并执行。`MSIME_TEST_CANVAS_ARTIFACTS` 指向可写目录时，渲染测试和 Xvfb 测试把画面存成 PNG。
- `cargo test -p msime-client-core --lib host_surface` 钉住 Linux 的两个能力位；`apps/desktop` 的 `tests/input/host-capabilities.test.tsx` 钉住画悬浮工具栏的 Linux 页面。
- 没有在真实桌面（GNOME、KDE、Xfce、wlroots）上看过：多显示器、分数缩放、合成器阴影、真实全屏应用和托盘共存都只经过 Xvfb 和单测。
