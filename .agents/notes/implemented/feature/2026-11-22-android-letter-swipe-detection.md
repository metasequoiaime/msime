# Agent Note: Android 26 键与 14/18 键字母键滑动判定统一

Status: implemented

## Problem

#11822 第 3 项。Android 26 键的 `bindLetterGestures` 用几个局部数组自己判定滑动，14/18 键用 `GridKeyGesturePolicy`，同一个「滑动输入符号」有两套判定，三处不同：

- 26 键只看纵向位移，斜着滑也出符号；14/18 键要求纵向位移大于横向。
- 26 键纵移超过 14 dp 却没触发手势（关掉滑动、没滑到「滑动距离」、英文反方向）又移出了键帽时，按钮自己取消点击，这次按键什么都不输入；14/18 键补一次点按。[Android 字母键「滑动距离」](2026-11-18-android-letter-swipe-distance.md) 当时有意让 26 键不补。
- 14/18 键把 `ACTION_UP` 那一点也交给判定，抬指时触点一跳就能单独触发符号或改选字母；26 键只看移动。

## Decision

- 26 键字母键改用 `GridKeyGesturePolicy.Gesture`，类名不变（鸿蒙的同一判定也叫这个名字）。新增两项按键选项：`Gesture(exactLetters, hasSymbol, oppositeOptions)` 的 `oppositeOptions` 只在中文为真，英文和按大小写写字的方案（越南语、藏文）反方向只是一次点按；`holdSymbol()` 让按大小写写字的方案长按直接出角标。
- 判定因此与 14/18 键相同：沿设定方向超过「滑动距离」且 |dy| > |dx| 才出符号；超过 14 dp、纵向为主却没触发手势的按压由监听补一次 `performClick`（`forcesTap()`）。抬起时离键太远，这次补的点击不带按下点，与原来候选态补点按相同。
- 26 键监听只报按下的那根手指，手指数报 1：按 `KeyboardKeyArea` 的说明，键距外落下的第二根手指会被框架并进第一根手指所在的键。14/18 键遇多指取消这次按压，26 键双手快打时不能这样。
- 两种键盘都只按 `ACTION_MOVE` 判定，14/18 键松手时不再调用 `move()`。
- 26 键原有的长按计时（`postDelayed`，查 `isPressed()`）、候选栏门控（`SwipeHintPolicy.tapsOnly`；按压中途出现候选栏时经 `composing()` 改按点按）、按下点记账和气泡行为不变。只被旧 26 键代码调用的 `SwipeHintPolicy.enabled` 删除。

## Alternatives considered

- **给 26 键另写一个判定类，或把 `GridKeyGesturePolicy` 改成中性的名字**：名字更贴切。不用：两份判定会再次分叉；改名让 Android 和鸿蒙的同名类对不上，diff 也更大。
- **26 键保留只看纵向，只补上丢键**：26 键手感改动最小。不用：斜滑误出符号是 #9256 那类误触的来源之一，14/18 键已经要求纵向为主，两套判定对不上的地方正是 #11822 要收掉的。
- **26 键也遇多指取消**：与 14/18 键逐字相同。不用：理由见上，键距外的第二根手指会被并进这个键。

## Consequences

收益：同一个开关、方向和距离在 26 键和 14/18 键上判定相同；26 键斜滑不再误出符号，纵移出键帽不再丢键；抬指时触点一跳不再单独出符号或改选字母。

代价：26 键手感有变化，横向不小于纵向的斜滑以前出符号，现在交给按钮（移出键帽时照样由按钮取消，不补点按），需要维护者在 #11822 确认。

验证：`GridKeyGesturePolicySmoke` 增加 26 键的反方向不开选项、斜滑、长按出符号、没有角标的键补点按。设备上 `LetterHintPunctuationDeviceSmoke -e letter-swipe true` 增加 26 键「滑动距离」48 时上滑 40 dp 移出键帽仍输入字母、横 16 dp 上 15 dp 的斜移输入字母；`FourteenKeyDeviceSmoke -e grid_symbols true` 增加移动 4 dp、只在抬起时跳到 24 dp 是点按。

## 历史笔记审计

- [Android 字母键「滑动距离」](2026-11-18-android-letter-swipe-distance.md)：部分取代。「26 键不补」和备选「26 键没滑到距离时也补点按」的取舍由本篇翻转，那篇已加链接。
- [14/18 键角标符号](2026-11-07-fourteen-eighteen-key-corner-symbols.md)：部分重叠。「纵向为主这一条 26 键没有」在 Android 上不再成立，那篇已加链接；iOS、鸿蒙不变。
- [Android 14/18 键角标符号](2026-11-07-android-fourteen-eighteen-key-corner-symbols.md)：部分重叠。「26 键的 `bindLetterGestures` 不变」已就地改为链接本篇。
- [Android 字母滑动选项](../bug-fix/2026-10-21-android-letter-option-gesture-settings.md)：事实更新。候选态门控改由 `GridKeyGesturePolicy` 的点按状态表达，`SwipeHintPolicy.enabled` 已删，那篇已就地改正。
- [三端 26 键默认上滑](2026-10-25-mobile-qwerty-swipe-symbols.md)、[滑行输入引擎](2026-10-07-glide-typing-engine.md)：无关。默认方向不变；滑行只看横向跨键，与纵滑判定不冲突。
