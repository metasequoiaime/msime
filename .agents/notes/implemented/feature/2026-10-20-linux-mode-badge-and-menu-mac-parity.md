# Agent Note: Linux 中英文切换提示和输入法菜单对齐 macOS

Status: implemented

## Problem

Linux 的中英文切换提示和输入法菜单与 macOS 差得很远，同一个产品在两个平台上看起来像两套东西：

- 切换提示：Fcitx5 的自绘徽章是 macOS 改成紧凑尺寸之前的样子（44 px 高、22.8 px 半粗体字），总带产品标志，显示 1.2 秒后直接消失，固定在屏幕右下角；同时还弹一个跟着光标的面板文字提示，一次切换出两处提示。IBus 只有辅助文本，没有徽章。macOS 是光标下方一块 32 pt 的紧凑徽章，标志默认不显示，0.6 秒后在 0.18 秒内淡出。
- 模式字：Linux 没有「双」「五」，IBus 面板还写「文」和「A」。
- 菜单：顺序、层级和名字都不同。英文候选模式和繁体输出收在「输入选项」里，表情、云剪贴板、云词库、屏幕键盘和手写收在「桌面工具」里，名字是「表情与符号」「手写识别板」「设置…」「主题：X」「86 五笔」「日文」，没有快捷键提示；Fcitx5 每一行都画同一个键盘图标。工具栏子菜单只有八项，表情默认开（共享默认值是关），没有切换方案、手写、语音，也没有 macOS 齿轮菜单里的宣传、官网、帮助、关于和反馈。
- 其他：按住按键时每次自动重复都响一次按键音（macOS 只响一次）；IBus 组件的图标名 `msime-client` 不存在；升级后 IBus 什么都不说，Fcitx5 只在辅助行里给一条命令。

## Decision

文案、顺序和行为照 macOS 改，视觉风格不动（产品决定：共享页面和候选窗的「系统」主题在 Linux 上保留 GNOME/Adwaita 的样子）。

切换提示（`platforms/linux/src/overlay/ModeBadge*`）：

- 尺寸与 macOS `InputModeHUDPanel.mm` 相同：18 px 常规字重，32 px 高，左右 8 px 内边距，8 px 圆角；标志跟随 `show_app_logo`，默认不画。显示 0.6 秒后分四帧在 0.18 秒内淡出（`ModeBadgeTiming`）。有合成器时画一圈阴影并淡出；没有合成器的 X11 上透明像素会变黑，所以不画阴影、到时直接收起。
- 两个宿主各只有一块徽章表面（Fcitx5 在 `FcitxEngine` 上，IBus 是进程级的 `process_mode_badge`），第一次真要画时才建（`mode_badge_surface_wanted`）：新的提示替换屏幕上那块，与 macOS 只有一个提示面板相同；IBus 为每个输入上下文建一个引擎对象，徽章跟着引擎走的话，每个获得过焦点的输入框都各占一条显示服务器连接，快速换输入框时还会在几个光标下同时留着几块徽章。
- 聚焦提示现在由[独立偏好](../bug-fix/2026-10-22-linux-focus-mode-hint.md)控制，默认关闭；开启后（#2589）光标徽章先等客户端报光标：GTK、Qt 的输入法模块在 FocusIn 之后才在空闲回调里补报光标位置，那一刻的矩形是上一个位置或全 0。Fcitx5 等 `InputContextCursorRectChanged`，IBus 等 `set_cursor_location`，最多等 `ModeBadgeTiming::focus_caret_wait_ms`（150 ms，矩形没变时 Fcitx5 不发变化事件）；等完仍没有可用矩形就改用面板的文字提示（`mode_badge_focus_hint`），不把徽章放到屏幕下方。真正的切换不等，那时客户端早已报过光标。「可用」只排除四个分量全为 0 的矩形（`mode_badge_caret_usable`），等待期间报来的全 0 矩形也不算，接着等：macOS 把高度为 0 的也当成没报，Linux 不能照搬，XIM 客户端（xterm、X 下的 Emacs、Wine 和 Java 程序）报的就是一个宽高为 0 的点，Fcitx5 的 XIM 前端和 ibus-x11 都这样传，评审前按高度判断时，这些客户端的切换徽章被放到屏幕下方的回退位置，原来跟着光标的面板提示也没了。
- 一次切换只出一处提示（`ModeBadgePlacement.h` 的 `mode_badge_hint`）：X11 客户端（Fcitx5 看输入上下文的 display，IBus 看会话里没有 `WAYLAND_DISPLAY`）时，徽章按 `MSIMEInputModeHUDFrame` 的规则放在光标下方 10 px，下面放不下就翻到上方，夹在工作区 8 px 以内，没有光标时放在下沿离底边四分之一高的位置，不再发面板提示。其余情况由面板的文字提示跟随光标；只有用户要了标志且允许角落提示时，带标志的徽章另放在屏幕角上，因为面板提示带不了图。Linux 设置页的「屏幕角落模式提示」（`input_mode_corner_hud`）默认开启以保留旧配置行为；关闭后不建仅用于角落的表面，保留面板文字、X11 光标徽章和其他位置的标志。此开关控制提示位置，`input_mode_hud` 和 `input_mode_focus_hud` 仍分别控制切换和聚焦触发。
- 离开输入框或换到别的输入法时立刻收起这个上下文自己的那块徽章（Fcitx5 在 `deactivate`，IBus 在 `focus_out`），与 macOS 在 `deactivateServer:` 里 orderOut 提示面板相同；还在等光标的获得焦点提示在用户按下第一个键时作废，否则候选窗刚出现在光标下方，徽章随后放上去会盖住它。macOS 打字时不收起已经显示的徽章，这里也不收。
- IBus 也链接同一套后端，有 X11 或 layer-shell 时画徽章，否则保留辅助文本。辅助文本的显示时间也改成 0.6 秒。

菜单（`platforms/linux/src/core/MenuLabels.h` 是两个前端共用的文案）：

- 顶层顺序与 macOS `InputController.mm` 的 `menu` 相同：中文输入、英文候选模式（Ctrl+Shift+E）；繁体输出、全角字符（Ctrl+Shift+Space）、中文标点（Ctrl+.）、显示译文；输入方案（当前方案）、主题（当前主题）；显示工具栏和工具栏子菜单、水杉表情面板…、云剪贴板…、云词库…、水杉屏幕键盘…、手写输入…、开始/结束语音输入；水杉输入法设置…、关于水杉输入法…。之后是 Linux 独有的词库…、候选操作、九键、剪贴板、三个选项子菜单和「桌面工具」。
- 方案名与 macOS 相同（双拼（小鹤）、五笔 86、日语、韩语、越南语）。Fcitx5 的方案子菜单按 Engine 顺序排成一组；IBus 保留「中文」一行和两组单选，两组内部按 Engine 顺序。
- Fcitx5 的状态区由 `arrangeStatusArea` 按顺序重新 addAction 一遍（已在的动作会移到末尾），工具栏和语音两项要等会话建好才定，`refreshToolbar` 和 `syncVoiceAction` 会再排一次。只出现在子菜单里的动作不带图标。
- 工具栏子菜单的项目、顺序和默认值与 macOS 悬浮工具栏相同，后面接齿轮菜单的宣传、官网、帮助、关于和反馈。`english_mode` 管的是中文输入那一项（macOS 的 `_inputModeButton`），工具栏里没有「英文候选模式」，它只在顶层；两个宿主的条目文案都取自 `MenuLabels.h`，与顶层相同。Fcitx5 的托盘在失焦后仍列最近的上下文，这时子菜单不列语音和宣传（它们要往有焦点的输入框里写），失焦时随状态区一起重建；`host_surface.rs` 对 Linux 报 `floating_toolbar_input_scheme`、`floating_toolbar_voice`、`floating_toolbar_handwriting`，设置页给这三个开关。Fcitx5 不加 Ctrl+Shift+P：macOS 和 Windows 的「宣传水杉输入法」都只在菜单里，而 Linux 上 VS Code、JetBrains、终端和浏览器都把 Ctrl+Shift+P 用作命令面板或隐私窗口，插件吃掉它会把四行介绍打进用户的文件；IBus 宿主原有的这个组合键不在本次范围里，没有改。

其他：

- 模式字加「双」「五」，IBus 面板符号改为「中」「英」（`InputModeIndicator.h`）。「⇪」仍保留，这是 Windows 语言栏的规则，macOS 没有。
- 自动重复不出按键音、不画特效，与 macOS 的 `!event.isARepeat`（`InputController.mm` 里 `reportKeyToLifecycle:` 的调用处）相同；但每次重复仍调 `msime_client_key(MSIME_KEY_POINTER)` 开新的一轮。最初的做法是重复时根本不调，评审指出那样按住退格时每次重复的派发（input、candidates、composition_update）都算在第一次按下那一轮里，共用 125 000 的每轮 fuel，运行到一半被截断的插件在这次聚焦里就停掉了；`MSIME_KEY_POINTER` 是 host-api 现有的「开一轮但不出声不画」，借它不用改核心，`script_plugins.rs` 的 `a_silent_turn_per_repeat_keeps_the_plugin_running` 钉住这个行为。重复按硬件键码认，不按 keysym：先松开 Shift 再松开 1 时松开事件带的是 `1` 而按下时是 `!`，按 keysym 记会把下一次 Shift+1 当成重复。`msime_client.h` 顺序契约下的过渡期例外随之写明自动重复借指针位开轮；这一条取代[桌面端生命周期核心](../architecture/2026-10-11-desktop-lifecycle-core.md)里「自动重复照旧出声」（Q3）在 Linux 上的做法，那篇已标注，Windows 仍按原样带 `MSIME_KEY_REPEAT` 出声。
- IBus 组件图标改为 `msime-linux`，引擎描述是产品名；Fcitx5 条目和桌面入口加 `zh_TW`、`zh_HK` 名称。
- 升级后：IBus 启动器换上新版本后经 `msime-linux-first-run-guide --reason upgraded` 发「水杉输入法已更新」通知；Fcitx5 插件发同一个标题的通知，带「立即重启 Fcitx5」按钮。KDE Plasma 的 Wayland 会话里 KWin 把 Fcitx5 当作虚拟键盘自己启动、并把输入法协议的连接交给它，父进程是 `kwin_wayland` 时既不给 `fcitx5 -r`（替身拿不到那条连接，Wayland 原生应用会打不了字），也不给 `systemctl --user restart`（Fcitx5 继承了 KWin 的 cgroup，读出来的服务是 `plasma-kwin_wayland.service`），通知不带按钮，只请用户注销或在系统设置里重新选一次虚拟键盘（`ReplacedProgram.h` 的 `kwin_launched`）。

历史检索：[桌面端生命周期核心](../architecture/2026-10-11-desktop-lifecycle-core.md)的「自动重复照旧出声」在 Linux 上被本篇取代，已在那篇标注；[macOS 紧凑切换提示](../bug-fix/2026-10-14-macos-compact-input-mode-hud.md)定下了这里照搬的尺寸，部分重叠，互链；[Linux 设置页把工具栏写成输入法菜单](../bug-fix/2026-10-09-linux-toolbar-settings-menu-copy.md)说明工具栏在 Linux 上是菜单，这里没有改，只是让它的项目和默认值跟 macOS 一致，并打开那篇说过不动的三个能力位；[Windows 托盘菜单对齐 macOS](2026-10-10-windows-tray-menu-mac-parity.md)是同一件事在 Windows 上的做法，部分重叠；[Fcitx5 模式提示补宽](../bug-fix/2026-10-07-fcitx5-mode-hint-width.md)的补宽在面板提示那条路上照旧生效。

角落提示的独立偏好由 `appearance_preferences_absent_defaults_and_roundtrip` 检查旧配置缺字段、关闭后的保存与恢复默认；`input-mode-hud-section.test.tsx` 检查角落开关不修改切换或聚焦开关；`mode_badge_style.cpp` 检查关闭角落提示后保留面板文字和 X11 光标徽章。

IBus 原先保留的宣传快捷键后来由 [Linux 移除宣传彩蛋快捷键](../bug-fix/2026-10-22-linux-remove-promotion-shortcut.md)移除；Fcitx5 的菜单入口仍按本篇决定保留。

## Alternatives considered

- **IBus 和 Fcitx5 也做成「中文输入」「英文输入」一对单选** — 与 macOS 一模一样。没有采用：IBus 的 `InputMode` 属性同时是面板读 中/英 符号的那个属性，ibus-ui-gtk3、GNOME Shell 和测试都按开关协议激活它（CHECKED/UNCHECKED）；拆成单选要么改这套协议，要么多一个隐藏的同步属性。一个勾选即中文的开关表达的是同一个状态。
- **IBus 方案菜单去掉「中文」一行，像 macOS 一样做成一个单选列表** — 与 macOS 一致，也少一行。没有采用：「中文」这一行的作用是回到上次用的中文方案，去掉它要先决定这个入口放哪里，这是产品行为，不在这次范围里。
- **Wayland 上也只显示徽章，放弃面板提示** — 一处提示，有图。没有采用：layer-shell 拿不到光标的屏幕坐标，徽章只能放在角上，离用户正在看的地方很远；面板提示由合成器放在光标旁，更接近 macOS。
- **Fcitx5 状态区也画分隔线** — classicui 和托盘菜单会画出分组。没有采用：kimpanel 把状态动作原样列出，分隔线在那里是空白项，KDE 用户的状态区会多出几个空按钮。
- **按模式出单色托盘图标（`*-symbolic.svg`）** — 与 macOS 的每模式图标一致。这次没有做。工具不是障碍：已经链接的 Pango 和 Cairo 可以把字形画到 `cairo_svg_surface_create` 的 SVG 表面上，输出的就是轮廓路径，不需要新依赖。没定的是随包分发的字形 SVG 取自哪套字体、按 OFL 怎样署名，这要先有字体来源和许可记录。

## Consequences

- **收益**：X11 上的切换提示与 macOS 位置、大小、时长一致，一次只出一处；两个前端的菜单顺序、名字和快捷键提示与 macOS 相同，文案只有一份；工具栏子菜单的内容和默认值与 macOS 工具栏相同；按住退格不再连响。
- **代价与已知上限**：
  - Wayland 原生客户端仍看不到跟着光标的徽章，只有面板文字提示；GNOME Wayland 下 IBus 的徽章（打开标志时）经 Xwayland 显示在角上。
  - IBus 的徽章只在纯 X11 会话里跟随光标：同一个引擎服务 Wayland 原生客户端时分不出坐标系，宁可用辅助文本。
  - Fcitx5 顶层从 24 项变成 32 项（失焦时 23 项），kimpanel 的按钮更多。
  - Linux 的聚焦提示可以独立开启以满足 #2589，默认只在切换时提示；触发控制见[独立偏好](../bug-fix/2026-10-22-linux-focus-mode-hint.md)。
  - GNOME Shell 的 IBus 菜单仍只有两项（Shell 的属性树会卡死桌面），每模式图标、「检查更新…」入口和设置页的「已更新到 X」状态卡都没有做。
  - 升级通知的按钮需要 libnotify 0.7.10 起的 `notify-send --action`；带按钮的通知会让一个 `notify-send` 等到用户关掉它。

## Verification

- `bash platforms/linux/build-container.sh`（Debian 容器里编译 IBus 宿主、Fcitx5 插件和全部单测并跑 ctest，评审修正后 98 项全过）：`ibus-mode-hint-x11-badge` 和 `ibus-mode-hint-x11-badge-no-caret`（门禁镜像为此装了 `xvfb`、`xauth`，`build-container.sh` 要求这两项注册并执行）在 Xvfb 里跑真实的 IBus 宿主：获得焦点后客户端没报光标前不画，经 D-Bus 报来光标后徽章出现在光标下方且没有辅助文本，引擎在徽章显示期间销毁后徽章照常收起；等满时间仍没有光标时改发辅助文本、不画徽章。`linux-mode-badge-style`（尺寸、淡出帧、光标定位、提示规则、获得焦点时的回退）、`linux-mode-badge-render`（用 cairo 和 pango 实画徽章，检查本体、描边、阴影边和半透明淡出）、`linux-wave-overlay-placement`、`linux-menu-labels`、`linux-input-mode-indicator`、`linux-wubi-profile-names`、`ibus-mode-hint-*`、`linux-fcitx5-contract`、`linux-ibus-component-contract`、`linux-settings-launcher-contract`、`linux-first-run-guide`。
- 第二轮评审修正（XIM 点状光标、自动重复开轮并按键码认、失焦与打字时收起徽章、工具栏映射与文案、KWin 下的升级通知）之后在 Studio 上重跑 `bash platforms/linux/build-container.sh`，99 项全过，其中新加的 `ibus-mode-hint-x11-badge-point` 在 Xvfb 里验证全 0 的光标报告不算、点状光标 `(200, 300, 0, 0)` 下徽章画在它下方、失焦后 50 ms 内收起、再次获得焦点后先打字再报光标时不画徽章也不发辅助文本；`linux-key-sound`（键码认重复）、`linux-mode-badge-style`（点状光标的位置和获得焦点时的提示）、`linux-replaced-program`（`kwin_launched`）、`linux-first-run-guide`（KWin 下的通知不带按钮和命令，两个宿主的标题相同）、`linux-fcitx5-contract`（自动重复开轮、工具栏映射和文案、收起徽章的调用位置）。`cargo test -p msime-host-api --lib -- tests::script_plugins` 跑 `a_silent_turn_per_repeat_keeps_the_plugin_running`：一次打字键加 50 次 `MSIME_KEY_POINTER` 各接一次派发，51 次全部投递、插件不记失败。没有在真实的 KWin Wayland 会话里看过升级通知，XIM 客户端的徽章位置也只经过 Xvfb 里的 IBus 用例。
- `cargo test -p msime-client-core --lib host_surface` 钉住三个工具栏能力位。
- 菜单顺序和方案名由 `tests/runtime/engine_smoke.cpp`、`fcitx5/tests/native.cpp` 钉住，它们只在 `tests/tools/check-container.sh` 的验收容器里跑（要已校验的词库）。写这篇时 develop 上这两个测试本身就过不去：`ibus-engine-smoke` 在分支的基线提交上同样停在「IBus did not keep an input-method ASCII digit as smart-punctuation context」，Fcitx5 的几个 native 模式停在「Disabled expression mode must pass through」一类已关闭的快捷模式仍吃掉 Shift 组合键的断言。把这几条基线就失败的断言改成只打印之后，`ibus-engine-smoke` 全部通过（含顶层顺序、方案菜单的各项激活）；`fcitx5-native-context` 走过了状态区数量、顺序、文案和选项子菜单的全部检查，停在更后面与菜单无关的翻译 socket 用例上。
- 徽章在 Fcitx5 验收镜像里的真实 GTK 编辑器、classicui、原生画布和 Wayland 画布用例里都画得出来、不挡输入（`fcitx5-gtk-editor`、`fcitx5-*-canvas-*` 通过；`fcitx5-native-canvas-vertical` 在一次整轮运行里因测试自己读词库的竞态失败，单独重跑通过）。还没有在真实桌面上看过：GNOME、KDE 的托盘菜单和 IBus 面板的实际样子、X11 徽章跟随光标的位置都只经过单测和容器。

- #9282 集成验证（2026-10-23）：`bash scripts/verify-local.sh --quick` 完整预热重跑通过。首次遇到 Swift 冷缓存超时及 Docker DNS 失败，Swift 两项和 Linux 原生门禁单独重跑通过后，再完整重跑确认。Linux 原生门禁编译 IBus、Fcitx5、full/wubi 并执行测试；这不是实际桌面会话验收。共享设置页五项专项及徽章策略检查已通过。
- 集成提交时格式门禁指出九个上游 TypeScript 文件不符合当前 `oxfmt` 输出；仅按门禁列出的路径机械格式化并检查，没有修改行为。
- `input_mode_corner_hud` 在 macOS 偏好覆盖表中声明为 Linux 专用：macOS 没有对应的角落徽章入口，共享设置页也不展示此开关。新提交的 macOS CI 已通过旧审查指出的 `settings-glass`，但因缺少此平台边界声明在 `preference-coverage` 失败；本地复现同一错误后补齐声明，不以修改宿主行为或删除覆盖检查绕过门禁。
