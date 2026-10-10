# Agent Note: Windows 托盘菜单补齐 macOS 输入菜单的行、翻页与键盘操作

Status: implemented

## Problem

macOS 输入菜单有「英文候选模式」（⌃⇧E）、「繁体输出」、「云剪贴板…」，「输入方案」和「主题」是两个子菜单，主题可以就地切换；标点被 `punctuation_lock` 钉住时「中文标点」变灰；NSMenu 自带方向键、回车、Esc。Windows 的托盘卡片（`platforms/windows/src/candidate/TrayMenu*`）没有前三行，「主题」只打开设置应用的主题页，方案是一整组十行单选，标点锁定时那一行照样能点，卡片不抢焦点所以收不到键盘消息，完全不能用键盘操作。云剪贴板在 Windows 上只能从设置应用打开，那时前台是设置窗口，面板记不下用户的编辑器，只能复制、不能输入。

## Decision

卡片的行由 `tray_menu_items(capabilities, state, page)` 决定，分三页（`TrayMenuPage`）。主页依次是：中文、英文、英文候选模式；繁体输出、全角字符、中文标点、显示译文；输入方案、主题两个翻页行（右侧写当前选择，画向右箭头）；工具条；云剪贴板…、词库…、设置…、关于。方案页是返回行加 `tray_menu_scheme_items`（与悬浮工具栏的方案菜单同一组行），主题页是返回行、主题目录的各行（行的 `value` 是主题 id）和「主题设置…」。翻页只在 `TrayMenuWindow` 里处理，按上次的锚点重新摆放卡片，从不交给 Server；每次打开都从主页开始。

- **英文候选模式**：可用与否由 `tray_menu_dedicated_english_action` 判定，规则与 TIP 的 Ctrl+Shift+E 相同——TIP 报告中文、方案不是在 TIP 宿主会话里组字的韩文/注音/越南文/藏文。点击后 Server 用新的单槽工作线程调用 `SessionController::set_dedicated_english(lease, enabled)`（经 `InputState`、`FocusedSession` 到 `ServerSession::set_dedicated_english`），成功后立刻发布到 `DedicatedEnglishMailbox`，下一轮循环按原有路径用 DedicatedEnglishChanged 推给 TIP。托盘点击只发一次、没有重试，所以 `set_dedicated_english` 不像别的外部请求那样拿不到事务锁就放弃，而是等锁（每 5 毫秒试一次，Server 停止时放弃），等锁和等输入队列共用 2 秒；250 毫秒一次的英文模式读取、按键、候选点击都会短暂占着这把锁，放弃会让开关偶尔点了没反应。焦点会话正在组字或列着候选时不切换，因为托盘点击不经过 TIP，Engine 丢掉组字而 TIP 的组字还留在编辑器里会让两边对不上。
- **繁体输出**：提交给工具栏简繁按钮和 Ctrl+Shift+F 共用的 `character_set_clicks`，勾来自同一个 `traditional_output`；`toggle_character_set_ctrl_shift_f` 开着时旁边写出 Ctrl + Shift + F。
- **中文标点**：`TrayMenuPreferences` 读 `punctuation_lock`，是 chinese 或 english 时这一行不可用。
- **主题**：主题页的行写存储的 `global_theme`（`store_global_theme`，只换这一项，与 macOS 的 `selectGlobalTheme:` 相同），只接受 `theme_catalog_entries` 从 `msime_client_theme_catalog` 读出的 id。
- **云剪贴板…**：`shell_surface_request` 映射到 `cloud-clipboard` 路由，由 MSIME.exe 打开。卡片不抢焦点，Tauri 打开面板时 `remember_opening_panel_target` 记下的前台窗口就是用户的编辑器，`send_cloud_clipboard_text_windows` 能把条目输入回去。打开前用 `focused_field_kind`（`platforms/windows/src/system/SecureFieldProbe.h`）看焦点控件：Win32 编辑类控件带 ES_PASSWORD，或 UI Automation 报告 IsPassword，就响一声、卡片留着、不打开。UI Automation 的跨进程调用放在临时 MTA 线程上，UI 线程最多等 250 毫秒，没回答按未知处理、照常打开。
- **键盘**：卡片可见时 `TrayMenuWindow` 装一个 `WH_KEYBOARD_LL` 钩子，隐藏时卸下，同一时刻只属于一张卡片（悬浮工具栏的两个弹出菜单也是 `TrayMenuWindow`，同样受益）。按键映射和结果是纯函数 `tray_menu_key` / `tray_menu_key_result` / `tray_menu_page_highlight`：↑↓、Home/End 移动高亮，回车/空格执行高亮的行（没有高亮时收起卡片、交给应用，不吞掉用户接着打的空格），→ 打开翻页行，← 与 Esc 在子页上返回，Esc 在主页上收起，工具条里 ←→ 在几格之间移动；导航键连同它的松开都被吞下，别的键和带 Shift/Ctrl/Alt/Win 的组合收起卡片并照常交给应用，单按修饰键和模拟的按键（`LLKHF_INJECTED`）不算。Shift 也算修饰键：吞掉 Shift+↓ 的 ↓ 时 TIP 只看到 Shift 的按下和松开，会把它当成单按 Shift 切换中英文；原生菜单里 Shift+方向键也不是菜单导航。钩子只在卡片真的显示出来之后才装，卡片看不见时钩子把按下原样交给应用。卡片的尺寸按它要去的那块显示器的 DPI 算（`monitor_effective_dpi`），挪到 DPI 不同的显示器时同步收到的 WM_DPICHANGED 在摆放期间不收起卡片，否则工具栏在缩放不同的副屏上时弹出菜单第一次打不开，钩子却已经装上、吞掉所有程序里的导航键。钩子回调只投递一条消息，执行和重画在卡片自己的窗口过程里做。吞下的按下记在一张按键表里，它的松开也吞掉；交给应用的按下会清掉表里那一项，新装钩子时整张表清空，免得上一张卡片收起前没松开的键让下一次松开被吞、应用只收到按下。UI 循环把 `keyboard_activity()` 当作指针活动（托盘卡片和工具栏的两个弹出菜单都是），键盘用户的指针停在托盘图标或工具栏按钮上也不会被 5 秒闲置收起。

## Alternatives considered

- **方案和主题仍内联在主页里** — 一眼就能看到全部方案，少点一下，也是原来的设计。但原来十个方案的卡片在 150% 缩放的 1080p 工作区里已经要靠压缩行高才放得下，再加英文候选模式、繁体输出、云剪贴板三行，压到最小行高 28 DIP 也放不下，最后几行会被裁掉；主题目录再内联更不可能。macOS 自己也因为同样的高度问题把两组改成了子菜单。
- **做悬停展开的二级卡片** — 最像原生子菜单。但卡片不抢焦点，靠 UI 循环轮询指针收起，两张卡片之间的指针移动、各自的闲置计时和收起时机都要另写一套规则，误收起的情况更多；整张卡片换页只要一个页状态和一次重新摆放。
- **用 `TrackPopupMenu` 或 TSF 语言栏的 `InitMenu` 提供原生菜单** — 键盘和读屏都是现成的。但原生菜单要求所属窗口能激活，激活就把焦点从正在打字的应用抢走，模式行就落到错误的焦点上，云剪贴板也记不下用户的编辑器；语言栏菜单属于 TIP，不在这次范围里。
- **英文候选模式在 TIP 英文状态下也可点，先切中文再进入** — 与 macOS 完全一致（那边从英文输入点它会直接进入）。但 Windows 的 Ctrl+Shift+E 在 TIP 英文状态下本来就不起作用，先发 Chinese 模式命令、再在 Engine 上设英文模式是两步异步操作，TIP 的开关状态回报和 Engine 模式之间的先后没有保证；行显示为不可用更诚实。
- **托盘点击时取消组字再切换英文模式（与 Ctrl+Shift+E 一样）** — 用户正在组字时点击也能生效。但 Ctrl+Shift+E 是 TIP 先在本地取消组字再告诉 Server，托盘点击 TIP 不知道，Server 单方面取消会留下 TIP 侧的组字；组字中不切换没有这个风险。
- **只看 Win32 的 ES_PASSWORD 判密码框** — 零跨进程调用，不会等。但浏览器、WinUI、WPF 的密码框都不是 Win32 编辑控件，只有 UI Automation 的 IsPassword 能认出来。

## Consequences

- **收益**：托盘里能切英文候选模式、简繁输出和主题，能打开带着当前编辑器的云剪贴板，密码框里不会把云端历史列出来；卡片在 150% 缩放的 1080p 屏幕上按设计尺寸放得下；卡片和工具栏的两个弹出菜单都能用键盘操作。
- **代价与已知上限**：
  - 切方案从一次点击变成两次（先点「输入方案」翻页）；悬浮工具栏的方案按钮仍是一次。
  - 英文候选模式在 TIP 英文状态下不可用，与 macOS 不同；组字中点击什么也不做（卡片照常收起）。
  - 低级键盘钩子只在卡片可见时（最长 30 秒）装着，但这段时间里系统所有按键都要等 UI 线程处理钩子回调；UI 线程卡住超过 `LowLevelHooksTimeout` 时按键会延迟，Windows 也可能在多次超时后静默移除钩子，之后这次打开就没有键盘导航。
  - 读屏支持（`win-tray-keyboard-navigation` 的另一半）不在这里，见 [自绘窗口的 UI Automation 读屏](2026-10-10-windows-server-window-ui-automation.md)。
  - 密码框判断依赖目标程序如实报告 IsPassword；没有在 250 毫秒内回答时照常打开（只能复制时面板不会把内容输入别处）。每次点击（Win32 样式位没认出密码框时）都起一个短命线程，目标程序挂起时它会等到 UI Automation 自己超时才退出。
  - 主题页只写 `global_theme`，和 macOS 一样不动 `custom_theme`；从托盘选「自定义」会用上次留下的自定义主题。

## Verification

`platforms/windows/tests/ui/tray_menu_layout.cpp`（三页的行和顺序、英文候选模式与繁体输出的可用性和勾、标点锁、主题页上限、1080p 150% 放得下、键盘映射与导航结果）、`tests/ui/shell_surfaces.cpp`（cloud-clipboard 路由、`SecureFieldPolicy.h` 的判定）、`tests/ui/candidate_theme_reload.cpp`（`theme_catalog_entries`）、`tests/input/dedicated_english.cpp` 与 `tests/input/dedicated_english_controller.cpp`（`set_dedicated_english` 在组字中拒绝、过期租约不动、读回新状态、不写 TIP）。真机要确认：托盘卡片里点「云剪贴板…」后面板能把条目输入回原编辑器；Chrome/Edge 和 Win32 密码框里被拒；钩子装着时打字没有可感知的延迟；Ctrl+Shift+E 与托盘行切换后 TIP 的字母分类一致。

相关：[Windows 悬浮工具栏补齐 macOS 的按钮、菜单与行为](2026-10-10-windows-floating-toolbar-mac-parity.md)（方案菜单与托盘方案页共用 `tray_menu_scheme_items`，两个弹出菜单现在也有键盘导航）。
