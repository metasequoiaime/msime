# Agent Note: Android 14/18 键字母键宽度与双字母键面字距

Status: implemented

偏好、排法规则、字距、同步和设置页文字是三端共用的，见 [14/18 键字母键宽度与双字母键面字距的三端规则](2026-11-06-fourteen-eighteen-key-letter-widths.md)；本篇只记 Android 怎么画、怎么接触摸、怎么接线。

## Problem

共享偏好 `touch_fourteen_key_letter_widths` 要在 Android 上真的改变 14/18 键的三排：键盘、「按键布局」页的预览和皮肤页的预览要画同一种排法；「宽窄」第二排两端的空白不能是死区；改了偏好键盘要就地重排。Android 上有三处现成的东西挡路：建键代码按固定份额逐键加按钮，三处预览各自算份额；`KeyboardKeyArea` 只把落在键的外边距（键距、行距）里的按下交给那个键，普通的占位 `View` 里的按下没有键接（26 键第二排的缩进就是这样）；`LinearLayout` 先扣外边距再按份额分，少一个键的那一排单位宽度会大一点。双字母键面的字距同样要在键盘和两处预览里一致，而键盘按键面字符串取键位 id 和读屏念法，字符串不能动。

## Decision

### 排法只有一份

`FourteenKeyLetterWidths`（`platforms/android/java/app/msime/android/keyboard/`，不依赖 Android）把共享笔记里的两个参数记成 `Geometry(singleLetterWeight, fillRows)`，`Mode.EQUAL` 是 (1, 铺满)，`Mode.VARIED` 是 (0.75, 不铺满)；`Mode.parse` 宽松读取。`plan` 把三排排成每一格的份额，格子分三种：字母键（`KEY`）、分词和 ⌫（`EDGE`）、第一二排两端的空白（`PAD`）。键盘（`ImeLetterRows#rebuildFourteenKeyRows`）、「按键布局」页的预览（`KeyLayoutStylesPolicy`）和皮肤页、社区皮肤的预览（`KeyboardPreview`，经 `KeyLayoutStylesPolicy.Preview.letterWidths`）都从这里取，不各自算。`equal` 逐格等于原来的份额（18 键每格 1 份，14 键字母键 `FourteenKeyLayout.LETTER_WEIGHT` 2 份、分词和 ⌫ `KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT` 1.4 份）。预览里的空白画成只占宽度、不画的格（`Key.rows() == 0`）。

建键代码按格拆成分词键（`fourteenSeparator`）和字母键（`fourteenLetterKey`）两个方法，键面、长按、18 键双拼的横滑和键位 id 都没有改。

### 占位格把按下交给所属的键

宽窄第二排两端的空白是 `KeyboardSpacer`：不画、读屏不念，`setOwner` 指定同一排里的一个键（18 键是 A、L，14 键是 AS、L）。`KeyboardKeyArea.search` 找命中时，按下落在指定了所属键的占位格或它的外边距里，就选那个键（`claim`），按下点挪进它靠这一侧的边缘；之后同一根手指的移动、抬起和取消挪同样的距离，和落在键距里的按下走同一条路。按下即反馈（`PressListener`，#10757）和双拼的按下点记录（`TouchPointPolicy`，#10135）都按那个键算，记下的是挪动之前的真实坐标。

`KeyboardSpacer.usableOwner` **只在**所属的键与占位格同一个父视图、可见、量过尺寸时给出它，否则占位格不接收触摸。没有指定所属键的占位格**一律不接收**触摸。26 键第二排的缩进后来也改成了归 A、L 的占位格（[26 键第二排缩进归 A、L](../bug-fix/2026-11-09-android-twenty-six-key-indent-dead-zone.md)）；分离式键盘中间的空隙（`splitGap()`）仍是普通 `View`；14/18 键不画分离式键盘（`SplitKeyboardPolicy.splitsLayout` 只认标准 26 键和韩文两套式），不涉及中间空隙。

### 补齐少扣的键距

字母键、分词和 ⌫ 都带左右两个半键距的外边距，`LinearLayout` 先从行宽里扣掉外边距再按份额分。18 键宽窄第二排比首排少一个键，也就少扣一个键距，单位宽度会比首排大一点。两端的占位格各朝外补 `FourteenKeyLetterWidths.padHalfGaps` 个半键距（18 键是 1；14 键前两排键数相同，是 0）。占位格自己记着朝哪边补几个（`KeyboardSpacer#setOuterHalfGaps`），`ImeStyler.applyKeyboardGeometry(View)` 本来就逐个视图套用键距，走到占位格时按当前键距换成外边距（上下不留），键距就地变化时跟着变；键行里不另外记占位格。补出的外边距也归所属的键，屏幕边上这条不会变成死区。

### 读取和重排

键盘冷启动（`applyTouchGeometry`）和实时偏好（`applyPreferencesSnapshot`）都用 `fourteenKeyLetterWidthsFrom` 宽松读取。这项进 `touchGeometryKey` 和 `SKIN_HINT_KEYS`：冷启动第一帧按皮肤片段里记下的宽度画；偏好变了就 `render()`，`ImeLetterRows.letterWidthsStale` 让键行按新分法重建（`rebuildKeyRows`），和数字行、侧栏符号表等其他键行重建一样不打断正在组的字。没建 14/18 键时 `letterWidthsStale` 恒为假，26 键和九键的重建条件不变。

### 双字母键面的字距

`FourteenKeyLayout.faceLetterSpacing(face)` 按键面给出字距（em）：两个大写字母是 `TWO_LETTER_SPACING_EM`（0.15），双拼的 `L;` 是 `L_SEMICOLON_SPACING_EM`（0.08），其余是 0。按键面而不按键判断，预览里拿到的只有键面也能用；别的键盘没有两个大写字母或 `L;` 这样的键面。键盘在 `fourteenLetterKey` 里 `setText` 之后 `setLetterSpacing`，双拼切换让 L 键面在 `L` 和 `L;` 之间变（`updateFourteenKeySeparator`）时一起换；两种宽度下都设，不读宽度偏好。18 键的自适应字号（`setFittedKeyTextSize(12, 22)`）用 TextView 自带的自适应，量宽度时已算进字距，窄键上照样单行。18 键双拼横滑时键面临时换成单个字母，字距留着，单个字母照样居中。

预览：「按键布局」页的 `KeyLayoutPreview.drawFace` 和皮肤页的 `KeyboardPreview.drawKey` 画键面前给画笔设同样的字距，量宽度缩字号时已算进去；`KeyboardPreview` 的画笔还画候选条文字，画完复原成 0。

### 设置页

「按键布局」页（`KeyLayoutStylesPage`）的 14/18 键分段在开关卡片下面加「字母键宽度」一组：两枚胶囊按 `Mode.CHOICES` 排，与样式胶囊同一个 `chip`，胶囊下面一行组末说明，文字取自共享笔记。这一组只在 14/18 键分段建、也只在那个分段显示；点胶囊先就地重画预览再写偏好，保存失败时和页面其他选项一样提示并重读。设置搜索（`PageId.KEY_LAYOUT_STYLES`）登记「字母键宽度」「宽窄」「键宽」。

Android 没有 14/18 键的热力图，统计页不涉及。

## Alternatives considered

- **占位格用普通 `View`，和 26 键的缩进一样。** 最省事，键行里已经有这种写法。不用：普通视图里的按下没有键接，手指落在那里这一下就丢了；[iOS 18 键](2026-10-25-ios-eighteen-key.md)当初也是为此不用两侧占位视图。
- **占位格的外边距由 `ImeLetterRows` 记一张表、单独套用。** 和 #10401 之前 18 键缩进的 `applyEighteenKeyIndentGeometry` 一样，写法直白，表里最多两项。不用：这张表是要和视图树保持一致的状态，以后哪条重建路径漏了清表，就会留下过期的占位格；让占位格自己记着补几个半键距，在本来就逐个视图套键距的那一遍里换算，没有要同步的东西。
- **补外边距改成调占位格的份额。** 不碰外边距，占位格里的按下天然都在格子里。不用：要补的是像素（一个键距），份额补出来的宽度随行宽变，只能在每次量完尺寸后重算，键距、单手、浮动、旋转都要跟着算一遍。

## Consequences

收益：一份排法喂三处画键盘的地方，宽度不会在键盘和预览之间分叉；`equal` 逐格不变，默认的键宽和触摸区域和以前相同。占位格归属是 `KeyboardKeyArea` 上的通用机制，26 键缩进的修复能直接用。字距只在绘制时加，统计和读屏不受影响。

代价：`KeyboardKeyArea` 每次按下多一次子视图类型判断。默认键面多了字距，所有 14/18 键用户更新后都看得到。宽窄时 18 键单字母键的字号由 `setFittedKeyTextSize(12, 22)` 自适应收着，14 键的 L 仍是 22 号字，窄屏上要看真机效果。

验证：`FourteenKeyLetterWidthsSmoke` 钉住两种分法的份额（等宽逐格等于原来的份额，宽窄与 #11051 的比例相同、三排份额之和相同、补半键距的个数）；`FourteenKeyLayoutSmoke` 钉住哪些键面有字距、`L;` 用较小的字距、键面字符串不变；`KeyLayoutStylesPolicySmoke`、`KeyboardPreviewLayoutSmoke` 钉住两处预览（等宽与原来逐格相同，宽窄的空白、比例和同种键三行等宽，26 键和九键不受影响，皮肤页预览带着这项、换类型时保留）；`KeyPressAreaDeviceChecks` 在合成按键区上验证占位格和它的外边距归所属的键、没有所属键的占位格不回调；`check-host.sh` 钉住键盘和设置页的接线。API 35、API 28 模拟器上 `KeyPressAreaDeviceChecks`、14 键和 18 键的 `FourteenKeyDeviceSmoke` 通过；API 35 上手动看过两种宽度、14 键和 18 键、全拼和双拼、单手模式的键面，点了宽窄第二排两端的空白（归 A、AS、L）。
