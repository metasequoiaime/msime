# Agent Note: Windows 悬浮工具栏、中英切换提示、打字特效和托盘菜单对齐 macOS 的尺寸、字形与文案

Status: implemented

## Problem

对照 macOS 逐项检查 Windows 的悬浮工具栏、中英切换提示、打字特效和托盘卡片，按钮集合和交互大体一致，差在两处被覆盖的值和几处各自演化的细节。

- 工具栏按钮的顺序是 全/半 在 标点 之前，macOS 是 标点 在前，`platforms/windows/README.md` 还写着「与 macOS 一致」。偏好里缺 `emoji` 键时 Windows 画表情按钮，macOS 和 client-core 都不画。
- `toolbar_metrics` 把栏高算成 font×2+4（24 号时 52），按钮带 36 高，`toolbar_palette` 又把圆角钉成 8、描边 1.4；macOS 是 font+20 高（44）、按钮 font+8 的正方形、圆角 10、描边 1。logo 后面还画一条 2 DIP 宽的强调色拖动条，macOS 有意去掉了它，只留 logo 或握把。悬停是左右各缩 2 的药丸，按下用选中色；macOS 填满整个按钮、按下用 hover 色。
- 中/英、标点、全半角、简繁画的是 Segoe Fluent Icons 的私有区字形（Windows 10 没有这个字体），macOS 画的是按钮标题文字，字号 0.95。西文标点画「,」，藏文画「藏」，Engine 的英文模式画带下划线的「En」，macOS 分别是「.」「ཀ」和不带下划线的「En」。
- 只有 logo 和右边 8 DIP 的条能拖动，按钮之间、上下留白和尾部都按不动；macOS 的面板 `movableByWindowBackground`，只有按钮不拖。
- 中英切换提示的尺寸由 `toolbar_metrics` 推出来，跟着工具栏的字号和缩放变大，logo 无视 `show_app_logo`，0.6 秒后直接消失。macOS 在 [紧凑中英文提示](../bug-fix/2026-10-14-macos-compact-input-mode-hud.md) 里改成固定的 18 号字、高 32，尊重 `show_app_logo`，再用 0.18 秒淡出。
- 打字特效的强调色取主题的 accent，macOS 取系统的 `controlAccentColor`。
- 托盘卡片的语言行写「中文」「英文」，方案写「日文」「韩文」「越南文」，末两行是「设置…」「关于水杉输入法」，没有 macOS 输入菜单里的「云词库…」，云剪贴板在工具条之后隔着一段。工具栏的方案菜单把缺词典的方案列成不可用，macOS 干脆不列。

## Decision

以 macOS 为准逐项对齐，Windows 只在做不到或有版本差异的地方保留自己的写法。

**工具栏几何（`platforms/windows/src/candidate/ToolbarLayout.h`）。** `ToolbarMetrics` 按 macOS 的 `applySizingPreferences:` 给出：`button` = font+8，`cell` = button+2，`height` = font+20，`icon_top`/`icon_bottom` 把按钮竖直居中（上下各 6）。logo 槽之后依次是 `logo_gap` 4（仍算 logo 的一部分）、`divider_gap` 3、`divider` 1.2、`button_gap` 4，最后一个按钮之后是 `trailing` 6；这些只乘用户缩放，不随字号变。`handle` 字段删掉。`toolbar_cell` 返回按钮正方形，`toolbar_button_at` 对按钮之间的 2 DIP 返回空，`toolbar_divider` 给出分隔线，`toolbar_button_radius` 是按钮高的四分之一、不小于 2。logo 槽固定 34（`kToolbarLogoWidth`），`show_app_logo` 关着时换成固定 10 的握把（`kToolbarGripWidth`），也只乘用户缩放；`toolbar_logo` 画 22 见方的 logo（`kToolbarLogoMarkSide`），与 macOS 一样在 logo 视图和栏高里各留出至少 2 DIP 的边，放不下时才缩小。所以无论 logo 还是握把、无论字号，整条宽度都与 macOS 的 `ToolbarPreferredWidth` 逐 DIP 相同。

**工具栏外观（`FloatingToolbarWindow.cpp`、`CandidatePalette.h`、`CandidateSkin.h`）。** `toolbar_palette` 只设圆角 10、描边 1，不再覆盖 `item_radius`。不画强调色拖动条，分隔线画成 1.2 宽、按钮那么高的实心矩形，没有按钮时不画。`apply_toolbar_skin` 照旧读皮肤包 `[toolbar]` 的 `handle` 颜色，但不再映射到 accent，与 macOS 的 ToolbarSkinTokens 一样没有落点。悬停和按下都用 `palette.hover` 填满整个按钮，按下时字的颜色不变。

**按钮字形（`ToolbarIcons.h`）。** 四个模式按钮没有 codepoint，只给文字：中、英、A、En、双、五、日、한、粤、注、越、ཀ、笔，。和 .，全和半，简和繁。文字用 `toolbar_text_font`（Microsoft YaHei UI）常规字重、字号乘 `toolbar_glyph_scale`（0.95）；「ཀ」带 `family` = Microsoft Himalaya，缺这个字体时由 DirectWrite 自己回退。`ToolbarIcon::underline` 删掉。切换输入方案、表情、手写、屏幕键盘、语音和设置仍画图标字体的字形、字号不乘 0.95，字体里没有时画替代字。任务栏语言栏图标不走 `toolbar_icon`，藏文在那里仍是「藏」。

**按钮顺序与缺省（`FloatingToolbarSettings.h`）。** `floating_toolbar_slots` 按 中/英、切换输入方案、标点、全/半、简繁、表情、手写、屏幕键盘、语音、设置 排列；`items` 的缺省把表情改成关，`PreviewConfig::floating_toolbar_items` 的缺省同步改：启动快照缺这六个键、或偏好的缩放和字号超出范围读不出工具栏设置时，Server 用的是它。WinUI 设置窗口里「工具栏按钮」的复选框仍按共享设置页 `floating-toolbar-options.ts` 的顺序（全/半在标点之前），预览按工具栏的实际顺序画。

**拖动。** `toolbar_drag_at_pixel` 是「在卡片上且不在按钮上」，按下时发 `WM_NCLBUTTONDOWN` + `HTCAPTION`；阴影边距不算。移动光标 `IDC_SIZEALL` 只在 logo 槽上出现（`toolbar_logo_at_pixel`），对应 macOS 只在 logo 视图上显示张开手形，其余地方照样能拖但不改光标。

**图片工具栏（[原生宿主图片工具栏](2026-10-21-native-image-toolbar.md)）。** 以上几何、外观和拖动只管普通条。皮肤包带 `toolbarImage` 时 `FloatingToolbarWindow` 走 `ToolbarImageState`：不画底色、阴影、边框、logo 或握把和分隔线，按钮矩形、图标字号来自共享布局桥；拖动仍按背景图 alpha 判定（`drag_at`），移动光标出现在所有能拖的地方，因为图片条没有 logo 可以指示；悬停仍是左右各缩 2 的药丸、按下用选中色，按钮没有状态图时文字用 Segoe UI 按皮肤给的图标字号画，藏文换成 `ToolbarIcon::family`。两种模式共用 `cell`、`button_at_pixel`、`drag_at_pixel`、`card_at_pixel` 这几个入口，绘制、命中、提示和读屏不会各算各的。

**中英切换提示（`InputModeHudLayout.h`、`InputModeHudWindow.cpp`）。** `input_mode_hud_metrics(show_logo)` 返回固定值：字号 18、高 32、内缩 8、logo 16、间距 4、字宽 18、圆角 8、描边 1，阴影边距沿用工具栏的。窗口只按显示器 DPI 换算，不乘 `scale_percent`；画不画 logo 由 `set_show_logo` 决定，再看可执行文件里有没有图标；Server 用 `hud_show_app_logo`（`FloatingToolbarSettings.h`）单独读偏好 `show_app_logo`，只认布尔 true，与 macOS 的 `applySizingPreferences:` 相同，不经 `floating_toolbar_settings`：工具栏的字号或缩放越界时那份设置整个作废，提示的 logo 不该因此消失。字用 Microsoft YaHei UI 常规字重。0.6 秒后开一个 16 毫秒的帧计时器，`input_mode_hud_fade_opacity` 在 180 毫秒里从 1 线性降到 0，每帧把整张徽标连同阴影画进一个带不透明度的 `ID2D1Layer`，降到 0 时隐藏；淡出中再次切换时直接回到完全显示，显示设置或 DPI 变化时直接收起。

**打字特效的强调色（`src/system/SystemAccent.{h,cpp}`、`CandidateWindow.cpp`）。** 每次取到特效时，候选窗经 `SystemAccent::rgb()` 读 `Windows.UI.ViewManagement.UISettings` 的 `GetColorValue(UIColorType::Accent)`，存进 `effect_accent_`，交给浮层的 `presentation.accent`，也用于卡片闪光和卡片里画的升档连击数；读不到时退回主题的 accent。特效包给了颜色时仍以包为准。`IUISettings3` 按 Windows SDK 元数据自己声明，`RoActivateInstance` 和 `WindowsCreateStringReference` 运行时从 combase.dll 取，与 msimeui 的 `CompositionInterop.h` 同一个理由：MinGW 和 MSVC 编同一份代码。UISettings 对象在候选窗的界面线程上建一次留着，改了强调色不用重启。

**托盘卡片（`TrayMenuLayout.h`、`ShellSurfaces.h`）。** 语言行是「中文输入」「英文输入」。日语、韩语、越南语和藏文方案下第一行写「日语输入」「韩语输入」「越南语输入」「藏文输入」：macOS 不改这一行，但 Windows 有只提供日语、越南语或藏文的版本，在那里写「中文输入」是错的。方案行的名字与 macOS 方案菜单相同：日语、韩语、越南语。图标工具条保留，顺序改成工具栏、表情、键盘、手写、语音（macOS 输入菜单里同一组行的顺序）。之后是「云剪贴板…」「云词库…」两行，紧接着「词库…」「水杉输入法设置…」「关于水杉输入法…」，中间不另加分隔线（macOS 这一组只在设置之前有一条，工具条之后已经有一条），产品名取 `TrayMenuCapabilities::product_name`。多了一行，卡片在矮工作区里压缩时命令行的最小行高从 28 改成 26 DIP（`tray_menu_compact_row_height`），压缩后的主页仍是 499 DIP，200% 缩放的 1080p 工作区（约 500 DIP）放得下，最后一行不被裁掉。「云词库…」是新命令 `TrayMenuCommand::OpenCloudDictionary`，`shell_surface_request` 把它映射到 `cloud-dictionary` 路由，由 MSIME.exe 打开共享应用的云词库面板，与 macOS 的 `MSIMEOpenDesktopCloudDictionary` 打开的是同一个面板（client-core 的 `SurfaceRoute::CloudDictionary`）；共享应用不在时这一行不可用。`tray_menu_scheme_items` 多一个 `omit_unavailable` 参数，悬浮工具栏的方案菜单传 true，缺词典的粤拼、注音、笔画不列出；托盘的方案页仍列出并显示为不可用。

## Alternatives considered

- **托盘工具条换成七行，与 macOS 输入菜单逐行一致** — 文字行能放下「水杉表情面板…」「手写输入…」「开始/结束语音输入」这些完整标题，键盘导航也只剩上下。但七行替换一条 52 DIP 的工具条，卡片高出约 170 DIP，150% 缩放的 1080p 工作区里命令行就要被压到 28 DIP 才放得下；用户也明确要保留工具条，只改顺序、文案并补云词库。
- **去掉托盘的「词库…」，只留「云词库…」** — 与 macOS 输入菜单完全一致。但用户只要求补云词库，没有要求删词库；「词库…」直接打开设置窗口的词库页，是 Windows 上进本机词库最短的路。
- **托盘方案页也不列缺词典的方案** — 与 macOS 输入菜单一致。托盘方案页把它们画成不可用，用户能看到这个方案存在、只是缺数据；工具栏的方案菜单是一次点击的快捷入口，按 macOS 只给能用的方案。
- **语言行在任何方案下都写「中文输入」** — 与 macOS 一字不差。但日文版、越南文版和藏文版根本没有中文方案，第一行写「中文输入」会把用户正在用的语言说错。
- **打字特效继续用主题强调色，反过来改 macOS** — 主题强调色与候选窗、工具栏同色，换主题时特效也跟着换。用户选了与 macOS 相同的规则：特效是系统级的反馈，跟随系统强调色。
- **用 `DwmGetColorizationColor` 取强调色** — 一个 Win32 调用、不碰 WinRT。但它是 DWM 的着色色，带着系统混入的透明度，用户关掉「在标题栏和窗口边框上显示强调色」时也未必等于设置里选的强调色；`UISettings.GetColorValue(Accent)` 才是系统给应用的强调色。
- **用 C++/WinRT 或 SDK 的 `windows.ui.viewmanagement.h` 调 UISettings** — 少写一段接口声明。但 Server 同时用 MinGW 和 MSVC 编，MinGW 带的 WinRT 头文件版本旧、不带 C++/WinRT，msimeui 已经为同样的原因自己声明 Composition 接口；这里只用一个方法，自己声明最省。
- **提示的淡出用 DirectComposition visual 的 Opacity 或 `SetLayeredWindowAttributes`** — 不用每帧重画。但窗口经合成交换链绘制，`SetLayeredWindowAttributes` 管不到；`DeviceResources` 没有把 visual 暴露出来，为一个 180 毫秒的淡出改共享库不划算。徽标很小，11 帧重画的开销可以忽略。
- **logo 槽和握把继续随字号缩放（font×1.5、icon×10/24）** — 大字号时 logo 跟着变大。但 macOS 的 logo 槽、握把和 22 DIP 的 logo 都是固定值、只乘缩放，跟着字号走只有 24 号时与 macOS 同宽，其他字号下整条宽度和 logo 大小都对不上。
- **托盘云词库之后再加一条分隔线、最小行高仍是 28** — 云行和设置页入口分得更开。但 macOS 输入菜单在这里没有分隔线，多这 9 DIP 让压缩后的主页变成 536 DIP，200% 缩放的 1080p 工作区放不下，最后一行「关于水杉输入法…」被摆放时裁掉。

## Consequences

- **收益**：
  - 同一份偏好下 Windows 工具栏的顺序、高度、按钮大小、圆角、分隔线和拖动区域与 macOS 相同，握把模式下宽度也相同；模式按钮在 Windows 10 上也画得出和 macOS 一样的字。
  - 提示不再随工具栏放大，`show_app_logo` 同时管工具栏和提示；消失时有淡出。
  - 打字特效的颜色与 macOS 用同一条规则。
  - 托盘里能打开云词库面板，文案与 macOS 输入菜单一致；工具栏的方案菜单不再出现点不了的行。
- **代价与已知上限**：
  - 按钮之间和上下留白现在会拖动工具栏，偏一点点按下去就进了系统的移动循环（不动就松开不会记位置）。用户反馈误拖多时要重访，可以只让 logo 一段拖动。
  - 托盘主页多了一行，设计高度从 551 DIP 变成 583 DIP。150% 缩放的 1080p 照样按设计尺寸放下；更矮的工作区里命令行最矮压到 26 DIP，比原来的 28 更挤，14 号字上下各剩约 3.5 DIP，观感要在真机上看。
  - 藏文按钮依赖 Microsoft Himalaya，它的「ཀ」比同字号的汉字小一些，观感要在真机上看。
  - 系统强调色每次特效都读一次（`GetColorValue` 是进程内调用）；UISettings 建不起来时（激活失败）一直用主题的 accent，不重试。
  - 皮肤包 `[toolbar]` 的 `handle` 颜色从此不起作用，写了它的包在 Windows 上的工具栏少了那条彩色竖条。
  - WinUI 设置窗口的工具栏预览只能在 Windows CI 上编译和截图核对。

## Verification

`platforms/windows/tests/ui/` 下的 `toolbar_layout.cpp`（macOS 的高度、按钮、间隔、分隔线和尾部，logo 和握把两种模式在各字号下的整条宽度、22 DIP 的 logo，按钮之间不是按钮）、`toolbar_coordinates.cpp`（按钮上下留白、按钮之间、分隔线一带和尾部都拖动，只有 logo 一段显示移动光标）、`toolbar_icons.cpp`（模式按钮是文字、「.」「ཀ」「En」、藏文的字体）、`floating_toolbar_reload.cpp`（按钮顺序、表情缺省关）、`preview_config.cpp`（启动配置缺 `floating_toolbar_items` 时表情同样缺省关）、`candidate_palette.cpp`（圆角 10、描边 1）、`candidate_skin.cpp`（`handle` 不再改 accent）、`input_mode_hud_layout.cpp`（固定尺寸、logo 开关、淡出曲线）、`tray_menu_layout.cpp`、`toolbar_menus.cpp`、`shell_surfaces.cpp`、`accessible_elements.cpp`（托盘文案、顺序、云词库路由、工具栏方案菜单省略缺词典的方案，主页在 500 DIP 的工作区里压缩后放得下）。这些纯头文件的单测在 macOS 上用 clang++ 编译运行通过，Server 源文件和测试用 `x86_64-w64-mingw32-g++ -std=c++17 -fsyntax-only` 检查过。没有在 Windows 上运行过：工具栏和提示的实际观感（YaHei 字的位置、Himalaya 的「ཀ」、淡出的流畅度）、按钮之间拖动的手感、`UISettings` 在 Server 线程上取到的强调色、托盘「云词库…」拉起的面板，以及设置窗口工具栏预览的编译和截图，要由 Windows CI 和真机确认。

相关：[Windows 悬浮工具栏补齐 macOS 的按钮、菜单与行为](2026-10-10-windows-floating-toolbar-mac-parity.md)（按钮组与菜单）、[Windows 托盘菜单补齐 macOS 输入菜单](2026-10-10-windows-tray-menu-mac-parity.md)（卡片的页与键盘操作）、[Windows 中英文切换体验对齐 macOS](2026-10-10-windows-input-mode-parity.md)（提示何时出现、摆在哪里，尺寸由本篇取代）、[Windows 打字特效的光标浮层](2026-10-10-windows-typing-effects-caret-overlay.md)（强调色来源由本篇取代）。
