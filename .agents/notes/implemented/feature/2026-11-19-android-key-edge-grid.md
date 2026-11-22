# Agent Note: Android 各键盘按 20 列外框对齐键边

Status: implemented

## Problem

Android 的按键行是按权重分宽的 `LinearLayout`，键距是每个键左右各半个键距的外边距。`LinearLayout` 先从整行扣掉所有键的外边距，再按权重分剩下的宽度，所以键数不同的行，同样权重的键不一样宽，各行同一列的键边对不齐：411 dp 宽、键距 6 dp 时，26 键三排字母约 34.3 / 35.0 / 35.8 dp，第二排也不是正好错开半个键。14 键「宽窄」第三排因此比前两排窄约 1.5 dp，靠占位格朝外补键距也补不全。底行另有一张权重表，26 键的 123 比 ⇧ 窄约 5.7 dp，↵ 的左边比 ⌫ 靠左约 17.5 dp；九键一族有四种外框比例（拼音九键 15/70/15，笔画 18.75/62.5/18.75，注音九键、日文九键又各一种）。

## Decision

- **先按份额切格，再从格里扣外边距。** `KeyCellEdges`（纯 Java）按「宽度 × 前缀份额 / 总份额」逐条取整算出一行各格的边，最后一条边恰好是行宽；键宽是格宽减去这个键自己的左右外边距。`KeyRowLayout`（`LayoutPolicy.keyRow`）和横向的 `KeyboardKeyArea`（底行）在 `LinearLayout` 照常量完一遍之后，按格把每个子视图重新量宽、放回自己的格里；高度和纵向位置仍由 `LinearLayout` 定。键距仍是外边距，皮肤、按下动画、键位预览、按下统计和键面看到的仍是画出来的那个键；空隙里的按下照旧由 `KeyboardKeyArea` 交给拥有那段空隙的键，空隙在两个键之间对半分，正好是格的边界。
- **份额之和相同、宽度相同的行，同一列的键边落在同一像素上。** 整个键区按 20 列外框计宽：一个字母键是 1 份（2 列），每一行合计 10 份（`KeyboardActionRow.ROW_SHARES`）。
  - 26 键：⇧、⌫ 各 1.5 份（3 列），第二排两端缩进各 0.5 份；底行 123 和回车各 1.5 份，在 ⇧、⌫ 正下方，符、。、，、中/英各 1 份，在第三排字母键正下方。
  - 空格占一行剩下的份额（`withSpaceFill`）；样式和地球键多出的键都从空格让出，其余键的位置和宽度不变。
  - 14 键「等宽」第三排：分词和 ⌫ 与 26 键 ⇧、⌫ 同宽，字母键平分中间（1.75 份）；18 键不变。「宽窄」的份额本来就与网格一致。
  - 123 / #+= 层：#+= 和 ⌫ 在 ⇧、⌫ 的位置，五个标点平分中间；底行返回与 123 同一格，回车在 ⌫ 下面。
  - 九键一族（拼音九键、九键双拼、笔画、注音九键、日文九键）统一为 3 | 14 | 3 的外框（`NineKeyLayout`）。九键底行对齐九键自己的列，不与 26 键的底行对齐：符 和回车各占一个侧列，123、中/英和样式多出的键各占九宫格半列。
  - 借给 26 键、14/18 键、大千注音和手写的数字九宫格，底行按来源键盘的份额排（`inLetterShares`），切层时返回、符、中/英和回车不动。
- **只改 Android。** iOS、HarmonyOS 和 `packages/ui` 的设置预览仍是原来的份额。手写底行三端由 `scripts/test-mobile-handwriting-pad-parity.py` 钉成同一组份额，这次不动；三端一起改时，手写底行也按「各键 2 列、空格让出」排。

## 不变量

- 要对齐的行 must 合计相同的份额；只要各行份额成比例，单位可以不同（九键外框用 3 | 14 | 3，底行按 10 份换算）。
- 新的按键行 must 用 `LayoutPolicy.keyRow` 建；有固定宽度子视图的行由 `KeyRowLayout` 照 `LinearLayout` 原样排，不按格量。
- 键距 never 再靠占位格朝外补外边距：按格量之后补了反而错。原来 14/18 键宽窄的补法（`KeyboardSpacer.setOuterHalfGaps`、`FourteenKeyLetterWidths.padHalfGaps`）已删去。
- 底行新增或删去一个键时只改条目，空格 must 由 `withSpaceFill` 补齐，不另写空格常量。

## 例外

- 符号层的字母行（注音、笔画、手写的符号键面）第三排有 10 个键，与 10 份的行对不齐，⇧/⌫ 份额照旧（`letterRowEdgeWeight`）。
- 九键双拼的 0 仍另加一份（九宫格半列），这一行比外框宽；底栏带标点又有地球键时，空格让不出这一份。
- 地球键从空格让出一份后，分离式底行在逗号隐藏时空隙最多偏开 1.5 个键；Android 9 起宿主不画地球键，这种排法只在 JVM 冒烟里出现。

## 参考

键距放在键格之内、按整行宽度的单位计宽，参考了 AOSP LatinIME（[Key.java](https://android.googlesource.com/platform/packages/inputmethods/LatinIME/+/refs/heads/main/java/src/com/android/inputmethod/keyboard/Key.java)）和 FlorisBoard（[TextKeyboard.kt](https://github.com/florisboard/florisboard/blob/release/0.5/app/src/main/kotlin/dev/patrickgold/florisboard/ime/text/keyboard/TextKeyboard.kt)）；LatinIME QWERTY 的 ⇧、⌫、?123、↵ 也是 15% 宽。这里按本仓的需求改成所有键盘共用的 20 列外框，没有照搬它们的实现。

## Alternatives considered

- **键盘改用 XML 定义（LatinIME 的做法）。** 布局一目了然，宽度直接写成整行的百分比。不用：本仓的键盘按设置（样式、地球键、互换、语言）在 Java 里拼出来，改 XML 是重写所有键盘，动态的行仍要代码。
- **`GridLayout` 或 `ConstraintLayout`。** `GridLayout` 20 列、键跨列，边天然对齐，是最接近的现成控件。不用：18 键、大千注音和九宫格中间三列不能整分 20 列，要嵌套；行高角色（`KeyboardHeightRole`）挂在行视图上；每个键盘的视图树都要重写。`ConstraintLayout` 同样要重写，测量也更重。
- **把键距画进键视图（外边距为 0，键自己内缩），用原生 `LinearLayout`。** 不需要任何测量代码，模拟下边差 1 px 以内。不用：所有读键矩形的地方（皮肤、按下动画、预览锚点、按下统计、键面计算）都会看到格而不是键，改动散到很多文件，每种皮肤都有可见风险。
- **保留权重，用占位格补键距。** 14/18 键宽窄就是这样做的。不用：每行要补多少取决于这一行有几个键，补法已经在宽窄第三排漏了 1.5 dp，换到每种键盘只会更乱。
- **先单独发一个不改画面的 PR，把份额收进一个新的行计划类。** 评审时能先确认「画面不变」。不用：调查后发现大部分份额本来就是数据（`KeyboardActionRow`、`FourteenKeyLetterWidths`、`KeyboardLayout`、`NineKeyLayout`），这个类只会是装着旧份额、下一个 PR 就改掉的过渡层。
- **三端一起改。** 三端现在共用同一组份额。不用：iOS、HarmonyOS 的布局各有自己的机制，是否跟进由维护者决定；这次只改 Android，手写底行和共用预览保持三端一致。

## Consequences

- 收益：各行同一列的键边在任意宽度、密度下都落在同一像素（模拟器 1080 px 上与示意图逐像素一致）；123、中/英、↵ 在 26 键、14/18 键和大千注音上占同样的列；份额只剩一套规则，空格自动补齐，不再维护一组空格常量。
- 代价：可见改动——26 键 ↵ 约 67 dp → 55 dp，拼音九键底行 123 约 51 dp → 41 dp、↵ 约 77 dp → 55 dp，笔画、注音九键、日文九键的外框比例变了。Android 与 iOS、HarmonyOS 的底行份额暂时不同，`packages/ui` 的预览也与 Android 不同，直到另两端跟进。
- 测试：`KeyCellEdgesSmoke` 钉住切格与对齐；`KeyboardActionRowSmoke` 钉住每种键面、样式、有无地球键的底行都合计 10 份；`FourteenKeyLetterWidthsSmoke`、`KeyboardLayoutSmoke`、`TouchKeyStyleSmoke`、`SplitKeyboardPolicySmoke` 改成新的份额。设备套件 `KeyRowEdgeDeviceChecks`（`OptionSheetDeviceSmoke -e key-feedback row-edges`）用真实的 `KeyRowLayout` 和横向 `KeyboardKeyArea` 核对每个键在自己的格里、同一列的边像素相同。
- 相关笔记：[14/18 键字母键宽度](../feature/2026-11-06-android-fourteen-eighteen-key-letter-widths.md)的补键距由本篇取代；[触屏按键布局样式](../feature/2026-10-27-touch-key-layout-styles.md)、[中/英放在回车左边](../feature/2026-10-09-mobile-language-key-beside-return.md)里的 Android 份额改为本篇的规则；[26 键缩进死区](../bug-fix/2026-11-09-android-twenty-six-key-indent-dead-zone.md)的占位格归属不变。
