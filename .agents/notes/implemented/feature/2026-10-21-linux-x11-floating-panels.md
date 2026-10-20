# Agent Note: Linux 在 X11 上画悬浮工具栏和双拼键位提示

Status: implemented

## Problem

macOS 有两块 Linux 没有的屏幕浮窗：

- 悬浮工具栏（`platforms/macos/src/core/FloatingToolbarPanel.mm`）：一条可拖动、不抢焦点的工具条，左端是标志（或握把），右边一排按钮直接显示当前状态（中/双/五……英/En/A、。/.、半/全、简/繁），点一下切换或打开面板，停住出提示，10 秒没输入自动隐藏，按键唤醒，位置记住。Linux 只有输入法菜单里的「工具栏」子菜单，当前状态只是菜单里的勾，屏幕上看不到；GNOME Shell 下 IBus 的输入源菜单连这个子菜单都没有。
- 双拼键位提示（`platforms/macos/src/settings/ShuangpinKeymapPanel.mm`）：双拼组字时在候选窗旁画当前方案的三排键帽，高亮刚按下的键。Linux 两个前端都没有，`host_surface.rs` 对 Linux 关着 `shuangpin_keymap_hint`，设置页连开关都不给。

## Decision

两块浮窗都在 X11 上自绘，IBus 宿主和 Fcitx5 插件共用 `platforms/linux/src/overlay/` 里的同一份实现，行为、几何、文案照 macOS，视觉保留 Linux 的配色（产品决定：系统主题在 Linux 上用 Adwaita 的配色，皮肤画自己的颜色）。

### 分层

- 纯计算（单测直接包含）：`ShuangpinKeymapModel.h`（何时显示、三排键帽、零声母、几何、摆放与翻转、候选窗高度估计）、`FloatingToolbarModel.h`（读偏好、按钮与字、提示文案、两种菜单、几何、命中、摆放、位置文件）、`FloatingPanelColors.h`（从候选调色板推出两块浮窗的颜色）。
- 画法：`ShuangpinKeymapPainter.h`、`FloatingToolbarPainter.h`（工具栏、菜单、提示，图标是 cairo 路径，对应 SF Symbols）、`OverlayPaint.h`（与切换徽章共用圆角和阴影画法）。
- 窗口：`OverlayX11Window` 是一块 override-redirect、带 32 位 visual 的窗口，每次画时按合成器是否在场决定透明还是按 alpha 裁外形（与候选窗原生画布同一种退路）。`ShuangpinKeymapX11Surface` 点击穿透；`FloatingToolbarX11Surface` 接受指针、从不接受键盘焦点，菜单打开时抓住指针，点到外面就收起。
- 控制：`FloatingToolbar` 管属主、显示规则、空闲隐藏、全屏、位置记忆，显示后端经 `FloatingToolbarSurface` 注入（单测用假的）。事件循环经 `OverlayLoop` 注入：IBus 用 GLib 主循环，Fcitx5 用 `fcitx::EventLoop`。

### 悬浮工具栏

- 一个宿主进程一条，跟着当前属主走：IBus 的属主是引擎对象（每个输入上下文一个），Fcitx5 的属主是输入上下文。获得焦点或切换到本输入法时接手（IBus 自己的 `fake` 上下文不接手），失焦不收起，换输入法（IBus 的 `disable`、Fcitx5 的 `deactivate` 非失焦事件）或属主销毁时收起并放开。
- 显示条件是 macOS 的 `MetasequoiaFloatingToolbarShouldShow`：开关开着、有属主、前台窗口不是全屏（`_NET_ACTIVE_WINDOW` 的 `_NET_WM_STATE_FULLSCREEN`，根窗口的 `_NET_ACTIVE_WINDOW` 一变就重查）。10 秒没有按键也没有点它就隐藏，下一次按键或点击回来；同一个属主换输入框不唤醒，新的激活或换属主才唤醒，与 `activateForDelegate:visible:` 相同。
- 几何照搬 macOS 的常量（按钮字号加 8、间距 2、标志 34 或握把 10 加 4、分隔线 1.2、高字号加 20、圆角 10），按 `scale_percent` 和 `font_size` 缩放，再乘桌面缩放；默认一行 255 × 44，与 `kToolbarWidth`、`kToolbarHeight` 相同。
- 按钮动作走「工具栏」子菜单里同一项的路径：IBus 调 `property_activate` 的对应属性，Fcitx5 激活状态区里的同一个 `fcitx::Action`。打开面板、官网和「隐藏悬浮状态栏」不需要输入框，IBus 直接做。设置按钮右键是 macOS 齿轮菜单的内容，去掉 Linux 没有的「检查更新」，「表情与符号」换成水杉表情面板。
- 位置记在 `$XDG_STATE_HOME/<客户端目录>/floating-toolbar-position.json`，原子替换写入，与 Windows 的 `floating_toolbar_position.json` 一样只属于这台机器，不进同步的共享偏好；恢复时收进工作区四边 12 以内，没记过时放在焦点窗口所在显示器工作区右下角、离边 20。
- 后端只有 X11。Wayland 会话有 `DISPLAY`（Xwayland）时也用它：工具栏不跟随光标，只要一个屏幕位置，Xwayland 的根坐标在 GNOME、KDE、wlroots 上都对得上桌面布局。那里看不到 Wayland 原生窗口是否全屏，所以 `hide_on_focus_out`：失焦就收起，获得焦点再出现。没有 X 服务器或构建缺 x11/xext/xrandr/cairo-xlib/pangocairo 时没有窗口，「工具栏」子菜单照旧。

### 双拼键位提示

- 条件与 macOS 的 `updateKeymapPanel` 相同：有焦点、中文输入开着、`shuangpin_keymap_hint` 打开、双拼方案、没有局部模式、不在英文候选模式、正在组字；自定义方案不画。键位表经 `msime_client_shuangpin_key_hints` / `msime_client_shuangpin_zero_initials` 读 Engine 自己的表，按方案缓存。
- 只画给报根窗口坐标光标的客户端：Fcitx5 看输入上下文的 display 是否 `x11:`，IBus 看会话里没有 `WAYLAND_DISPLAY`，与切换徽章的判据相同。光标矩形高为 0 时不画（`MSIMEValidCaret`）。
- 摆放照 `MSIMEShuangpinKeymapPanelFrame`：左边对齐光标，放在光标下方、让开候选窗再隔 8，下面放不下就翻到光标上方，收进工作区四边 16。Linux 的候选窗由桌面面板画，宿主拿不到它的矩形，所以候选窗高度按候选字号、横竖排、条数和拼音行估计，与 macOS 在算精确几何之前的基线公式相同。
- 卡片 620 × 203、13 圆角；macOS 的标题行没有固定高度，被约束撑到 33，这里直接写成 33。高亮键用候选窗选中项的颜色（系统主题是 GNOME 蓝，不是 macOS 的青绿；皮肤用自己的选中色）。

### 能力位与设置页

`host_surface.rs` 对 Linux 打开 `shuangpin_keymap_hint` 和 `floating_toolbar_appearance`。共享设置页的悬浮工具栏页对 Linux 的判据改成 `linuxPlatform && !showToolbarAppearance`：报告外观能力的 Linux 宿主（现在就是）得到与 macOS 相同的预览、开关、尺寸和颜色链接，预览下另说明 X11、Xwayland 和退回子菜单；只有菜单的 Linux 文案仍保留给不报这项能力的宿主。

## Alternatives considered

- **把 Windows 的 `ShuangpinKeymapLayout.h` 挪到 `platforms/common` 两边共用** — 规则只有一份，Windows 和 Linux 不会漂移。这次没有做：那份头文件带着 Windows 的调色板和阴影类型，挪过去要同时改 Windows 的构建和测试，而 Windows 的键位图正在另一条并行的改动里；Linux 这份写成同样照 macOS 的纯函数，测试钉住相同的数值，合并可以单独做。
- **Wayland 用 layer-shell 画工具栏（切换徽章和语音浮层已经这样做）** — KDE、wlroots 上不经 Xwayland，缩放更准。没有做：layer-shell 只能锚在输出边上，拖动要改成调边距，指针、配置往返和缓冲都要再写一套；GNOME 没有 layer-shell，经 Xwayland 的 X11 窗口在三种合成器上都能显示、能拖。将来要做时 `FloatingToolbarSurface` 已经是注入点。
- **Wayland 上也用 layer-shell 画键位图** — 原生客户端也有提示。没有做：layer-shell 拿不到光标的屏幕坐标，键位图只能放在屏幕边上，离正在输入的那一行很远，而 macOS 的键位图是贴着候选窗的；不画比画在错的地方好。
- **工具栏跟着每个引擎对象（IBus）或每个输入上下文各建一条** — 不需要属主交接。没有采用：理由与切换徽章的进程级表面相同，每个获得过焦点的输入框各占一条 X 连接，快速换输入框还会同时留着几条工具栏；macOS 也只有一个 `sharedPanel`。
- **按钮动作在浮窗里另写一份（直接改偏好、直接调 host-api）** — 不依赖菜单那条路径的焦点检查。没有采用：同一个开关两份实现迟早行为不同，「工具栏」子菜单已经走好了保存、会话同步和失败重试。
- **设置页按 `!showToolbarAppearance` 判断要不要显示菜单版文案（[2026-10-09](../bug-fix/2026-10-09-linux-toolbar-settings-menu-copy.md) 否掉的那个）** — 现在用的是 `linuxPlatform && !showToolbarAppearance`，两个条件一起：菜单版文案仍只给 Linux，那篇担心的「别的宿主套上 Linux 文案」不会发生。

## Consequences

- **收益**：
  - X11（以及 GNOME、KDE、wlroots 的 Xwayland）上有了与 macOS 同样大小、同样按钮和状态字、同样空闲隐藏和位置记忆的悬浮工具栏；GNOME Shell 下 IBus 第一次有了工具栏入口。
  - X11 客户端的双拼用户有了键位提示，设置页给出开关。
  - 两块浮窗的规则、几何和颜色都是纯函数，各有单测；X11 窗口在 Xvfb 里真的建出来测位置、点击、提示、两种菜单、拖动和位置记忆；IBus 的接线在 Xvfb 里测获得焦点显示、失焦保留、换输入法收起。
- **代价与已知上限**：
  - 工具栏默认开着（共享默认值），所以升级后 X11 用户第一次会在屏幕右下角看到它；不想要可以在菜单里取消「显示工具栏」或点「隐藏悬浮状态栏」。
  - Wayland 原生客户端没有键位图；Wayland 会话里的工具栏经 Xwayland，失焦就收起，KDE 分数缩放下 Xwayland 窗口可能发虚。没有 Xwayland 的 Wayland 会话只有「工具栏」子菜单。
  - 键位图让开候选窗靠估计，主题加了大装饰或者字体比偏好大很多的面板可能被盖住一点；宿主拿到真实矩形（候选窗原生画布）时可以改用它。
  - 全屏只在前台窗口变化时重查，应用在同一个窗口里按 F11 进入全屏时，要到下一次换窗口才收起。
  - 菜单和提示不响应键盘（菜单抓指针，不抢键盘焦点），Escape 不会收起菜单；点到外面或换窗口才收起。
  - 属主失焦后点切换类按钮，菜单那条路径要求输入框有焦点，所以不起作用；打开面板、官网、设置和隐藏照常。
  - 键位图的规则与 Windows 的 `ShuangpinKeymapLayout.h` 是两份，改其中一份时要对照另一份，直到合并。

## Verification

- `bash platforms/linux/build-container.sh`：`linux-floating-panels-model`（规则、几何、颜色和控制逻辑，假的事件循环和后端）、`linux-floating-panels-render`（cairo、pango 实画键位图、工具栏、菜单、提示并检查像素）、`linux-floating-panels-x11` 和 `ibus-mode-hint-x11-toolbar`（Xvfb），`build-container.sh` 要求它们注册并执行。`MSIME_TEST_CANVAS_ARTIFACTS` 指向可写目录时，渲染测试和 Xvfb 测试把画面存成 PNG。
- `cargo test -p msime-client-core --lib host_surface` 钉住 Linux 的两个能力位；`apps/desktop` 的 `tests/input/host-capabilities.test.tsx` 钉住画悬浮工具栏的 Linux 页面。
- 没有在真实桌面（GNOME、KDE、Xfce、wlroots）上看过：多显示器、分数缩放、合成器阴影、真实全屏应用和托盘共存都只经过 Xvfb 和单测。
