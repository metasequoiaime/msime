# Agent Note: Windows 悬浮工具栏补齐 macOS 的按钮、菜单与行为

Status: implemented

## Problem

macOS 的悬浮工具栏有切换输入方案、手写、语音三个可选按钮，设置按钮右键有实用菜单（表情与符号、检查更新、官网、帮助、关于、反馈、隐藏），语言按钮在双拼和五笔下显示「双」「五」，每个按钮带悬停提示，`show_app_logo` 关掉时 logo 换成握把，连续 10 秒没有键盘输入自动隐藏，拖动后的位置跨重启保留，共享应用不在时表情退回系统字符面板。Windows 的工具栏只有中英、全角、标点、简繁、表情、屏幕键盘、设置七个按钮：方案只能从托盘切换，手写和语音只在托盘里，隐藏动作写了却没有入口，生产 Server 的拖动位置重启就丢，logo 一直画，没有提示，没有共享应用时表情和屏幕键盘按钮直接变灰。`HostCapabilities` 的 `floating_toolbar_handwriting` / `floating_toolbar_voice` / `floating_toolbar_input_scheme` 因此只对 macOS 报 true，Windows 用户在设置里也看不到这三个开关。

## Decision

工具栏的按钮组、默认值与 macOS 和 client-core 的 `FloatingToolbarPreferences` 一致。`FloatingToolbarSettings`（`platforms/windows/src/candidate/FloatingToolbarSettings.h`）在原有六项 `items` 之外另读 `input_scheme`（默认开）、`handwriting`、`voice`（默认关）和顶层 `show_app_logo`（默认关）。六项 `items` 不扩成十项：它同时是 PreviewConfig `floating_toolbar_items` 的契约，那里要求正好六个键。`floating_toolbar_slots` 按「中英、切换输入方案(11)、全角、标点、简繁、表情、手写(7)、屏幕键盘、语音(8)、设置」排，手写还按版本（`MSIME_EDITION_HANDWRITING`）去掉。手写和语音沿用它们原来的 id 7、8，9（关于）不再分配。

切换输入方案按钮和右键实用菜单都用托盘卡片的 `TrayMenuWindow` 画，新增按行构造的构造函数和 `open_beside`（`toolbar_menu_bounds`：上方放得下开在工具栏上方，否则开在下方）。方案菜单的行就是托盘「输入方案」那一组（`tray_menu_scheme_items`，托盘也从这里取），选中后走 Server 里托盘同一个 `tray_command`，也就是 `store_input_scheme` 写偏好；实用菜单的行由 `toolbar_utility_menu_items` 给出，照搬 macOS 的 `CreateMetasequoiaFloatingToolbarUtilityMenu`。右键在卡片任何位置都打开实用菜单，不只设置按钮：设置按钮能被关掉，隐藏工具栏那一项不能跟着丢。「隐藏悬浮状态栏」写回 `floating_toolbar.enabled`，与托盘的工具栏开关同一条路径；「检查更新…」和「关于」一样打开设置窗口的「关于」页（`settings:about`），检查在那一页里经 `msime_client_update_check` 完成（合入 develop 的「设置窗口关于页直接检查更新」之后，原先直接让 MSIME.exe 打开共享应用关于页的做法随之改掉），没有设置窗口时这一行不可用；帮助和反馈落到设置窗口的「帮助与反馈」页。两个弹出菜单与托盘卡片一样不抢焦点，由 UI 循环用 `tray_menu_dismissal` 轮询收起；被轮询收起后 400 ms 内同一按钮不再重开，所以再按一次按钮就是关上它。工具栏窗口不感知 DPI，它量到的按钮位置是系统虚拟化过的逻辑坐标，而弹出菜单按物理像素摆放，所以锚点先按同一个窗口在两种坐标里的外框换算（`toolbar_physical_coordinate`），否则缩放不是 100% 时菜单开到离按钮很远的地方。

语音按钮调用 `VoiceInputSession::toggle()`，与托盘语音行和语音快捷键的切换消息相同。手写按钮经 `shell_surface_request(OpenHandwritingPanel)` 打开共享应用的手写面板。表情和屏幕键盘先找共享应用，起不来时打开系统表情面板（`SendInput` 合成 Win+.）和 `osk.exe`（`platforms/windows/src/system/SystemPanels.h`），托盘的表情行和键盘行同样如此，所以这两处入口不再因为缺共享应用而变灰；只有设置按钮（缺设置窗口）和手写按钮（缺共享应用）还会画成不可用。

语言按钮在中文模式下按运行中的方案画「双」「五」（`ToolbarLanguageState::scheme`）。悬停提示用 comctl32 的 tooltip 控件，每个按钮一个区域，文字由 `toolbar_tooltip` 现算，与 macOS 的 toolTip 相同，语言按钮带方案名（`toolbar_scheme_title`，如「小鹤双拼 · 切换到英文输入」）；用户自定义的双拼键位（`custom`）写「自定义双拼」，不按 macOS 的规范化说成小鹤双拼，其余不认识的键位仍按 macOS 落到小鹤。`show_app_logo` 关掉时 logo 槽按 macOS 的比例收窄（图标 24 时 10 DIP），画两列三行圆点，仍是拖动区。

空闲隐藏由 `FloatingToolbarIdleTimer`（`FloatingToolbarVisibilityPolicy.h`）决定：每次真实按键（维护快捷键的低级键盘钩子新增的 `KeySink`，不含注入的按键）、Server 经 TSF 管道收到的按键（`ServerKeyActivity`：焦点会话处理的键和 Aux 管道的 KeySound 各加一次计数，界面线程每轮比较计数，变了就算一次输入；钩子看不到屏幕键盘和 SendInput 类键鼠共享工具注入的按键、看不到发往提权窗口的按键，也可能没装上，这些按键只要经过了 Server 照样唤醒工具栏）、在工具栏上按下鼠标、拖动工具栏松手（系统的移动循环占着 UI 线程，拖动期间计时没有机会重置，松手时补记一次）、弹出菜单开着，都让工具栏回来并重新计时；开关打开或输入法重新激活也算一次输入；全屏切换、偏好刷新和普通焦点切换不唤醒。生产 Server 把拖动后的位置写进状态目录的 `floating_toolbar_position.json`（`FloatingToolbarPosition.h`），启动时读回。移动循环里窗口动了就记下松手时的位置（`floating_toolbar_drag_end`），没记过位置的第一次拖动也算，只在拖动条上按一下没动时不记；越界由原有的放置逻辑夹回工作区；预览实例照旧写自己的配置文件。位置只属于这台机器的显示器布局，不进同步到云端的共享偏好。

WinUI 设置窗口的「工具栏组件」加入切换输入方案、手写识别板（不提供手写的版本不显示）和语音输入；`HostCapabilities::for_platform(Windows)` 的三个能力位翻成 true，手写仍由 `narrow_to_edition` 按版本收窄。

## Alternatives considered

- **把 `items` 扩成十项** — 一个数组装下全部按钮最整齐，开关也只有一处要读。但 `items` 同时是 PreviewConfig `floating_toolbar_items` 的落盘契约，PreviewConfig 要求正好六个键，扩项要同时改启动配置格式和所有预览配置；新按钮只从共享偏好读就够了。
- **用系统 `TrackPopupMenu` 弹方案菜单和实用菜单** — 代码最少，键盘导航、无障碍都现成。但它要求所属窗口能激活，工具栏是 `WS_EX_NOACTIVATE`，激活它会把焦点从正在打字的应用抢走，方案切换就落到错误的焦点上；托盘卡片已经解决了不抢焦点的弹出和收起，并且跟随主题配色。
- **只在设置按钮上挂右键菜单（与 macOS 完全一致）** — 行为最贴近 macOS。但 Windows 的设置按钮可以关掉，关掉后「隐藏悬浮状态栏」就没有入口，正是要修的那个问题。
- **拖动位置写进共享偏好** — 设置页和其他设备都能看到。但坐标只对这台机器的显示器布局有意义，同步到另一台机器会把工具栏放到不存在的屏幕位置；macOS 的 frame autosave 也只存在本机。
- **空闲判断用 `GetLastInputInfo`** — 不用改钩子。但它把鼠标移动也算作输入，macOS 只认键盘，指针掠过就唤醒会让空闲隐藏几乎不起作用。

## Consequences

- **收益**：Windows 用户可以从工具栏切方案、开手写和语音、隐藏工具栏、检查更新；设置页出现三个按钮的开关；没有共享应用时表情和屏幕键盘仍能用；位置跨重启保留。
- **代价与已知上限**：
  - 老用户的共享偏好里 `input_scheme` 缺省为开、`show_app_logo` 缺省为关，升级后工具栏多出方案按钮、logo 换成握把，与 macOS 新装时一样；想要原样的用户在设置里改回。
  - 空闲 10 秒自动隐藏是 Windows 上的新行为，与 macOS 一致但没有单独的开关；用户反馈不需要时要重访，可能加一个偏好。
  - Server 只看得到 TIP 交给它的键：中文模式下组字的字母都经过它；TIP 自己放行给应用的键（英文模式的字母、没有组字时的空格、回车、数字）只在按键音或打字特效开着时经 Aux 管道报 KeySound。两样都关着、这些键又是注入的或发往提权窗口时，钩子和 Server 都看不到，唤不醒工具栏；切回中文打一个字就会唤醒。
  - 系统表情面板靠合成 Win+.，Windows 没有给 Win32 进程的公开接口；系统改了这个快捷键就失效。
  - 提示只是 tooltip 控件；读屏读到的按钮名字（与提示同一段文字）由 UI Automation 提供者给出，见 [自绘窗口的 UI Automation 读屏](2026-10-10-windows-server-window-ui-automation.md)。
  - 任务栏语言栏图标按方案画一个字，Ctrl+Shift+Win+K 也退回 `osk.exe`，见 [遗留项](2026-10-10-windows-mac-parity-leftovers.md)；按方案注册多个 TSF 语言配置文件没有做。
  - 藏文在工具栏仍画「藏」，macOS 画「ཀ」；Windows 的托盘和语言栏都用「藏」，这里保持一致。

## Verification

`platforms/windows/tests/ui/` 下的 `floating_toolbar_reload.cpp`（按钮组、默认值、版本收窄）、`floating_toolbar_visibility.cpp`（空闲计时，含 Server 收到按键时唤醒）、`toolbar_icons.cpp`（新按钮字形、双/五徽标）、`toolbar_tooltips.cpp`、`toolbar_menus.cpp`（方案菜单与托盘一致、实用菜单的行与可用性、菜单位置）、`floating_toolbar_position.cpp`、`toolbar_layout.cpp`（握把尺寸）、`toolbar_coordinates.cpp`（卡片命中）；`crates/client-core/src/host_surface/tests.rs` 断言 Windows 的三个能力位并随版本收窄手写。真机行为（Win+. 面板出现在当前输入框、tooltip 在不抢焦点的窗口上显示、菜单收起手感）要在 Windows 上确认。
