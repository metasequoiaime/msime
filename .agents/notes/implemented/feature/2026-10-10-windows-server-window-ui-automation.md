# Agent Note: Windows 自绘窗口（候选窗、悬浮状态栏、托盘卡片）的 UI Automation 读屏

Status: implemented

## Problem

macOS 上读屏（VoiceOver）能读出候选窗的每一行（`InputController.mm` 给每个候选、页码、翻页箭头、预编辑和 logo 设了 accessibilityLabel），悬浮工具栏每个按钮的名字随状态变化（`FloatingToolbarPanel.mm`），输入菜单是原生 NSMenu，自带方向键和读屏。Windows 的这三处都由 Server 用 Direct2D 自己画在 `WS_EX_NOACTIVATE` 窗口上，没有子控件，也没有处理 `WM_GETOBJECT`：讲述人、NVDA 只看到一个没有名字的窗口。候选本身虽能通过 TSF 的 `ITfCandidateListUIElement` 读到，但翻页箭头、页码、预编辑读不到；工具栏只有第一波加的 tooltip；托盘卡片第一波加了键盘导航，读屏却读不出任何一行。

## Decision

三个窗口共用一个 UI Automation 提供者 `AccessibleWindow`（`platforms/windows/src/candidate/AccessibleWindow.{h,cpp}`）。窗口本身是片段根（`IRawElementProviderSimple` + `IRawElementProviderFragment` + `IRawElementProviderFragmentRoot`，位置和 RuntimeId 由 `UiaHostProviderFromHwnd` 给出），每个元素是一个子片段（另带 `IInvokeProvider`、`IToggleProvider`、只读的 `IValueProvider`，按元素需要通过 `GetPatternProvider` 交出）。窗口在 `WM_GETOBJECT` 里调用 `answer()`，只回答 `UiaRootObjectId`，交给 `UiaReturnRawElementProvider`。

窗口与提供者之间的契约是一棵纯数据的树 `AccessibleTree`（`AccessibleElements.h`）：元素有 id、种类、AutomationId、名字、补充说明、快捷键、值、客户区像素坐标、可用、可执行、勾选和键盘焦点。每个窗口按刚画好的布局用一个纯函数算出这棵树，再 `publish()`：

- **候选窗**：`candidate_accessible_tree`（`CandidateAccessibility.h`），在 `paint()` 记下 `painted_rows_` 之后发布。元素自上而下是 logo（产品名）、预编辑（名字「候选窗预编辑」，拼音是它的值，拼音为空时没有）、页码「第 N 页，共 M 页」、「上一页候选」「下一页候选」两个按钮、每行候选（名字「序号  候选」，后面跟注释或훈음，补充说明是悬停提示那段全文和释义）。上一页在第一页不可用；下一页总可用，与画出来、点下去的规则相同。只能用键盘选的列表（`pointer_input` 为 false）名字照读，但不可执行。
- **悬浮状态栏**：`toolbar_accessible_tree`（`ToolbarAccessibility.h`），在 `paint()` 末尾发布。按钮名就是悬停提示 `toolbar_tooltip`，与 macOS `accessibilityLabel == toolTip` 一致；AutomationId 沿用 macOS 的 `MetasequoiaFloatingToolbar*`（隐藏按钮 macOS 没有，用 `MetasequoiaFloatingToolbarHide`）；logo 打开时有一个以产品名为名的图片。语言按钮的名字带方案名，`set_scheme_title` 在方案名变了时重画工具栏，只换双拼、五笔键位时读屏读到的也是新方案名。读屏执行按钮走与鼠标点击同一个 `run()`（从 `WM_LBUTTONUP` 抽出），要求按钮画出来时的焦点租约仍是当前的，并重置空闲隐藏计时。
- **托盘卡片和工具栏的两个弹出菜单**（都是 `TrayMenuWindow`）：`tray_menu_accessible_tree`（`TrayMenuAccessibility.h`），在打开、翻页、执行后刷新、键盘移动高亮时发布。菜单以标题行的产品名为名，每行是菜单项；只有勾上的行报告 Toggle（和原生菜单一样，「设置…」这类行没有勾选这回事），工具条里的开关格开关都报告；快捷键归 AcceleratorKey，翻页行右侧的当前选择归补充说明。键盘导航移动高亮时那一行标为焦点并发 `UIA_AutomationFocusChangedEventId`；鼠标悬停不算焦点，免得读屏跟着指针抢读。卡片从隐藏到显示发 `UIA_MenuOpenedEventId`，收起前发 `UIA_MenuClosedEventId`。
- **托盘卡片的键盘钩子让出读屏键**：讲述人和 NVDA 的读屏键是 Caps Lock 或 Insert。读屏键单按或按着时的按键（`tray_menu_screen_reader_key`，`TrayMenuAccessibility.h`）原样交给读屏，卡片 never 拿它导航、也不因它收起，并算作在用卡片（刷新 `keyboard_at_`，不按闲置收起）。读屏自己的钩子会吞掉读屏键，`GetAsyncKeyState` 看不到它按着，所以钩子按自己看到的按下、松开记录 Caps Lock 和 Insert 的状态，再与 `GetAsyncKeyState` 取或。没有这条时，NVDA 用户每按一次 Insert 卡片就收起，讲述人键加方向键被卡片当成导航吞掉。

### 线程与生命周期

- `publish`、`focus`、`menu_opened`、`menu_closed`、`disconnect` 只在窗口的 UI 线程上调用。提供者对象可能在 UI Automation 的线程上被调用：它们与窗口共用一个 `AccessibleShared`（互斥锁、窗口句柄、最近一次发布的树、序号），只在锁里读树；`UiaReturnRawElementProvider`、各种 `UiaRaise*` 和窗口位置的量取都在锁外，提供者从不碰窗口的其他成员。
- 读屏要执行一个元素时，提供者 must 把 `accessible_invoke_message`（wparam 是元素 id，lparam 是树的序号）投递回窗口，由窗口在 UI 线程上用 `AccessibleWindow::current` 核对序号后执行；树变过（换页、换了快照、焦点移动）的旧请求直接丢掉，never 按旧树里的位置点到新树里的另一样东西。
- 树和上一棵完全相同时 `publish` 什么也不做；变了才在有读屏在听（`UiaClientsAreListening`）且已经交出过根时发 `StructureChangeType_ChildrenInvalidated`。候选窗每次打字闪光都会重画，这样不会产生事件。
- 子元素提供者按 id 缓存在 `AccessibleShared` 里（每个窗口只有一小组固定 id），同一元素总是同一个对象。窗口析构时先 `disconnect()`（`WM_DESTROY` 里也调用一次，幂等）：对每个交出去的提供者 `UiaDisconnectProvider`，再 `UiaReturnRawElementProvider(hwnd, 0, 0, nullptr)`；之后仍被读屏拿着的提供者一律回答 `UIA_E_ELEMENTNOTAVAILABLE`。
- 坐标：Server 进程不感知 DPI，候选窗和托盘卡片按每显示器感知建立，悬浮状态栏不是。元素坐标在窗口自己的客户区坐标系里，提供者按「窗口在每显示器感知下的屏幕矩形 / 窗口自己坐标系里的客户区大小」换成读屏要的物理像素（`accessible_frame`），与调用线程的 DPI 上下文无关。

### 构建

`AccessibleWindow.cpp` 进 `msime-windows-server`，链接 `uiautomationcore` 和 `oleaut32`。MinGW-w64 12（CI 的 Debian trixie）自带 `uiautomation.h` 和 `libuiautomationcore.a`，所用函数都在其中，不需要自己声明接口；代码只用 Win32/COM，MSVC 同样编译。

### TSF 语言栏的 InitMenu 不填

`tsf/LanguageBar/LanguageBar.cpp` 的 `InitMenu`/`OnMenuSelect` 是空的，但语言栏项的样式是 `TF_LBI_STYLE_BTN_BUTTON`，TSF 只对 `TF_LBI_STYLE_BTN_MENU` 调用 `InitMenu`，所以它们从不被调用。右键已经交给 Server 打开托盘卡片（`SendLangbarRightClickEventToUIProcess`），而托盘卡片就是 macOS 输入菜单在 Windows 上的对应物，现在有了键盘导航和读屏。

## Alternatives considered

- **填 TSF 语言栏的 `InitMenu`，用原生菜单当无障碍替代** — 原生菜单的键盘和读屏都是现成的，一行 COM 也不用写。但要把语言栏项改成 `TF_LBI_STYLE_BTN_MENU`，左键就不再切中英文；菜单由 TIP 在应用进程里建，行的状态要从 Server 取，和托盘卡片是两套行；托盘卡片仍然读不出来，候选窗和工具栏也没解决。
- **每个窗口各写一份提供者** — 各窗口可以按自己的数据结构直接回答，不需要中间的树。但三份 COM 样板、三份线程与断开逻辑，一处修了另两处容易漏；中间的树还让名字、坐标、可执行的规则能在宿主机上测试。
- **只给工具栏按钮设 tooltip、不做 UIA（第一波的状态）** — 改动最小。但 tooltip 是悬停才出现的弹出窗口，读屏不会把它当成按钮名；讲述人对没有提供者的自绘窗口只读窗口标题。
- **用 MSAA（`IAccessible`）代替 UI Automation** — 老读屏都支持。但 `IAccessible` 的子元素靠 VARIANT 子 id，没有片段树和 RuntimeId，名字、坐标、执行都要另一套写法；讲述人和新版 NVDA 优先走 UI Automation。只问 MSAA（`OBJID_CLIENT`）的老客户端仍只看到 DefWindowProc 给的窗口本身。
- **候选高亮变化时发焦点事件** — 读屏会随选词读出当前候选。但候选已经通过 TSF 的 `ITfCandidateListUIElement` 读给讲述人，再发焦点事件会让同一个候选被读两遍，还可能把读屏的焦点从正在打字的编辑框拉走。

## Consequences

- **收益**：读屏能读出候选窗的每一行、页码、翻页箭头和预编辑，也能执行它们；工具栏每个按钮有随状态变化的名字，可以执行；托盘卡片和工具栏弹出菜单的每一行可读、可执行，键盘导航时读屏跟着高亮读出当前行，菜单打开和收起会被播报。名字与 macOS 一致。
- **代价与已知上限**：
  - 没有在真实的 Windows 和讲述人、NVDA 上验证过：提供者只在 MinGW 下做了语法检查，规则由宿主机上的纯函数测试覆盖。最需要在真机上确认的是：`WS_EX_NOACTIVATE` 的托盘卡片发的焦点事件讲述人是否跟随；`StructureChangeType_ChildrenInvalidated` 传空 RuntimeId 时客户端是否刷新子元素；不感知 DPI 的工具栏在 150% 缩放下的元素矩形是否与画面重合。
  - 读屏键要靠卡片的钩子先于读屏的钩子看到才记得住；卡片打开前就按着的读屏键只能靠 `GetAsyncKeyState`，被读屏吞掉时这一下仍按卡片的规则走，需要在真机上确认。
  - 读屏只读不按键时，托盘卡片照常按 5 秒闲置收起；读屏执行一行、按读屏键的命令都算作键盘活动。
  - 树变了旧的执行请求就被丢掉，所以读屏执行的那一刻如果卡片刚好重新发布（比如键盘移动了高亮），这次执行什么也不做，需要再执行一次。
  - 工具栏两个弹出菜单没有标题行，菜单名字为空，读屏只说「菜单」。

## Verification

- `platforms/windows/tests/ui/accessible_elements.cpp`（`windows-accessible-elements`）：坐标换算和命中、悬浮状态栏的元素（名字等于悬停提示、AutomationId、logo、不可用按钮）、托盘卡片的元素（菜单名、勾选、快捷键与补充说明、键盘焦点只落在可执行的行上、行与几何对不上时为空树），以及读屏键让给读屏的判定。
- `platforms/windows/tests/candidate/candidate_accessibility.cpp`（`windows-candidate-accessibility`）：候选窗各元素的名字、顺序、坐标，第一页的上一页不可用，只能键盘选的列表不可执行，logo、预编辑、翻页缺席时的树。
- 两个测试在宿主机上用 `clang++ -std=c++17 -Wall -Wextra -Werror` 编译并通过；`AccessibleWindow.cpp`、三个窗口和两个测试在 x86_64 与 i686 MinGW-w64 下 `-fsyntax-only -Wall -Wextra -Werror` 通过。
- 相关笔记：[候选窗补齐 macOS](2026-10-10-windows-candidate-window-mac-parity.md)、[悬浮工具栏补齐 macOS](2026-10-10-windows-floating-toolbar-mac-parity.md)、[托盘菜单补齐 macOS](2026-10-10-windows-tray-menu-mac-parity.md)。
