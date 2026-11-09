# Agent Note: Android 14 键拼音选择挂到读音行

Status: implemented

## Problem

#11040 第 1 项：Android 14 键还保留字母区左侧的拼音栏，空闲时显示 `，` `。` `？`，有拼音可选时换成纵向拼音列，三排字母为它让出约 15.6% 的宽度。同一个 Android 键盘上，18 键的拼音条自 #10401 起在读音行右边，iOS、鸿蒙的 14/18 键也都在读音行；只有 Android 14 键挑拼音要往左看，换一个键面就换一套手感，14 键的字母键也因此比 18 键窄。

## Decision

Android 14 键和 18 键走同一条挂法：`ImeLetterRows.rebuildFourteenKeyRows` 对两者都调 `ImeLayoutRows.attachGridSpellingsToReadingRow`（原 `attachEighteenSpellingsToReadingRow`），拼音条挂在读音行、读音右边，读音和拼音条各占半行，没有拼音可选时拼音条收起、读音占满整行。字母区左侧不再画符号栏，三排字母从同一左边界铺满整行；`FourteenKeyLayout.SPELLING_SIDEBAR_WEIGHT`、`LETTER_COLUMN_WEIGHT` 和 `ImeLayoutRows.fourteenSpellingSidebar` 删除。

- 适用全拼、双拼和五笔的 14 键：三者是同一副键面。五笔的 `nine_key_spellings` 恒为空，读音行没有拼音条，编码占满整行；原来的侧栏在五笔下只剩符号。iOS 的五笔 14 键同样没有侧栏。
- 空闲时的 `，` `。` `？` 随侧栏去掉，符号入口是第三排左端的「符」（组字时是「分词」），与 18 键相同。共享偏好 `touch_nine_key_sidebar_symbols` 只给拼音九键、九键双拼和笔画的字母层；设置页本来只在 9 键分段编辑它，不改。
- 候选不受影响：`MSIMEInputService` 只为注音 9 键把候选让成不可见，14 键组字时候选与拼音条同时可见、可点，与 18 键相同。
- #11016 的网格辅助码整串解释用的是同一个 `nineKeySpellings`，随拼音条一起挪到读音行。
- `SpellingPlacement.SIDEBAR` 只剩九键使用；`s.nineKeySidebar` 每次重建键行先清空，14 键不再建它，`applySidebarRail` 和 `sidebarStale` 对 14 键直接返回。
- 设置页预览（`KeyLayoutStylesPolicy.fourteen`）跟着键盘去掉左边那一格，14 键与 18 键一样三行按整行缩放。#11051 的「宽窄」按行计算份额，不受影响。

### 历史笔记检索

- [Android 14 键与九键共用左侧栏](../bug-fix/2026-10-13-android-fourteen-key-left-spellings.md)：本篇取代它在 Android 上的拼音位置决定，两篇互链。它的理由（读音行的横向拼音条难点按、当时用户要求沿用九键交互）在下面「备选方案」里回应。
- [Android 14 键独立换行拼音区](../bug-fix/2026-10-13-android-fourteen-key-spelling-grid.md)：已被上一篇取代，不再相关，不改。
- [全拼 14 键](2026-10-10-fourteen-key.md)：部分重叠。它的三端规格本来就是读音行横向选择，Android 现在回到这一条；更新其中指向左侧栏的两处。
- [Android 14 键双拼](2026-10-23-android-fourteen-key-shuangpin.md)、[Android 十八键全拼入口](2026-10-24-android-eighteen-key-quanpin.md)：部分重叠，只改其中「沿用侧栏」的事实。
- [五笔 14 键](2026-10-25-wubi-fourteen-key.md)：部分重叠，它说的「左侧栏始终是符号」随本篇失效；其余规则不变。
- [Android 14/18 键字母键宽度](2026-11-06-android-fourteen-eighteen-key-letter-widths.md)：无关，宽度分法按行算，不看侧栏。

## Alternatives considered

- **保留左侧栏（2026-10-13 的决定）**：最强的理由是点按目标大，侧栏每项 36dp 高，而读音行只有十几 dp；当时也是用户明确要求的九键交互。但 #11040 要的正是三端和两种网格键面一致，18 键在 #10401 之后已经在读音行上用了这条拼音条，iOS、鸿蒙一直如此；侧栏还让 14 键字母键比 18 键窄约 15.6%。读音行的高度和点按区域是 14/18 键和三端共有的问题，要放大应该一起改，不在 14 键上单独保留另一种位置。
- **只给全拼、双拼去掉侧栏，五笔 14 键保留**：五笔没有拼音可选，侧栏在那里只是一列符号，去掉它确实不换来拼音位置上的统一。但三种方案共用一副 14 键键面和一张设置预览，预览按键面类型画、不分方案；保留就要为五笔单开一条布局分支和一张预览，iOS 的五笔 14 键也没有侧栏，所以不采用。

## Consequences

- Android 14 键与 18 键、iOS、鸿蒙的拼音位置一致；14 键三排字母变宽，与 18 键一样铺满整行，空闲和组字时键位都不动，键盘高度不变。
- 代价：空闲时一键输入 `，` `。` `？` 的入口没了，要走「符」或底行的逗号句号；拼音条的点按目标回到读音行的高度，与 18 键相同。
- 设备验收 `FourteenKeyDeviceSmoke` 改成核对：空闲时没有「符号栏」、第一排与第三排同一左边界；组字时拼音条在读音右边、在候选和字母键上面且不与候选相交；窄屏上拼音条横排不出屏幕。五笔专项改成核对没有「符号栏」。
