# Agent Note: Android 14/18 键字母键的角标符号

Status: implemented

符号表、触发方式和各状态下的行为是三端共用的，见 [14/18 键字母键的角标符号与触发方式的三端规则](2026-11-07-fourteen-eighteen-key-corner-symbols.md)；本篇只记 Android 怎么画、怎么接触摸、怎么上屏。

## Problem

Android 的 14/18 键字母键是普通按键：点按经 `gridKey` 送整组；长按经 `ImeLayoutRows.showNineKeyHoldOptions` 弹出这一键的两个字母，选中前先发命令 2 结束组字，组字中长按 BU 会把「不」先上屏、再接一个 `u`；18 键双拼的两字母键另有一个 `OnTouchListener`（`bindEighteenKeySwipe` 和 `EighteenKeySwipePolicy.Gesture`）做横滑。要加上角标符号和 26 键的纵滑、反向选项、长按选项，又不能让同一个键上的两个触摸监听抢同一次按压。26 键的手势（`bindLetterGestures`）按候选栏可见判断「组字中」，这条规则在 14/18 键上不成立（见共享笔记）。

## Decision

### 符号表和键面

`GridLetterHintTable`（`platforms/android/java/app/msime/android/keyboard/`，不依赖 Android）按 `FourteenKeyLayout.Key` 查 14 键或 18 键的表；`face` 在中文标点生效时经 `ChineseSymbolFaces` 画中文形式（引号样式取 `punctuationMarks()`），`routesThroughEngine` 与 `LetterHintTable` 同一条规则。

`ImeLetterRows#fourteenLetterKey`（[14/18 键字母键宽度](2026-11-06-android-fourteen-eighteen-key-letter-widths.md)拆出的建键方法）建的字母键改成能画角标的 `KeyHintButton`（分词键已经是），双字母键面的字距不变。宽窄时第 2 排两端的占位格由 `KeyboardKeyArea` 把按下挪进同排最外侧的键，之后的移动挪同样的距离，所以从占位格起手的滑动和长按与从键内起手相同。右上角的位置、字号和颜色与 26 键相同。键和按钮按下标记在 `gridLetterButtons` / `gridLetterKeys` 里，建键时和每次 `render()`（`updateGridLetterCornerHints`）按「显示字母键角标」（`AndroidLocalSettings.LETTER_CORNER_HINTS`）和此刻的中文标点就地更新角标，切换中文标点不重建键行；设置页这一项的说明改成「26 键和 14/18 键字母键右上角…」。键面字符串、读屏念法和键位 id 都不变。

### 一个触摸监听

`bindGridLetterGestures` 是每个 14/18 键字母键唯一的触摸监听，判定交给 `GridKeyGesturePolicy`（不依赖 Android）：

- 按下时新建一个 `Gesture`：横滑字母只在双拼 18 键的两字母键上给（`fourteenShuangpin()`，每次按下按当前方案取），方向读 26 键同一对本机设置（`SWIPE_DOWN_SYMBOLS` 关着时传 null，`SWIPE_SYMBOLS_DIRECTION`），「组字中」取 `hasEngineComposition()`。按压中途另一根手指开始了组字，之后的事件调用 `composing()`，还没触发的纵滑和长按改按点按处理。
- 按下时的气泡：本篇落地时只有 18 键双拼的两字母键按「按键弹出」画；之后 [Android 14/18 键字母键按下画气泡](2026-11-08-android-grid-letter-press-bubble.md) 让每个 14/18 键字母键按下都画。纵滑出符号或横滑选中字母时，气泡换成符号或字母（`gridGesturePreview`），同样跟随「按键弹出」。
- 还没触发手势时监听返回 false，点按、按下态和长按计时都由按钮自己处理；触发后用 `cancelKeyTouch` 给按钮补一个取消，按钮不会再点击或长按。纵滑锁定后键面不变，按键气泡换成符号；横滑时键面和气泡换成选中的字母，与原来相同；反向纵滑打开选项。
- 长按经按钮的长按回调，读屏的长按动作走同一条：组字中返回 false，按钮松手照常点按这一组；空闲时打开选项。
- 松手：符号经 `ImeLayoutRows#commitGridLetterHint` 上屏；横滑选中的字母照旧经 `gridKey` 送大写精确标记；选项按 26 键的拖选收尾（`finishLetterOptions`），没拖进去时弹窗留着等点选；组字中没有触发手势的按压由这里补一次 `performClick`，即使位移让按钮自己取消了点击。多指、换指和系统取消什么都不输入。
- 一次滑出或选项里的选择、横滑都只记这个键一次（`countKey`），与 26 键相同。

旧的 `bindEighteenKeySwipe`、`EighteenKeySwipePolicy.Gesture` 和 `FourteenKeyLayout.holdLetters` 被它取代，已删除；`EighteenKeySwipePolicy.letter` 的阈值和方向规则不变，由 `GridKeyGesturePolicy` 调用。26 键的 `bindLetterGestures` 和它的候选栏门控不变。

### 选项和上屏

`ImeLayoutRows#showGridLetterOptions` 列出这一键的小写字母、大写字母和符号的键面（QW 是 `q w Q W ；`，L 是 `l L “`），一行最多五格，与 26 键的选项共用 `showChoiceOptions` 和拖选。选字母经 `commitNineKeyHoldOption` 上屏（空闲时命令 2 不改变什么，全角开关照旧生效）；选符号和滑出符号同一条路。

`commitGridLetterHint` 与 26 键的 `commitLetterHint` 共用 `commitHint`：中文标点生效、符号是 Engine 会转写的 ASCII（`( ) " ? ! : ;`）时交给 `MSIMEInputService#punctuation`，智能标点、成对补全和引号交替都照常生效；其余经 `commitNineKeyLiteral` 按字面上屏，`¥` `…` `、` 不会被 ASCII 上限丢掉。`;` 经 `punctuation` 上屏，不经 `type()`，所以碰不到 123 层把 `;` 当辅助码触发键的那条路（`ChineseHelpcodePolicy.layerTrigger`）；组字中纵滑和长按本来就不出符号。

### 检查

接线不在 `check-host.sh` 里加源码检查（AGENTS.md「代码健康」）：判定在 `GridLetterHintTableSmoke`、`GridKeyGesturePolicySmoke` 两个 JVM 冒烟里，字母键画不画角标、滑动和长按出什么、组字中是否按点按，由设备专项 `GridLetterSymbolDeviceChecks` 用真实触摸验收，见下面的验证。

## Alternatives considered

- **保留 18 键双拼的横滑监听，另加一个处理纵滑和长按的监听。** 横滑的代码一行不动。不用：一个 `View` 只有一个 `OnTouchListener`，两个监听只能嵌套调用，谁先接管、谁给按钮补取消就要在两处各写一份；纵滑和横滑的条件、锁定和取消要在同一个状态里判，才能保证一次按压只出一种结果。
- **直接复用 26 键的 `bindLetterGestures`，按键查 `GridLetterHintTable`。** 少一个类。不用：它按候选栏可见判断组字中，横滑和精确字母不在里面，键的字母也只有一个；改成同时服务两种键，26 键的门控和测试都要跟着动。
- **长按用触摸监听自己的计时（像 26 键的 `postDelayed`），不用按钮的长按回调。** 和 26 键写法一样。不用：读屏的长按动作只走按钮的长按回调，设备套件也用它验收长按；用回调后组字中返回 false 就是「按点按处理」，不用另写分支。

## Consequences

收益：14/18 键的符号、选项和横滑在一个状态里判，一次按压只有一种结果；组字中长按不再先结束组字（#11040 第 2 项的 Android 部分）。符号上屏和 26 键是同一条路，中文标点、成对补全和全角的行为一致。

代价：14/18 键字母键从普通按键换成 `KeyHintButton`，字号自适应和角标在 18 键的窄键上要看真机效果。纵滑触发时按钮的按下态马上撤掉（26 键要到松手），按键气泡仍显示符号。读屏的长按在组字中不做事。

验证：`GridLetterHintTableSmoke` 钉住两张表逐键相同、每键一个、没有数字、键盘内不重复、14 键没有 `% * & ¥`、中文和英文标点的键面、直角引号和 Engine 路由；`GridKeyGesturePolicySmoke` 钉住阈值、纵向为主、两种方向、开关关闭、反向选项、空闲长按、组字中按点按（含按压中途开始组字）、横滑重选和回中心、先触发者锁定、多指和换指取消；`EighteenKeySwipePolicySmoke` 只留横滑判定本身。`FourteenKeyDeviceSmoke` 和 `EighteenKeyQuanpinDeviceChecks` 改成验收单字母键 L 也有选项、两字母键的选项里有大写字母和符号。WSL 的 `check-host.sh` 通过（`TimestampDisplayPolicySmoke` 在 JDK 21 上的既有失败除外），设备套件 275 个源文件编译通过。设备专项 `GridLetterSymbolDeviceChecks`（`FourteenKeyDeviceSmoke -e grid_symbols true`，接进 `smoke.sh` 的完整路径）用真实触摸覆盖：缺省上滑出 `？：、；~/`，滑出后回到起点仍出符号；反向下滑打开选项并点选；组字中上滑和长按各算一次点按、不弹选项，接着选「你好」；上屏后的联想里上滑出 `！`；空闲长按 L 选大写；英文标点出 `?:`；下滑设置下下滑出符号、上滑开选项；关掉「滑动输入符号」后上滑是点按、长按照旧出选项；18 键全拼出 `¥%*…、（`，单字母 Q 反向开选项选 `；`，组字中上滑是点按。API 35（emulator-5580）和 API 28（emulator-5566）上这一项和 14 键（全拼、双拼、五笔）、18 键全拼、18 键双拼的原有专项都通过。真机键面另看截图。
