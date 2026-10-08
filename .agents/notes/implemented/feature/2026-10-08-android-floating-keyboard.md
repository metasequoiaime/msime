# Agent Note: Android 浮动键盘

Status: implemented

## Problem

#5621：键盘调高以后，个别应用的输入框被键盘挡住、显示不全；报告人希望有浮动小键盘兜底，并能把入口放进工具栏，点一下就切换。宿主原来只有停靠在底部的键盘：输入法窗口按内容包裹高度贴底，系统按窗口顶边让应用让出高度，键盘再窄（单手模式）也照样占满一整条。

## Decision

- 开关是本地设置 `platform.android.floating_keyboard`，位置是 `platform.android.floating_keyboard_x` / `_y`（可移动范围里的千分比，默认居中贴底），工具栏按钮是 `platform.android.toolbar_floating`（默认不显示），都只在本机。入口：功能面板第 3 页的开关磁贴、设置「键盘」页「布局」里的开关、可选的工具栏按钮（Material picture_in_picture_alt 图标），以及浮动面板拖动条上的「停靠」键。
- 几何规则在 `keyboard/FloatingKeyboardPolicy`：宽度是窗口的 80%，夹在 240–480 dp 且不超过窗口；千分比与像素的互换、拖动钳制都在这里。拖动条是 `keyboard/FloatingKeyboardBar`。
- 窗口：浮动时输入法窗口、系统的 `android.R.id.inputArea` 与键盘根视图都改成铺满（LatinIME 的 `updateSoftInputWindowLayoutParameters` 同一做法），根视图与导航栏底色透明；键盘外框（`keyboardSurface`）缩窄、包裹高度、贴在左上角，位置全靠平移，拖动不触发重新布局。`onComputeInsets` 在浮动时把 `contentTopInsets` / `visibleTopInsets` 设为窗口高度，`touchableInsets` 为 `TOUCHABLE_INSETS_REGION`、区域是面板的矩形，所以应用不让出高度，面板外的触摸落到应用上。停靠时把窗口和 inputArea 恢复成第一次浮动前记下的高度；`setInputView`、`updateFullscreenMode` 之后重新套一遍。状态在 `render()` 开头按「是否浮动」比较后才改窗口。
- 键盘列的系统栏内边距由服务自己的 insets 监听决定：停靠时照旧留出导航栏那一截，浮动时不留，系统栏四边只用来限制面板能拖到哪里（`WindowLayout.systemBars` 抽出了原来 `fitSystemBars` 里的换算）。
- 浮动时单手模式和横屏分离式键盘不生效（存着的值不变）；外接键盘的候选条模式里不浮动（`FloatingKeyboardPolicy.active`），候选条停在底部。

## Alternatives considered

- **另开一个悬浮窗（`TYPE_APPLICATION_OVERLAY`）画键盘**：位置不受输入法窗口约束，拖动更自由；但要「显示在其他应用上层」权限，输入法窗口与悬浮窗之间还得同步焦点、显示与隐藏，系统切换输入法时悬浮窗不会自动消失。在输入法窗口里铺满再收窄触摸区域，不要额外权限，生命周期跟着输入法走。
- **一直让窗口铺满、停靠时也用 `onComputeInsets` 算高度**：LatinIME 就是这样，浮动与停靠只差 insets；但宿主的停靠布局（平板居中外框的两侧底色、表情面板的测量修复）都建立在窗口包裹高度之上，一直铺满等于把停靠路径整个换掉，风险落在所有用户身上。只在浮动时铺满，停靠路径保持原样。
- **位置存像素**：最直接；但旋转、分屏或换布局后像素会落到窗口外，还得另写一套修正。千分比换算时自然落在范围里。
- **浮动时同时把键盘调矮**：「浮动小键盘」听起来更像小一号的键盘；但键盘高度已有自己的设置，内联高度条在浮动时照样能用，另加一套缩放会让两个高度互相覆盖。

## Consequences

- 收益：键盘挡住输入框时可以把键盘挪开，应用的布局不受影响；工具栏按钮按报告人的说法一点即切换。
- 代价与已知上限：浮动时应用不知道键盘在哪儿，输入框可能被面板盖住，需要用户拖开；拖动只能用手指，TalkBack 用户只能用「停靠」键回到停靠键盘。开关与位置不随账号同步，也不在 Tauri 共享设置页里出现。窗口与 inputArea 的改写依赖系统 `input_method.xml` 的结构（LatinIME、FlorisBoard 都依赖它），厂商若改了这层布局，浮动可能退化成铺满的透明窗口。
- 验证：`tests/keyboard/FloatingKeyboardPolicySmoke.java` 钉住生效条件、宽度、千分比互换与拖动钳制；`AndroidLocalSettingsSmoke` 钉住新设置项的默认值、范围与不同步；`FunctionPanelModelSmoke`、`KeyboardShortcutIconPolicySmoke` 钉住新磁贴与图标。均由 `platforms/android/check-host.sh` 运行。窗口铺满、触摸穿透和拖动没有在真机或模拟器上验收。
