# Agent Note: 九键底行改为 符 起头，1 键把常用标点放进候选条

Status: implemented

## Problem

拼音九键和笔画键盘共用的九键外框，底行在 iOS 和 HarmonyOS 上是 `123 | 空格 | 0 | 中 | 回车`，Android 是 `123 | 空格 | 中/英 | 回车`。产品方要求底行统一成搜狗式的 `符 | 123 | 空格 | 中 | 回车`：字母层上的 0 用处很小，符号面板应该有一个固定在底行的入口。同时，网格第一格（`@#`，即 1 键）点按时打开整块符号面板，产品方要的是常用快捷符：点一下，常用标点直接出现在候选条里，点哪个上屏哪个，不离开键盘。

## Decision

Android 数字层的 0 与空格位置由[底行中间的数字 0](2026-10-16-android-nine-key-numeric-bottom-row.md)取代；下文保留此前布局的理由，其余决定继续成立。

三端（iOS、Android、HarmonyOS）同一套行为：

- **底行**：拼音九键和笔画键盘的底行是 `符 | 123 | (地球) | 空格 | 中/英 | 回车`。符 占原先 0 的那一份宽度（相对空格 4.2 的 1.05，与 中/英 相同），点按打开符号面板，打字统计记符号键。26 键、14 键、假名、注音、手写的底行不变。
- **数字层的 0**：iOS 和 HarmonyOS 的拼音网格数字层只有 1–9，底行那个键是唯一的 0，所以数字层的底行保持 `123 | 空格 | 0 | 中 | 回车`，不画 符。Android 的 0 在网格右列，底行在两个键面上都是同一行，符 在数字键面也在。
- **1 键（`@#`）**：空闲时点按，候选条换成当前模式的常用标点，就是 ， 键长按那一组（中文模式 `， 。 ？ ！ 、 ； ：`）。点选一个标点，走与标点键相同的路线写出（中英文标点、成对标点、智能标点照常），候选条清空。组字中点选与按侧栏标点一样，先上屏首选再写标点。任何按键带来的下一次 Engine 快照用它自己的候选盖掉这组标点。Android 组字时 1 键仍是「分词」，不变；iOS 和 HarmonyOS 的分词在右列中间键，1 键没有组字形态。长按 1 仍输入数字 1。
- **实现位置**：iOS `KeyboardViewController` 的 `nineKeyQuickSymbols` / `showNineKeyQuickSymbols`，`ActionRowStyle.nineKey` 与 `nineKeyActionWidths`；Android `KeyboardActionRow.designEntries` 的 `DesignSlot.SYMBOL_PANEL` 与新类 `ImeNineKeyQuickPunctuation`（照 `ImeEmailSuffixes` 接进候选条，`render()` 时清空）；HarmonyOS `NineKeyLayout.actionRow(digitLayer)` 与 `KeyboardView.ets` 的 `nineKeyQuickMarks`。

这一篇翻转了此前「九键底行不再有 符 键、面板入口是网格的 @#」的做法，和 [手机九键保留符号入口的数字 1](../bug-fix/2026-10-13-mobile-nine-key-one-hint.md) 里「第一格短按仍打开符号面板」「iOS 和鸿蒙底行的 0」两条事实；该篇其余决定（1–9 数字提示与长按、字号）仍成立。

## Alternatives considered

- **整行照搬、数字层也去掉 0**：底行在两个键面上完全一致，实现最简单。不采用：iOS 和 HarmonyOS 的数字层就打不出 0 了，九键数字层要靠底行这个键补 0。
- **1 键保留打开面板，只在底行加 符**：改动最小，两个入口都开面板。不采用：产品方要的是不离开键盘的常用标点，两个键做同一件事也浪费一格。
- **常用标点另起一张表**：可以按九键单独挑选更多标点（…、～、@）。不采用：键盘已有一组按模式区分的常用标点（， 键长按菜单），另起一张表会让两个入口给出不同的「常用」，以后改一处漏一处。

## Consequences

- 收益：符号面板有了固定在底行最左边的入口，三端一致；1 键一点就能上屏常用标点，不用打开整块面板再关掉。
- 代价：字母层失去底行的 0（数字层仍有）；候选条上的常用标点在下一次 Engine 快照时就被替换，包括按键之后晚到的云候选、AI 候选或翻译，所以刚打完字立即点 1，标点可能被晚到的结果盖掉，再点一次即可。
- Android 笔画网格最后一行的 `@#` 仍打开符号面板，与底行 符 重复；HarmonyOS 2in1 屏幕键盘的底行（与假名网格共用）仍是 `123 | 符 | …`。两处都不在这次范围内。
- 14 键的「符/分词」键沿用 1 键旧行为的说法（打开面板）不再完全成立，14 键本身没有改。

## Verification

- iOS：`MSIMEClientTests` 在 iOS 27 模拟器上通过，新增 `testNineKeyOneKeyPutsQuickSymbolsInTheCandidateStrip`，改写 `testNineKeyKeepsItsOwnBottomRow`（两个键面的顺序和宽度）与通过 符 开面板的用例；真机签名构建并安装到 iPhone 17，真机交互待产品方确认。
- Android：`check-host.sh` 通过（231 项 JVM 冒烟，设备用例源码编译），签名 APK 构建与 `verify-native.sh` 通过；`KeyboardActionRowSmoke` 覆盖九键与笔画底行顺序（有无地球键、交换语言与数字键），`NineKeyEnglishDeviceSmoke` 新增点 `@#` 后选标点和 符 开关面板两段。模拟器被其他会话占用，设备用例没有运行。
- HarmonyOS：`tests/run.sh`、`scripts/test-harmony-*.py` 通过，`hvigorw assembleHap --no-daemon` 构建成功；没有设备，未做真机验收，HAP 不含原生库。

## Related notes

检索九键、底行、符号入口的活跃笔记：[手机九键保留符号入口的数字 1](../bug-fix/2026-10-13-mobile-nine-key-one-hint.md) 被本篇部分取代（见上），双方互链；[九键路径排序](../bug-fix/2026-10-08-nine-key-path-ranking.md) 里「Android 不组字时 1 键打开符号面板」的事实被本篇取代，组字时分词仍成立；[Android 九键默认符号](2026-10-13-android-nine-key-default-symbols.md) 说「其余标点从左上角符号面板取得」，现在面板入口在底行最左边，左栏三项的决定不变；[手机语言键位置](2026-10-09-mobile-language-key-beside-return.md) 的 中/英 紧挨回车仍成立；[Android 语言与数字键交换](2026-10-14-android-language-number-key-swap.md) 的交换在九键底行照常生效，符 不参与交换。
