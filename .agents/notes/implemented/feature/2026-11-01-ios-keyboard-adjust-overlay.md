# Agent Note: iOS 键盘调整遮罩取代内联高度条

Status: implemented

## Problem

iOS 键盘里调键盘尺寸的唯一入口是功能菜单的「键盘高度」：工具栏位置换成一条内联调节条（`InlineHeightBar`），只能上下拖动改高度，取消、重置、完成三个按钮，收起键盘按取消处理（见 [iOS 功能菜单的「键盘高度」与 Android 一样打开内联高度条](2026-10-08-ios-keyboard-height-tile.md)）。用户要的是 Gboard 那种整块键盘的调整界面：除了高度，还能把键盘抬离屏幕底边、收窄并左右挪动，所见即所得（目标效果截图：遮罩、四角括号、四个拖动柄、中间「重置 / 移动 / 确认」）。Android 只在设置页有「键盘底部高度」滑块，并在 [Android 恢复单一的「键盘底部高度」滑块](2026-11-01-android-keyboard-bottom-height-slider.md) 里把「内联调整条加下沿拖动柄」留给以后；鸿蒙两者都没有。三端由三个人同时按同一份契约实现，界面和行为必须一致。

## Decision

功能菜单的「键盘高度」改名「键盘调整」（图标不变，工具 id 改为 `keyboardAdjust`），点了打开 `KeyboardAdjustOverlay`，旧的 `InlineHeightBar` 和只为它存在的接线一并删除。App 设置搜索的「键盘」页加上关键词「键盘调整」「底部高度」「键盘宽度」。

### 三端统一契约

- **外观**：整块键盘（工具栏或候选行加按键区）上盖约 45% 不透明的黑色遮罩，下面的键盘照常画作实时预览，但不接受触摸。四角画皮肤强调色（accent）的直角括号标出键盘外框。上沿正中、下沿正中各一个横向把手（三道横线），左右两沿垂直居中各一个竖向把手（两道竖线），都在外框以内。中间一排三个白色图标加文字的按钮：「重置」（`arrow.counterclockwise`）、「移动」（`arrow.up.and.down.and.arrow.left.and.right`）、「确认」（`checkmark.circle`）。
- **每个柄只动它那条边，对边在屏幕上不动**：
  - 上沿：改高度，往上拖变高。
  - 下沿：改底部抬高。从按下时的高度 h0、抬高 l0 出发，手指累计往上移 u：u ≥ 0 时抬高跟着手指走，`lift = min(l0 + u, 160)`，高度尽量反向让出同样多但不低于下限，`height = h0 − min(lift − l0, h0 − (−12))`，也就是先拿高度换抬高、上沿不动，高度到下限后整块继续上移，抬高到 160 才停；u < 0 时抬高减、高度加同样多，上沿不动，抬高到 0 或高度到 48 就停住。读屏的「键盘底部高度」增加只要抬高没到上限就有效。
  - 左、右沿：改宽度，同时改水平位置使另一侧边不动。
  - 「移动」：按住拖动整块键盘，同时改底部抬高和水平位置，大小不变。
- **按钮**：「重置」把四项预览回缺省（高度 0、抬高 0、宽度 100%、居中），不保存；「确认」保存四项并退出。没有「取消」。其他任何离开方式（键盘收起、换输入框、切到别的应用）都按确认保存。
- **不生效的项不显示对应的柄**：iPhone 横屏、iPad 浮动键盘和台前调度 / Slide Over 缩成手机尺寸的窗口里，底部抬高、宽度、位置都不生效，只留上沿柄、隐藏「移动」；单手模式或分离式键盘时宽度和位置不生效，隐藏左右柄，「移动」只改抬高。存下的值不因此改变。
- **读屏**：每个柄是可调节元素，三端朗读同样的文字：上沿柄「键盘高度」、下沿柄「键盘底部高度」、左柄「键盘宽度（左边）」、右柄「键盘宽度（右边）」，三个按钮「重置」「移动」「确认」。步长为高度 5%（按键区百分比，与 App「键盘」页同一换算）、抬高 4 pt、宽度 5%、位置 50‰。「移动」上下方向的增减调抬高（4 pt）；能左右移时左右方向调位置（50‰），iOS 上是「移动」的两个自定义操作「向左移」「向右移」（VoiceOver 的可调节手势只有上下），宽度不生效时不提供这两个操作。
- **存储一律只存本机、不同步**：

| 项 | 存放 | 范围与缺省 |
| --- | --- | --- |
| 高度 | 沿用 App Group `keyboard.height.adjustment` 与共享文档 `touch_keyboard_height_adjustment` | −12…48 pt，缺省 0 |
| 底部抬高 | App Group `keyboard.bottomLift` | 0–160 pt，缺省 0 |
| 宽度 | App Group `keyboard.widthPercent` | 60–100，缺省 100；拖动时键盘宽度另不小于 240 pt，取两者中更宽的 |
| 水平位置 | App Group `keyboard.positionX` | 0–1000，剩余空间里左侧占的千分比，缺省 500 居中 |

- 收窄后两侧和底部抬起的地方显示皮肤背景，点击不落到按键。宽度作用于整块键盘：工具栏或候选行、按键区和面板一起收窄。

### iOS 的实现

- **纯类型**：`KeyboardPlacement`（`SharedUI/preferences/KeyboardLayoutPreference.swift`，构造时限制范围，`KeyboardLayoutPreference.placement` 读写三个 App Group 键，不是数字的存储值按缺省）；`KeyboardAdjustCapabilities`、`KeyboardAdjustLayout`、`KeyboardAdjustSession`（`KeyboardExtension/Sources/keyboard/KeyboardAdjustLayout.swift`）管能力判定、外框换算、各边拖动约束和读屏步长。宽度存整数百分比，左右沿拖动先把宽度取整，再按不动的那条边反推位置，所以不动的边误差不到 0.5 pt。
- **键盘外框**：控制器新增 `keyboardFrameGuide`，`applyKeyboardPlacement` 按 `KeyboardAdjustLayout.frame` 设它到视图的前、后、下距离；根视图的四条内边距约束改为钉在外框上，回复面板、符号面板和 AI 润色、语音结果两个服务面板（`attachServicePanel`，原来是 `frame = view.bounds`）也钉在外框上（功能面板等原本就钉在根视图上）。皮肤背景仍铺满视图。高度约束 `keyboardHeight` 的常量是 `默认高度 + 高度调整 + 生效的抬高`，抬高为 0 时与原来相同，读这个约束的测试不受影响。
- **按外框宽度计算**：单手模式的块宽（`applyOneHanded`）、中间一排缩进（`updateLetterRowInsets`）和 iPhone 横屏能否分离（`wantsSplitKeyboard`）原来都用 `view.bounds.width - padding` 硬算，现在统一读 `keyRowWidth`（外框宽减内边距）。单手和分离时收窄不生效、外框就是全宽，所以这两种情况的行为不变；iPhone 横屏同理。
- **拖动坐标**：遮罩把手指位置换算成「到窗口底边的高度」和「窗口里的横坐标」，从按下时算位移。键盘长高、抬高和收窄都会挪动柄本身，窗口的底边和左右却不动，所以不会追着拖动自己引起的变化跑。旧调节条按 `window.bounds.maxY` 算的前提「键盘底边不动」在有抬高后不再成立，但窗口底边仍是屏幕底边，换算照用。
- **中间一排按钮**：排在左右两个柄之间。收窄到最窄（iPhone 上约 240 pt）时三个按钮按原尺寸（72 pt 宽、间距 28）排不下，会盖住左右柄的中段，而按钮叠在柄之上，按住把手拖动就成了点「重置」或「确认」；排不下时先把间距收到 4 pt（按钮与柄之间也留 4 pt），再平分剩下的宽度给按钮。
- **离开即保存**：`closeKeyboardAdjust` 是唯一的退出路径，`viewWillDisappear`、`textDidChange` 里发现换了输入框、`NSExtensionHostWillResignActive` 和「确认」都调它；它写 App Group 的三项摆放和高度，再 `commitTouchKeyboardGeometry()` 把高度写进共享文档。
- **预览不被同步覆盖**：遮罩开着时 `synchronizeSharedTouchPreferences` 不再用文档里的高度覆盖 `sharedKeyboardHeightAdjustment`（App Group 镜像照常更新）。键盘出现时发出的偏好重读可能在用户打开遮罩之后才到，原来会把正在预览的高度弹回去。没有取消，也就不需要像 Android 的 `adoptSavedHeightAdjustment` 那样另记「原值」。

## Alternatives considered

- **保留内联调节条，只加一个下沿把手** — 改动最小，打字时按键照常可用，能边打边看。但宽度和位置没有地方放，柄挤在工具栏那一行也做不到「每个柄只动它那条边」；三端要一致的是截图里的整块遮罩。
- **收窄靠改根视图的左右内边距约束** — 不用新加外框，`keyboardPaddingConstraints` 现成。但回复面板和符号面板钉在视图上，不会跟着收窄，要逐个改成钉在根视图上再减内边距；外框作为一个布局参考，所有「整块键盘」的东西都钉它，语义更直接，遮罩的括号也直接取它的 `layoutFrame`。
- **把抬高、宽度、位置放进共享偏好文档随账号同步** — 换设备不用重调。但这几项依赖屏幕尺寸，iPhone 上调好的宽度百分比放到 iPad 上意思不同；契约定为只存本机，与 `keyboard.oneHanded` 同类。
- **离开时按取消处理（沿用 iOS 原来的语义）** — 严格的「确认才算数」。但这正是 [键盘里调的高度被设置页的值盖回去](../bug-fix/2026-10-08-touch-keyboard-height-reverts.md) 里 Android 推翻掉的做法：调到满意直接收起是最自然的结束方式；新界面没有取消按钮，三端统一为离开即保存。

## Consequences

- **收益**：iPhone 竖屏和全宽 iPad 上可以把键盘抬离底边、收窄并左右挪动，当场看到效果；四项都有读屏入口。三处硬算宽度的地方收成一个 `keyRowWidth`，以后再有改外框的需求不用逐处找。
- **代价**：误拖之后收起键盘也会保存，要撤销只能再调回去或点「重置」再确认。下沿柄往上拖越过高度下限后上沿不再固定，整块键盘跟着上移，这是维护者为了让下沿柄能一直抬到 160 而接受的。遮罩开着时不能打字，失去了旧调节条「边打字边看」的手感。
- **iOS 与契约的细小差别**：「移动」的左右方向在 iOS 上是自定义操作而不是滑动手势，见上；宽度渲染时也按 240 pt 兜底（契约只说拖动时），iPhone 上只在竖屏生效、宽度又只能拖到下限，所以实际不会触发，只在 iPad 极窄窗口里起作用，而那时宽度本来就不生效。
- **未验证**：没有在真机上走过一遍，拖动手感、括号和把手的视觉、iPad 台前调度里的柄显隐只经模拟器单测和代码审读。App 的「键盘」设置页只显示高度，抬高、宽度和位置只在键盘里能调。

## Verification

- `KeyboardTests/settings/KeyboardAdjustLayoutTests.swift`：存储键名、范围与缺省（超出 `Int` 的存储值也限制到范围内、不崩）；能力判定（iPhone 竖屏、横屏、iPad 浮动、全宽 iPad、单手、分离）；外框换算与 240 pt 下限；上沿限幅；下沿在高度到下限之前「高度加抬高不变」、高度在下限时往上拖抬高继续增加到 160、一次拖动跨过下限再拖回、往下拖的各端点停住；左右沿另一侧边不动；「移动」不改大小、不出视图；会话每帧从按下时重算、不生效的柄不改值、「重置」回缺省；读屏步长（含「移动」上下调抬高、左右调位置）。
- `KeyboardTests/settings/MorePanelFoldTests.swift`：遮罩铺满视图、按键收不到触摸、七个控件的显示与三端统一的读屏文字、上沿预览不保存、「确认」写 App Group 和共享文档；下沿抬高时视图高度不变、根视图底边上移、收起键盘即保存且抬高不进共享文档；左右沿收窄整块键盘（工具栏一起）、空出的地方点不到键、「重置」不保存；iPhone 横屏只留上沿柄；单手时没有左右柄、「移动」只改抬高；遮罩开着时晚到的偏好重读不覆盖预览；收窄到最窄时中间三个按钮不盖住左右柄，柄的中段点下去落在柄上。
- `KeyLayoutStyleTests.testTallReturnFollowsTheKeySpacingTheFailedGeometryCommitFallsBackTo` 改为经「键盘调整」的「确认」触发写文档失败的回退路径。
- 在 iPhone 17（iOS 27.0）模拟器上用 `-only-testing` 跑 `KeyboardAdjustLayoutTests`、`MorePanelFoldTests`、`OneHandModeTests`、`KeyboardFormFactorTests` 和上面那一条，共 73 个用例全过；下沿柄规则改为越过高度下限后继续抬高之后，`KeyboardAdjustLayoutTests`（13）和 `MorePanelFoldTests`（19）共 32 个用例全过；审查时补上按钮避让左右柄、服务面板钉外框和存储值溢出之后，同样五组（`MorePanelFoldTests` 20、`KeyboardAdjustLayoutTests` 13、`OneHandModeTests` 16、`KeyboardFormFactorTests` 26 和上面那一条）共 76 个用例全过；单手与分离式键盘的既有用例覆盖了宽度改从 `keyRowWidth` 读之后行为不变。

## 相关笔记

写这篇前检索了 `.agents/notes/` 里的「键盘高度」「内联」「调节条」「底部高度」「单手」「分离」：

- [iOS 功能菜单的「键盘高度」与 Android 一样打开内联高度条](2026-10-08-ios-keyboard-height-tile.md)：被本篇部分取代。内联调节条、「取消」和「收起按取消」不再成立；这一格在功能菜单里的位置仍然成立，那篇保留并加了指向本篇的链接。
- [键盘里调的高度被设置页的值盖回去](../bug-fix/2026-10-08-touch-keyboard-height-reverts.md)：部分重叠，iOS 键盘里现在也是收起即保存，那篇加了互链。
- [Android 恢复单一的「键盘底部高度」滑块](2026-11-01-android-keyboard-bottom-height-slider.md)：它把内联下沿拖动柄留给以后，本篇是三端契约在 iOS 上的那一份；Android 和鸿蒙的实现各自另写。
- [iPhone 横屏与鸿蒙大屏的分离式键盘](2026-10-30-split-keyboard-iphone-harmony.md)、[iPad 单手大小](2026-10-29-tablet-one-handed-size.md)：无冲突，它们的宽度现在从外框宽度算，收窄在这两种模式下不生效。
