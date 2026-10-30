# Agent Note: 鸿蒙浮动键盘

Status: implemented

## Problem

#10225：鸿蒙要有「浮动键盘」，键盘缩成一块可以拖动的小面板，不占满屏幕宽度。Android 已有（[Android 浮动键盘](2026-10-08-android-floating-keyboard.md)，#5621），iPhone 系统不允许第三方键盘悬浮，不做。鸿蒙的软键盘原来只建 `FLG_FIXED` 面板，系统把它钉在屏幕底部，宽度铺满。动手前要先确认系统输入法面板能不能缩小、能不能挪、能不能拖。

## API 支持到什么程度

查的是两份 SDK 的 `openharmony/ets/api/@ohos.inputMethodEngine.d.ts`：Studio 上 DevEco 自带的 HarmonyOS 6.0.2（API 22，`sdk-pkg.json` 的 `apiVersion: 22`），和本机 `~/command-line-tools` 的 HarmonyOS 6.1.1（API 24）。两份的 `Panel` 接口在下面几项上相同。工程的 `compatibleSdkVersion` / `targetSdkVersion` 都是 `6.0.1(21)`（`platforms/harmony/build-profile.json5`）。

| 接口 | 起始 API | `.d.ts` 原文要点 |
| --- | --- | --- |
| `PanelFlag.FLG_FLOATING` | 10 | 「When the flag is set, the soft keyboard is floating.」只对 `SOFT_KEYBOARD` |
| `PanelInfo.flag` | 10 | 建面板时给标志，`@default FLG_FIXED` |
| `Panel.resize(width, height)` | 10 | 没有标志限制 |
| `Panel.moveTo(x, y)` | 10 | 「It's unusable for SOFT_KEYBOARD panel with FLG_FIXED.」 |
| `Panel.changeFlag(flag)` | 10 | 「Before flag is changed, Developers should hide the panel. After that, developers can change the content, size, point of the panel and show it again」 |
| `Panel.startMoving()` | 15 只限 `STATUS_BAR`；20 起 | 「It's only used for STATUS_BAR panel or SOFT_KEYBOARD panel with FLG_FLOATING or FLAG_CANDIDATE.」 |
| `Panel.setUiContent(path)` | 10 | 与标志无关 |
| `Panel.adjustPanelRect(flag, EnhancedPanelRect)` | 15 | 浮动、固定都可用；`landscapeAvoidY` / `portraitAvoidY` 是「面板顶边到避让区顶边的距离」，默认 0 |
| `on('sizeChange')`、`updateRegion` | 12 / 15 | 浮动、固定都可用 |

结论：在工程的目标 API 21 上，浮动面板能缩小（`resize`）、能定位（`moveTo`）、能拖（自己算位移再 `moveTo`，或 20 起的 `startMoving`），能在运行中切换（`changeFlag`，要先隐藏）。可行。

`.d.ts` 没有说浮动面板会不会让应用避让（键盘高度事件、`TYPE_KEYBOARD` 避让区只写了固定面板的语义），这一项只能在设备上看。

## Decision

- 开关和位置存在设备本地的按键反馈文件里（`KeyboardFeedback` 的 `floating`、`floatingX`、`floatingY`），与单手模式、隐私模式放在一起，不进共享偏好，也不随账号同步：Android 的 `platform.android.floating_keyboard` 和 `_x` / `_y` 同样只在本机。设置页和账号同步经 `KeyboardFeedbackBridge.fromShared` 写这个文件时从已存的值沿用这三项。位置是可移动范围里的千分比，默认 500 / 1000（水平居中、贴底），与 Android 相同。
- 入口与单手模式在鸿蒙上已有的入口相同：功能面板的开关磁贴「浮动键盘」（Android 同名，排在鸿蒙额外项的最后），以及浮动面板拖动条右端的「停靠」键。鸿蒙的单手模式没有设置页开关和工具栏按钮，浮动键盘也不加；Android 设置页的开关和可选的工具栏按钮没有移植。
- 几何在 `keyboard/FloatingKeyboardPolicy.ts`，常数与 Android 的 `FloatingKeyboardPolicy` 相同：宽度是屏幕的 80%，夹在 240–480vp 且不超过屏幕；高度是停靠时的键盘高度加 22vp 拖动条，不另外缩放；圆角 12vp。位置换算、拖动钳制、转屏后重算都在这里。
- 面板：扩展能力按存着的开关建 `FLG_FLOATING` 或 `FLG_FIXED` 面板，第一次显示就在存着的位置上。磁贴或停靠键改开关时，会话经 `onFloatingKeyboardChanged` 通知扩展能力，扩展能力按 `.d.ts` 的要求先 `hide`，再 `changeFlag`，按新标志 `resize`（浮动时再 `moveTo`），最后 `show`；换不成功也显示回来。几次切换排进同一条 Promise 链，一次做完一个。停靠路径与以前一样只 `resize`，宽度是建面板时的屏幕宽度，从不对固定面板调 `moveTo`。
- 拖动：拖动条中间 36×4 的横条接触摸，视图报告手指在屏幕上的位移（`TouchObject.displayX` / `displayY`，面板跟着移动，条内坐标会变，屏幕坐标不会），扩展能力换成像素加到按下时的位置上，钳在屏幕里，`moveTo`。松手时换成千分比，经 `KeyboardSession.saveFloatingPosition` 写盘；写不进去时只在这个进程里有效，与 Android 相同。
- 转屏和折叠屏开合：扩展能力在手机和平板上订阅 `display.on('change')`，记下新的屏幕尺寸和密度；浮动时按存着的千分比重新定宽、定位置，所以同一个位置在横屏上仍落在屏幕里。销毁时退订。
- 视图：浮动时根节点顶部多一条拖动条，背景是皮肤底色，整块切 12vp 圆角；键盘这一层分到剩下的高度，停靠时就是整个窗口，与以前相同。
- 与单手模式、分离式键盘的关系：浮动时单手模式不生效（`KeyboardView.oneHandActive`），存着的值不变，磁贴显示为不可用，与 Android 相同；大屏单手的侧边数字键盘随之不画。2in1 的面板是跟随光标的候选窗（`FLAG_CANDIDATE`），浮动磁贴在那里不可用。鸿蒙的分离式键盘（#10224，见[分离式键盘](2026-10-30-split-keyboard-iphone-harmony.md)）合进来之后，浮动优先：浮动时不画分离式键盘（`SplitKeyboardPolicy.drawn` 多收一个浮动状态，浮动时返回 false），分离设置不变，停靠后按原样回来；分离画着时浮动磁贴照常可用，打开即停止分离；单手磁贴在分离和浮动时都置灰。三条都与 Android 相同：`MSIMEInputService.splitKeyboardDrawn` 在 `floatingDrawn()` 时返回 false，`ImeFunctionPanel` 的 FLOATING 磁贴只看 `panelPreferenceSaving` 和外接键盘模式，ONE_HAND 磁贴要求 `!splitKeyboardDrawn() && !floatingDrawn()`。

## Alternatives considered

- **拖动用系统的 `startMoving()`**：API 20 起支持浮动软键盘，系统接管拖动，手感最顺，不用每次移动发一次 `moveTo`。不用它：`Panel` 没有读取当前位置的接口，系统拖完之后宿主不知道面板停在哪儿，存不了位置，转屏后也没法按千分比重摆；手指在 `startMoving` 之后还能不能收到抬起事件 `.d.ts` 也没写。2in1 工具栏已经用「手势位移 + `moveTo`」拖了很久，浮动键盘沿用。
- **不切换标志，只把固定面板 resize 窄**：不用碰 `changeFlag` 和隐藏再显示。不用它：`moveTo` 对 `FLG_FIXED` 不可用，固定面板窄了也只能贴底，拖不动，不是需求要的东西。
- **另开一块 `STATUS_BAR` 面板画浮动键盘**：`STATUS_BAR` 可以自由移动。不用它：系统只给一块 `STATUS_BAR`（2in1 上实测，工具栏和模式角标为此二选一），软键盘面板的显示与隐藏由系统跟着编辑框管，另一块面板得自己同步焦点和生命周期。
- **开关进共享偏好，随账号同步**：设置页能直接显示。不用它：Android 不同步，位置与这块屏幕的尺寸有关；鸿蒙的单手模式也是同一个文件里的设备本地值。
- **位置存像素**：最直接。不用它：转屏、折叠屏开合后像素会落到屏幕外，还得另写修正；千分比换算时自然落在范围里，与 Android 相同。

## Consequences

- 收益：键盘挡住输入框时可以把键盘缩小挪开；转屏后面板留在屏幕里、相对位置不变；停靠路径的代码和行为与以前相同。
- 代价与已知上限：面板钳在整块屏幕里，不避开状态栏和导航条（Android 钳在系统栏之间）；扩展能力拿不到系统栏的避让区，拖到最上面时拖动条可能压在状态栏下。拖动只能用手指，读屏用户只能用「停靠」键回到停靠键盘。切换开关时面板要隐藏再显示一次，会触发一次系统的 `keyboardHide`（视图借此停掉语音、写盘打字统计）。开关和位置不随账号同步，设置页里没有这一项。扩展能力在 `display.on('change')` 里更新密度，停靠面板之后的 resize 也按新密度换算（以前一直用建面板时的密度）。
- 验证：`platforms/harmony/tests/floating-keyboard.test.ts` 钉住常数、生效条件、宽度、高度、千分比互换、拖动钳制、任意千分比都落在屏幕里、竖屏转横屏后仍贴右边且在屏幕里、三项设置的读写与回退、不进共享结构、磁贴的名称与位置；`keyboard-logic.test.ts` 的功能面板顺序和开关列表跟着加了一格；`tests/run.sh` 用文本检查钉住扩展能力只在隐藏之后 `changeFlag`、换完显示回来、停靠路径不 `moveTo`、订阅和退订 `display.on('change')`、松手存位置，以及视图在浮动时关掉单手模式；合入分离式键盘后，`split-keyboard.test.ts` 加了浮动时不分离、停靠后分离回来、2in1 两者都不画的用例，`run.sh` 钉住视图把浮动状态传给 `SplitKeyboardPolicy.drawn`、单手在分离或浮动时都让位、单手磁贴在两种情况下都置灰。HAP 在 Mac Studio 上构建通过；没有在设备或模拟器上验证（本机手机模拟器因磁盘剩余空间不足起不来，2in1 模拟器不浮动），面板外观、拖动、切换时的隐藏再显示、转屏重摆、应用是否避让、窗口是否透明都还要在手机或平板上看。
