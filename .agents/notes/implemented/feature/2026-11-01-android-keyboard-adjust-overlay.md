# Agent Note: Android「键盘调整」遮罩取代内联高度条

Status: implemented

## Problem

功能面板的「键盘高度」打开的是工具栏位置的内联高度条（`InlineHeightBar`）：只有一个上下拖动柄，只能改高度，「取消 | 拖动柄 | 重置 | 完成」四样挤在一行。用户要的不止高度：#10447 照着 Gboard 找的是键盘下沿的拖动柄（[键盘底部高度滑块](2026-11-01-android-keyboard-bottom-height-slider.md) 只把抬高放进了设置页，在键盘里调不了）；手机上想把键盘收窄、挪到一侧的需求也没有任何入口，Android 没有持久的键盘宽度或水平偏移。用户给的目标效果是 Gboard 式的整块遮罩：键盘上盖一层暗色，四边各有把手，中间是「重置 / 移动 / 确认」。iOS 和鸿蒙同时按同一份契约改，三端界面和行为要一致。

当场审计（`rg` 内联高度条、`InlineHeightBar`、键盘高度磁贴、水平位置）：

- 部分取代 [键盘高度范围](2026-10-08-android-keyboard-height-range.md)：75%–170% 的范围、窗口封顶和存储照旧；内联条的「N% · H px」读数随内联条一起删掉。
- 部分取代 [调整高度时固定输入法窗口](../bug-fix/2026-10-29-android-inline-height-drag-window.md)：全高窗口、固定避让边界、只把外框算作可触摸区域这一整套照旧，触发条件从内联高度条换成本遮罩，并多了抬高和收窄两种预览。
- 部分取代 [键盘里调的高度被盖回去](../bug-fix/2026-10-08-touch-keyboard-height-reverts.md)：「收起即保存」照旧并扩大到四项；那篇留下的「点取消才放弃」不再成立，因为没有取消了。
- 部分取代 [键盘底部高度滑块](2026-11-01-android-keyboard-bottom-height-slider.md) 的备选「在内联条加下沿拖动柄」：那篇因交互改动大留给以后，本篇就是那个以后；滑块和它的存法照旧，两处读写同一份。
- 相关但不重叠：[浮动键盘](2026-10-08-android-floating-keyboard.md)、[单手模式大屏尺寸](2026-10-29-tablet-one-handed-size.md)、[手机横屏分离式键盘](2026-11-01-android-phone-landscape-split.md) 决定了哪些状态下宽度和位置不生效；[iOS 键盘高度磁贴](2026-10-08-ios-keyboard-height-tile.md) 描述的是 iOS 当时对齐 Android 内联条的做法，iOS 那边另行改。

## Decision

功能面板原「键盘高度」磁贴改名「键盘调整」（`FunctionPanelModel`，图标不变，`Id.KEYBOARD_HEIGHT` 这个稳定 id 不改），设置搜索的键盘页加关键词「键盘调整」。点开后按下面的三端契约工作。

### 界面

- 整块键盘（工具栏或候选行加按键区）上盖一层黑色、约 45% 不透明（`0x73000000`）的遮罩 `KeyboardAdjustOverlay`，下面的键盘照常画作实时预览，但触摸全被遮罩接走。遮罩四角画皮肤强调色（`skin.accent()`）的直角括号，标出键盘外框。遮罩是外框 `PanelSurface` 的普通子视图，外框按键盘列的底部内边距给它底边距，所以它正好盖住外框，不盖底栏、底部抬高和导航栏那一截；那一截在调整期间同样不接受触摸（`PanelSurface.onInterceptTouchEvent`）。
- 拖动柄：上沿正中、下沿正中各一个横向把手（三道横线），左右两沿垂直居中各一个竖向把手（两道竖线），都画在外框以内：调整期间输入法窗口铺满，`onComputeInsets` 只把外框的矩形算作可触摸区域，画到框外的柄点不到。
- 中间一排三个按钮，白色图标加文字：「重置」（逆时针回转箭头）、「移动」（四向箭头）、「确认」（圆圈里的对勾）。图标经 `generate_keyboard_icons.py` 加进 `KeyboardIconPaths`（`ADJUST_RESET`、`ADJUST_MOVE`、`ADJUST_CONFIRM`，Lucide 风格）。三个按钮加两个竖向把手不超过最窄的键盘（240 dp）。

### 每个柄只动自己那条边，对边在屏幕上不动（下沿柄往上拖到高度下限后例外）

- 上沿：改键盘高度，往上拖变高。只在窗口封顶（`KeyboardHeightPolicy.windowHeightAdjustment`）以内拖，存着的值高过封顶时从封顶处起算。
- 下沿：改底部抬高，高度反向变化同样多，上沿不动（往上拖 d：抬高 +d、高度 −d）。从按下时的高度 h0、抬高 l0 出发，手指累计往上移 u：u ≥ 0 时抬高跟着手指走，`lift = min(l0 + u, 抬高上限)`，高度尽量反向让出同样多但不低于下限，`height = h0 − min(lift − l0, h0 − 高度下限)`，即先拿高度换抬高、上沿不动，高度到 75% 后整块继续上移（上沿也跟着上去），直到抬高到上限才停；u < 0 时抬高减少、高度反向增加，上沿不动，抬高到 0 或高度到上限时停。高度调整量和抬高都是 dp，在键盘总高里一比一相加，所以这一条只靠换算就成立，不用量布局。
- 左、右沿：改宽度，同时改水平位置让另一侧边不动。
- 「移动」：按住拖动整块键盘，竖直改抬高、水平改位置，大小不变。
- 拖动按按下处起算的位移报告（屏幕坐标），每一步都从按下时的值重新算，手指来回拖不累积误差；换算全在 `KeyboardAdjustPolicy`。

### 重置、确认与离开

- 重置：四项预览回默认（高度 100%、抬高 0、宽度 100%、居中），不保存。
- 确认：保存四项并退出。没有「取消」。收起键盘、返回键、切应用、换输入框（都经 `onFinishInputView`）一律按确认保存，沿用 [键盘里调的高度被盖回去](../bug-fix/2026-10-08-touch-keyboard-height-reverts.md) 的决定。调整期间偏好重载不覆盖预览（`adoptSavedHeightAdjustment`、`applyPreferencesSnapshot` 只在没在调整时才换高度）。

### 当前不生效的项不显示对应的柄

`KeyboardAdjustPolicy.Availability`：

- 浮动键盘（含跟随光标的候选栏）、手机横屏、实体键盘的候选条模式里，底部抬高、宽度和位置都不生效：只留上沿柄，不显示「移动」。候选条模式按 `hardwareKeyboardMode` 判断而不是此刻的 `hardwareKeysCollapsed()`：调整期间键区会临时展开，关上遮罩后才收回去，柄要按关上后的状态给。
- 单手模式或分离式键盘画着时宽度和位置不生效：不显示左右柄，「移动」只改抬高。
- 底部抬高的生效条件与 `KeyboardBottomBarPolicy.extraPaddingDp` 现有规则一致。

### 读屏

每个柄和「移动」都是可调节元素：RangeInfo 加前后滚动的增减动作，步长高度 5%、抬高 4 dp、宽度 5%、位置 50‰。上沿柄的范围是 75% 到窗口封顶对应的百分比；下沿柄的范围是抬高的 dp，只要抬高没到上限就提供「增」（高度到 75% 后整块上移），规则同拖动。「移动」能左右移时前后滚动调位置、上下滚动（`ACTION_SCROLL_UP` / `DOWN`）调抬高，只能改抬高时前后滚动调抬高。三个按钮各有文字说明；柄的说明是「键盘高度」「键盘底部高度」「键盘宽度（左边）」「键盘宽度（右边）」。遮罩打开时键盘列和底栏设成 `IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS`：下面的按键只是预览，读屏不进去。

### 存储

一律只在本机、不随账号同步，和浮动键盘位置、底部留白同一口径；新键进 `AndroidLocalSettings` 的规格表和本地备份（`LocalBackupPolicy.backsUpLocalSetting` 不排除它们），不进同步白名单。

| 项 | 键 | 取值 |
| --- | --- | --- |
| 高度 | `platform.android.keyboard_height_adjustment` | 沿用现有的 −46..129 dp（75%–170%） |
| 底部抬高 | `platform.android.bottom_padding` + `platform.android.bottom_padding_height` | 0–160 dp，复用设置页「键盘底部高度」的两项 |
| 宽度 | `platform.android.keyboard_width_percent` | 60–100，缺省 100；拖动时另有下限：键盘不窄于 240 dp，两者取更宽 |
| 水平位置 | `platform.android.keyboard_position_x` | 0–1000，剩余空间（可用宽度减键盘宽度）里的千分比，缺省 500 居中 |

拖动柄操作的是实际抬起的距离 extra：底栏画着时存 `h = extra + 40`，extra 为 0 时只写 `bottom_padding = false`、保留原高度（与 `bottomHeightEdits` 一致），底栏不画时 `h = extra`。设置页滑块继续读写同一份。确认时只写此刻生效、并且和打开时不同的项（`AndroidLocalSettings.keyboardAdjustEdits`）：没动过的抬高不写，免得把底栏画着时存的 30 dp 这类「抬起 0」的旧值改成关。这几项和高度在同一次原子写里落盘（`saveTouchGeometry(reset, localEdits)` 经 `KeyboardHeightSave` 的本地写入），写之前先用 `AndroidLocalSettings.Snapshot.withEdits` 套进内存里的本地设置，键盘不必等后台写完才按新值画，写失败时 `finishTouchGeometrySave` 重读文件回到旧值。

### 宽度与位置落在哪里

宽度作用于整块键盘：工具栏或候选行、按键区、面板和底栏一起收窄。实现是外框 `PanelSurface` 测量时按可用宽度算出的左右内边距（`MSIMEInputService.keyboardHorizontalInsets` → `KeyboardAdjustPolicy.insetsPx`）：两侧空出来的地方是外框自己的皮肤底，不透出应用，点上去落不到按键，调整期间和 API 28 的全高窗口里也仍在外框的可触摸矩形内。底部抬起的那一截本来就是键盘列的底部内边距，同样画外框的皮肤底。大屏上外框本身是 720 dp 的居中块（`KeyboardFormFactorPolicy.surfaceWidthDp`），在它里面再收窄。按键的触点几何（`ImeTouchPoints`）在键区布局变化时重新登记，宽度一变就触发重新布局，平移不用重新登记。

## Alternatives considered

- **保留内联高度条，只在它旁边加一个下沿拖动柄**：最强的理由是改动小，[键盘底部高度滑块](2026-11-01-android-keyboard-bottom-height-slider.md) 当时设想的就是这一步。不用它，是因为宽度和位置同样要在键盘里调，一行工具栏放不下四个柄，三端也已经按整块遮罩统一了契约；遮罩把每个柄放在它改的那条边上，比一行里的按钮直观。
- **用外框的布局参数（宽度加左外边距）收窄**：外框直接变窄，两侧露出根视图的底色，不碰外框的内边距。不用它，是因为调整期间和 API 28 的输入法窗口铺满全屏，可触摸区域只算外框的矩形：外框变窄后两侧那一截不再属于输入法，点上去会落到下面的应用里；而视觉上那里画着键盘底色，用户以为点的是键盘。内边距让两侧仍属于外框，不论窗口铺不铺满都不漏触摸。
- **宽度和位置存成 dp 或像素**：拖动时换算最直接。不用它，是因为同一台设备转屏、折叠屏开合、分屏时可用宽度都会变，存 dp 会让键盘在新宽度里偏到一边甚至超出；存百分比和剩余空间里的千分比，任何宽度下都落在里面、相对位置不变，和浮动键盘位置的千分比同一个思路。
- **上沿柄拖到设计上限 170%，不管窗口封顶**：读屏范围在所有设备上都是 75–170，测试也好写。不用它，是因为封顶以上那段画出来一样高，手指往上拖、边却不动，再往下拖又要先走完这段看不见的距离才开始变矮；下沿柄把高度和抬高一比一互换也要求高度是画出来的值，否则上沿会跟着动。
- **保留「取消」**：误拖之后能一键回去。不用它，是因为三端契约和 Android 已有的决定都是离开即保存；「重置」能回默认，要回到打开前的值只能再拖回去，代价已在 [键盘里调的高度被盖回去](../bug-fix/2026-10-08-touch-keyboard-height-reverts.md) 里记过。

## Consequences

- 收益：在键盘里就能调高度、抬高、宽度和位置，所见即所得；下沿柄补上了 #10447 想要的 Gboard 式交互；手机上能把键盘收窄并挪到一侧，单手握持够得着。拖动时输入法窗口不改高（沿用 #10025 的全高窗口），应用不重排。
- 代价：
  - 照片皮肤收窄后，两侧画的是外框的皮肤底图（照片延伸到两侧），不是纯色；纯色皮肤与契约的「皮肤底色」一致。
  - 手机横屏、浮动键盘、单手、分离式键盘里宽度和位置不生效，回到竖屏或停靠后存着的值才重新生效；设置页没有宽度和位置的控件，只能在键盘里调，或者在这里「重置」。
  - 「键盘调整」打开期间工具栏、候选行和自动填充行照常画在遮罩下面，不再像内联条那样替换工具栏行。
  - 撤销误操作只能再拖回去或重置。
- 测试：`KeyboardAdjustPolicySmoke` 覆盖哪些柄显示、上沿只改高度且受窗口封顶、下沿一比一互换、高度到 75% 后抬高继续增加且高度停在下限（含一次拖动跨过下限）、抬高到 0 或上限停住、左右柄另一侧边不动（含 240 dp 下限）、「移动」大小不变、读屏范围和步长；`AndroidLocalSettingsSmoke` 覆盖新键的范围、不同步、`keyboardAdjustEdits` 只写生效且变了的项和抬高的存法、`withEdits` 与写盘同一套校验；`LocalBackupPolicySmoke` 确认两项随本地备份走；`ms_w2_kb_ViewLogicSmoke` 钉住遮罩的颜色和说明；`FunctionPanelModelSmoke`、`SettingsSearchIndexSmoke` 钉住改名和搜索。以上由 `platforms/android/check-host.sh` 运行。设备套件 `KeyboardHeightDeviceSmoke` 改为驱动遮罩：上沿到 130%、窗口上限和 75%，返回键收起即保存且没有「取消」，重置，下沿抬 24 dp 时第一行键不动、空格上移 24 dp 并按 24 dp 存，右柄收窄到 80% 时左边不动，「移动」移到最右，左柄加宽时右边不动且收起即保存，重置加确认回到四项默认；`checkDragKeepsEditorFrame` 保留编辑器边框不动的断言，并加上抬高和收窄时边框也不动。`BottomPaddingHeightDeviceSmoke` 加了调整期间按预览抬高、关上后回到存着的值，以及 80% 贴左时外框的左右内边距。
- 未验证：设备套件只在本机编译过，没有在模拟器或真机上跑（本机没有可用模拟器），交给 PR 上的 CI；照片皮肤、平板 720 dp 外框里的收窄、API 28 全高窗口两侧的触摸都还没有人在设备上看过。
