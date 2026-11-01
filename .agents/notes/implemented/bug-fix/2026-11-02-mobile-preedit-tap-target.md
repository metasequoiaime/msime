# Agent Note: 手机键盘放大读音行的点按区域

Status: implemented

## Problem

#10474：Android 上想改刚打的拼音，要点候选栏上方那行读音，打开放大编辑条。读音行设计高 14 dp，能点中的只有读音文字本身：`ImeToolbar.buildCandidateHeader` 里读音 `s.preedit` 是宽高都 `WRAP_CONTENT` 的 `TextView`，点击和长按只挂在它上面，打 `wo` 这样的短拼音时可点的地方只有十几 dp 宽；读音行底部和候选行之间那 3 dp（`READING_GAP_DP`，读音行的下内边距）也点不到。往旁边或往下点偏一点就落空，往上点偏则落到键盘外面的应用里。

iOS 同样：读音按钮 `preeditButton` 宽度贴着标题，只在 `compositionRow` 里竖直居中，读音并入候选行时按钮比候选行矮，单独一行（14 键）时右边空着的一整段点不到。

HarmonyOS 手机的读音行（`KeyboardView.ets` 的 `candidateBar`）没有 `onClick`，本来就没有「点读音编辑拼写」的入口，不在这次范围里，另开 issue 跟进。

## Decision

**只改命中，不改布局。** 行高、键盘高度、读音和候选的位置都不变，放大的只是能把触摸交给读音的那块区域。

- **Android**：读音行 `candidateHeader` 挂一个 `TouchDelegate`，目标是读音。区域是读音所在那一段（`preeditFrame`，从行首到右边的提示、页码、漢、× 为止）的整个宽度，靠行首一侧延到行的边缘（含 10 dp 左内边距），上下占满整行（含行底的间距）。每次读音行重新布局时按 `preeditFrame` 的当前位置重算（`addOnLayoutChangeListener`，与 `InfoTipView.extendTouchTarget` 同一个写法）。子视图先分发，只有子视图都不接的触摸才到 `TouchDelegate`，所以提示、页码、漢、× 和 18 键的拼音选择条（插在 `preeditFrame` 后面、各占一半）照旧接自己的触摸。读音能不能点、能不能长按仍由它自己的 `clickable` / `longClickable` 决定（`MSIMEInputService` 组字时按 `caretEditable`、`offersLocalModes` 设置），不能编辑时读音不接，宽区域也不吞触摸。
- **iOS**：读音按钮换成 `KeyboardViewController.PreeditButton`，重写 `point(inside:with:)`，可点区域是它的父视图 `compositionRow` 的整个范围。读音单独一行时是这一行从行首到行尾、上下占满；拼音选择条显示时叠在读音按钮上面（后加进 `compositionRow`），落在选择条里的触摸先归选择条。读音并入候选行时 `compositionRow` 只有读音那么宽，放大的是上下，占满候选行的高度。读音收起（`isHidden`）时不参与命中。

**往上扩不了。** 读音行就是键盘可触摸区域的最上沿（Android 停靠时 `onComputeInsets` 的可触摸区域只有键盘本身），再往上是应用的窗口。往上点偏落到应用的情况只能缓解，消除不了。

## Alternatives considered

- **空格左边第一个键上滑进入拼音编辑（报告人的提议），附带顶部 toast、设置开关默认开、再滑一次退出、高亮「完成」** — 最强的理由是这个入口在键区里，离拇指近，完全不受读音行高度限制。不做：空格左边是哪个键，随 9/14/18/26 键和底行样式（含用户自定义底行）变化，按位置固定一个手势在各布局之间维持不了一致，还要三端同做；它是看不见的隐藏手势，默认开启改变所有人的默认行为；键盘的提示已经从 Toast 改到诊断行，因为读屏会念出 Toast，再加顶部 toast 方向相反。Android 已有更大的入口：「拼音显示位置」选「输入框」时，点输入框里正在组的拼音同样打开放大编辑条（`editCompositionSelection`）。
- **Android 把读音 `TextView` 改成 `MATCH_PARENT` 撑满 `preeditFrame`，靠 gravity 保持文字位置** — 不用 `TouchDelegate`，结构更直观。但读音还画品牌药丸背景（`ImeStyler` 里 `brandPillVisible` 时设背景和内边距），撑满后背景会横跨整行；它也盖不到行底那 3 dp，那是 `candidateHeader` 的内边距，仍要 `TouchDelegate` 补。
- **iOS 改约束：按钮上下贴住 `compositionRow`，再加 `width >= 44`** — 按钮本身就变大，命中自然跟着变。但 `width >= 44` 在读音并入候选行时会把候选往右推，短拼音时候选位置随之变化；读音收起时 `compositionRow` 被约束成 0 宽，和 `>= 44` 冲突；单独一行时还要处理和拼音选择条分宽的优先级。改命中不碰这些约束。

## Consequences

- 短拼音也点得中：Android 读音左边整段加行底间距，iOS 单独一行时的整行、并入候选行时的整段高度，都能打开拼写编辑。
- Android 长按打开本地输入模式菜单的范围同样变宽，只在 `offersLocalModes` 时生效。iOS 单独一行在空闲时若显示「水杉输入法」和本地输入菜单，点这一行的空白处也会打开它。
- 测试：iOS `CompositionCaretTests.testTappingTheSpellingOffersHomeEndAndDelete` 断言读音并入候选行时这一段上中下都命中读音；`FourteenKeyKeyboardTests` 断言五笔 14 键（没有选择条）读音行最右端命中读音，全拼 14 键的拼音选择条里的触摸仍命中选择条。Android 的 `TouchDelegate` 区域没有 JVM 可测的纯逻辑，只经 `check-host.sh` 编译和静态检查，点按效果要在设备上看。
- HarmonyOS 手机没有「点读音编辑」入口，三端在这一点上仍不一致，由后续 issue 补齐。
