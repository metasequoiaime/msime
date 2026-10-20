# Agent Note: Linux 中英文切换提示和输入法菜单对齐 macOS

Status: implemented

## Problem

Linux 的中英文切换提示和输入法菜单与 macOS 差得很远，同一个产品在两个平台上看起来像两套东西：

- 切换提示：Fcitx5 的自绘徽章是 macOS 改成紧凑尺寸之前的样子（44 px 高、22.8 px 半粗体字），总带产品标志，显示 1.2 秒后直接消失，固定在屏幕右下角；同时还弹一个跟着光标的面板文字提示，一次切换出两处提示。IBus 只有辅助文本，没有徽章。macOS 是光标下方一块 32 pt 的紧凑徽章，标志默认不显示，0.6 秒后在 0.18 秒内淡出。
- 模式字：Linux 没有「双」「五」，IBus 面板还写「文」和「A」。
- 菜单：顺序、层级和名字都不同。英文候选模式和繁体输出收在「输入选项」里，表情、云剪贴板、云词库、屏幕键盘和手写收在「桌面工具」里，名字是「表情与符号」「手写识别板」「设置…」「主题：X」「86 五笔」「日文」，没有快捷键提示；Fcitx5 每一行都画同一个键盘图标。工具栏子菜单只有八项，表情默认开（共享默认值是关），没有切换方案、手写、语音，也没有 macOS 齿轮菜单里的宣传、官网、帮助、关于和反馈；Fcitx5 没有 Ctrl+Shift+P。
- 其他：按住按键时每次自动重复都响一次按键音（macOS 只响一次）；IBus 组件的图标名 `msime-client` 不存在；升级后 IBus 什么都不说，Fcitx5 只在辅助行里给一条命令。

## Decision

文案、顺序和行为照 macOS 改，视觉风格不动（产品决定：共享页面和候选窗的「系统」主题在 Linux 上保留 GNOME/Adwaita 的样子）。

切换提示（`platforms/linux/src/overlay/ModeBadge*`）：

- 尺寸与 macOS `InputModeHUDPanel.mm` 相同：18 px 常规字重，32 px 高，左右 8 px 内边距，8 px 圆角；标志跟随 `show_app_logo`，默认不画。显示 0.6 秒后分四帧在 0.18 秒内淡出（`ModeBadgeTiming`）。有合成器时画一圈阴影并淡出；没有合成器的 X11 上透明像素会变黑，所以不画阴影、到时直接收起。
- 一次切换只出一处提示（`ModeBadgePlacement.h` 的 `mode_badge_hint`）：X11 客户端（Fcitx5 看输入上下文的 display，IBus 看会话里没有 `WAYLAND_DISPLAY`）时，徽章按 `MSIMEInputModeHUDFrame` 的规则放在光标下方 10 px，下面放不下就翻到上方，夹在工作区 8 px 以内，没有光标时放在下沿离底边四分之一高的位置，不再发面板提示。其余情况由面板的文字提示跟随光标；只有用户要了标志时，带标志的徽章另放在屏幕角上，因为面板提示带不了图。
- IBus 也链接同一套后端，有 X11 或 layer-shell 时画徽章，否则保留辅助文本。辅助文本的显示时间也改成 0.6 秒。

菜单（`platforms/linux/src/core/MenuLabels.h` 是两个前端共用的文案）：

- 顶层顺序与 macOS `InputController.mm` 的 `menu` 相同：中文输入、英文候选模式（Ctrl+Shift+E）；繁体输出、全角字符（Ctrl+Shift+Space）、中文标点（Ctrl+.）、显示译文；输入方案（当前方案）、主题（当前主题）；显示工具栏和工具栏子菜单、水杉表情面板…、云剪贴板…、云词库…、水杉屏幕键盘…、手写输入…、开始/结束语音输入；水杉输入法设置…、关于水杉输入法…。之后是 Linux 独有的词库…、候选操作、九键、剪贴板、三个选项子菜单和「桌面工具」。
- 方案名与 macOS 相同（双拼（小鹤）、五笔 86、日语、韩语、越南语）。Fcitx5 的方案子菜单按 Engine 顺序排成一组；IBus 保留「中文」一行和两组单选，两组内部按 Engine 顺序。
- Fcitx5 的状态区由 `arrangeStatusArea` 按顺序重新 addAction 一遍（已在的动作会移到末尾），工具栏和语音两项要等会话建好才定，`refreshToolbar` 和 `syncVoiceAction` 会再排一次。只出现在子菜单里的动作不带图标。
- 工具栏子菜单的项目、顺序和默认值与 macOS 悬浮工具栏相同，后面接齿轮菜单的宣传、官网、帮助、关于和反馈；`host_surface.rs` 对 Linux 报 `floating_toolbar_input_scheme`、`floating_toolbar_voice`、`floating_toolbar_handwriting`，设置页给这三个开关。Fcitx5 也处理 Ctrl+Shift+P。

其他：

- 模式字加「双」「五」，IBus 面板符号改为「中」「英」（`InputModeIndicator.h`）。「⇪」仍保留，这是 Windows 语言栏的规则，macOS 没有。
- 自动重复不调 `msime_client_key`，与 macOS 的 `!event.isARepeat`（`InputController.mm` 里 `reportKeyToLifecycle:` 的调用处）相同：不出按键音、不画特效，重复引起的派发算在第一次按下的那一轮里，这是 `msime_client.h` 顺序契约里「不开新一轮的按下」那条过渡期例外，macOS 早就这样用。
- IBus 组件图标改为 `msime-linux`，引擎描述是产品名；Fcitx5 条目和桌面入口加 `zh_TW`、`zh_HK` 名称。
- 升级后：IBus 启动器换上新版本后经 `msime-linux-first-run-guide --reason upgraded` 发「水杉输入法已更新」通知；Fcitx5 插件发「水杉输入法已升级」通知，带「立即重启 Fcitx5」按钮。

历史检索：[macOS 紧凑切换提示](../bug-fix/2026-10-14-macos-compact-input-mode-hud.md)定下了这里照搬的尺寸，部分重叠，互链；[Linux 设置页把工具栏写成输入法菜单](../bug-fix/2026-10-09-linux-toolbar-settings-menu-copy.md)说明工具栏在 Linux 上是菜单，这里没有改，只是让它的项目和默认值跟 macOS 一致，并打开那篇说过不动的三个能力位；[Windows 托盘菜单对齐 macOS](2026-10-10-windows-tray-menu-mac-parity.md)是同一件事在 Windows 上的做法，部分重叠；[Fcitx5 模式提示补宽](../bug-fix/2026-10-07-fcitx5-mode-hint-width.md)的补宽在面板提示那条路上照旧生效。

## Alternatives considered

- **IBus 和 Fcitx5 也做成「中文输入」「英文输入」一对单选** — 与 macOS 一模一样。没有采用：IBus 的 `InputMode` 属性同时是面板读 中/英 符号的那个属性，ibus-ui-gtk3、GNOME Shell 和测试都按开关协议激活它（CHECKED/UNCHECKED）；拆成单选要么改这套协议，要么多一个隐藏的同步属性。一个勾选即中文的开关表达的是同一个状态。
- **IBus 方案菜单去掉「中文」一行，像 macOS 一样做成一个单选列表** — 与 macOS 一致，也少一行。没有采用：「中文」这一行的作用是回到上次用的中文方案，去掉它要先决定这个入口放哪里，这是产品行为，不在这次范围里。
- **Wayland 上也只显示徽章，放弃面板提示** — 一处提示，有图。没有采用：layer-shell 拿不到光标的屏幕坐标，徽章只能放在角上，离用户正在看的地方很远；面板提示由合成器放在光标旁，更接近 macOS。
- **Fcitx5 状态区也画分隔线** — classicui 和托盘菜单会画出分组。没有采用：kimpanel 把状态动作原样列出，分隔线在那里是空白项，KDE 用户的状态区会多出几个空按钮。
- **按模式出单色托盘图标（`*-symbolic.svg`）** — 与 macOS 的每模式图标一致。这次没有做：仓库里没有能把字形转成路径的工具，需要新引入 fontTools 之类的生成工具和字体来源记录，按规则先报告，不在这里加。

## Consequences

- **收益**：X11 上的切换提示与 macOS 位置、大小、时长一致，一次只出一处；两个前端的菜单顺序、名字和快捷键提示与 macOS 相同，文案只有一份；工具栏子菜单的内容和默认值与 macOS 工具栏相同；按住退格不再连响。
- **代价与已知上限**：
  - Wayland 原生客户端仍看不到跟着光标的徽章，只有面板文字提示；GNOME Wayland 下 IBus 的徽章（打开标志时）经 Xwayland 显示在角上。
  - IBus 的徽章只在纯 X11 会话里跟随光标：同一个引擎服务 Wayland 原生客户端时分不出坐标系，宁可用辅助文本。
  - Fcitx5 顶层从 24 项变成 32 项（失焦时 23 项），kimpanel 的按钮更多。
  - 焦点进入新输入框时 Linux 仍显示提示（#2589），macOS 只在切换时显示，这是有意保留的差异。
  - GNOME Shell 的 IBus 菜单仍只有两项（Shell 的属性树会卡死桌面），每模式图标、「检查更新…」入口和设置页的「已更新到 X」状态卡都没有做。
  - 升级通知的按钮需要 libnotify 0.7.10 起的 `notify-send --action`；带按钮的通知会让一个 `notify-send` 等到用户关掉它。

## Verification

- `bash platforms/linux/build-container.sh`（Debian 容器里编译 IBus 宿主、Fcitx5 插件和全部单测并跑 ctest，96 项全过）：`linux-mode-badge-style`（尺寸、淡出帧、光标定位、提示规则）、`linux-mode-badge-render`（用 cairo 和 pango 实画徽章，检查本体、描边、阴影边和半透明淡出）、`linux-wave-overlay-placement`、`linux-menu-labels`、`linux-input-mode-indicator`、`linux-wubi-profile-names`、`ibus-mode-hint-*`、`linux-fcitx5-contract`、`linux-ibus-component-contract`、`linux-settings-launcher-contract`、`linux-first-run-guide`。
- `cargo test -p msime-client-core --lib host_surface` 钉住三个工具栏能力位。
- 菜单顺序和方案名由 `tests/runtime/engine_smoke.cpp`、`fcitx5/tests/native.cpp` 钉住，它们只在 `tests/tools/check-container.sh` 的验收容器里跑（要已校验的词库）。写这篇时 develop 上这两个测试本身就过不去：`ibus-engine-smoke` 在分支的基线提交上同样停在「IBus did not keep an input-method ASCII digit as smart-punctuation context」，Fcitx5 的几个 native 模式停在「Disabled expression mode must pass through」一类已关闭的快捷模式仍吃掉 Shift 组合键的断言。把这几条基线就失败的断言改成只打印之后，`ibus-engine-smoke` 全部通过（含顶层顺序、方案菜单的各项激活）；`fcitx5-native-context` 走过了状态区数量、顺序、文案和选项子菜单的全部检查，停在更后面与菜单无关的翻译 socket 用例上。
- 徽章在 Fcitx5 验收镜像里的真实 GTK 编辑器、classicui、原生画布和 Wayland 画布用例里都画得出来、不挡输入（`fcitx5-gtk-editor`、`fcitx5-*-canvas-*` 通过；`fcitx5-native-canvas-vertical` 在一次整轮运行里因测试自己读词库的竞态失败，单独重跑通过）。还没有在真实桌面上看过：GNOME、KDE 的托盘菜单和 IBus 面板的实际样子、X11 徽章跟随光标的位置都只经过单测和容器。
