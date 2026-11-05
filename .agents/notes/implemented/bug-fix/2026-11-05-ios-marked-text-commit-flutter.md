# Agent Note: iOS 上屏显式替换标记区

Status: implemented

## Problem

#10873 报告 iOS 键盘在闲鱼的搜索框和聊天框里选词后，拼音和汉字同时留在输入框；#10843 的 B 站评论区「拼音也被打上去」已并入同一 issue。iOS 键盘的行内预编辑把拼写作为 marked text 写进宿主，选词上屏原来直接调用 `insertText`，依赖宿主自己替换标记区。

Flutter 引擎的 `FlutterTextInputView.insertText` 在 flutter/flutter#191062（2026-09-04 合入 master）之前一直写作 `[self replaceRange:_selectedTextRange withText:text]`：只替换当前选区，并在 `replaceRangeLocal` 里把 `markedTextRange` 清成空。键盘把标记区内的选区放在拼写末尾，于是正文插在拼音后面，拼音变成普通文字。本机 Flutter 3.44.0（2026-05-15）的引擎源码仍是这一写法；同一引擎的 `setMarkedText` 则先替换现有标记区，没有标记区时才替换选区。闲鱼是 Flutter 应用，B 站评论区是否也是 Flutter 宿主没有核实。

## Decision

`InlineCompositionPolicy.commit` 决定运行时上屏怎样替换行内预编辑，`KeyboardViewController.insertOwnText` 只按结果调用代理：

- 指定了光标后缀：沿用快捷短语定位的 `setMarkedText(正文, 选区)` 加 `unmarkText()`。
- 标记区非空、同一快照上屏后不再组字：`setMarkedText(正文, 末尾)` 加 `unmarkText()`，由 `setMarkedText` 显式替换标记区。遵守 UIKit 约定的宿主里 `unmarkText` 只是定稿，正文只写一次。
- 标记区非空、同一快照还接着组字（部分选词、韩语等就地组字的下一个音节）：仍然 `insertText`，随后照常标记剩余组字。
- 没有标记区：`insertText`，与原来相同。

`render` 以快照的 `preedit` 是否非空判断「还接着组字」，它正是随后 `showInlineComposition` 是否写入新标记的前提，因此判断只会偏向保留旧写法。

## 为什么接着组字时不改

[iOS 部分上屏直接替换标记区](2026-10-31-ios-cantonese-partial-commit.md) 在系统扩展与独立原生编辑器里复现过：`setMarkedText(上屏)`、`unmarkText()`、`setMarkedText(剩余)` 在同一事件里连续调用，剩余组字会盖掉刚上屏的汉字，那一篇因此改成 `insertText`。本篇不推翻那条证据，只在不再组字的上屏里换写法。代价是 Flutter 宿主里部分选词与就地组字方案的逐音节上屏仍会残留前一段拼写或音节，是已知缺口。

## 既有笔记审计

- [iOS 部分上屏直接替换标记区](2026-10-31-ios-cantonese-partial-commit.md)：部分重叠，继续持有「接着组字时用 `insertText`」的决定；本篇只改不再组字的上屏。
- [快捷短语光标](../feature/2026-10-31-quick-phrase-cursor.md)：部分重叠，本篇复用它的临时标记加 `unmarkText` 写法，光标后缀路径不变。
- [移动端默认数字页](../feature/2026-10-15-mobile-default-number-pad.md)：部分重叠，原样上屏与数字仍是一次替换，只是从 `insertText` 换成显式替换标记区；它的系统编辑器用例需要随本篇重跑。

## Alternatives considered

- **一律 `setMarkedText` 加 `unmarkText`**：Flutter 里部分选词也能正确，改动最少。但会原样恢复 #10284 在真实跨进程代理上复现的丢字，不能用进程内探针否定那次复现。
- **先 `setMarkedText("")` 清掉标记区，再 `insertText`**：两类宿主语义上都对。但 [移动端默认数字页](../feature/2026-10-15-mobile-default-number-pad.md) 已记录跨进程时先撤标记再单独插入会读到尚未更新的选区，实测得到 `1你`，后写的内容跑到前面，与 #10843 描述的「后打的字出现在前面」是同一类症状。
- **按宿主识别 Flutter 再分支**：键盘扩展拿不到宿主的 bundle id 或控件类型，没有可靠依据。
- **等宿主升级 Flutter**：上游修复只在 2026-09 之后的 master，已发布的应用要等各自升级引擎，用户现在就遇到。

## Consequences

- Flutter 宿主里整词上屏、数字页原样上屏不再留下拼音；遵守 UIKit 约定的宿主仍只收到一次正文。
- 普通上屏与快捷短语定位共用同一组代理调用，光标后缀路径的行为不变。
- 部分选词和就地组字方案在 Flutter 宿主里仍有残留，需要另找跨进程可验证的写法。
- `MarkedTextCommitHostTests` 用按旧 Flutter 引擎行为写的替身代理守住整词上屏、数字页原样上屏和部分选词的调用顺序，另用原生 `UITextView` 确认正文只写一次。替身代理不等于真实 Flutter 宿主，也不覆盖跨进程代理。

- 单元测试里几个内存替身文档把 `setMarkedText`、`unmarkText` 写成空操作，或只按 `insertText` 记上屏，等于假设上屏一定走 `insertText`。整词上屏换写法后，`KeyLayoutStyleTests` 的数字页韩文与藏文、`GlideTypingTests` 的双语滑行在 CI 上变红，正文其实已经写进标记区再定稿。替身改为按 UIKit 约定记标记区：`insertText` 替换它，`unmarkText` 把非空标记区算作一次上屏，断言不变。
- 同一次 CI 里 `NineKeyKeyboardTests.testPhrasesOpenInTheKeyAreaAndCloseFromTheirIcon` 也变红，原因是新加的 `MarkedTextCommitHostTests` 原生编辑器用例把 `UITextView` 放进窗口当第一响应者：测试宿主随之拉起自己的系统键盘，按字母排在后面的常用语面板用例在顶栏显示「输入运行时尚未准备完成」，工具栏被藏起。单独跑 `NineKeyKeyboardTests` 整个类通过；只跑 `testTextViewHostReceivesTheCandidateExactlyOnce` 加常用语用例就能复现，其余四个新用例在前时不复现。原生编辑器用例改为不放进窗口、不当第一响应者，标记与定稿仍由 `UITextView` 执行。

## 原生编辑器光标落到文档末尾的排查

评审指出 `QuickPhraseCursorKeyboardTests.testAbsentEmptyAndInvalidSuffixKeepTheCompleteCommitAtItsEnd` 在本 PR 上光标停在 12（文档末尾），预期 11（正文末尾）。CI（Xcode 26.3 模拟器）全套单元测试里这一项通过；本机 iOS 26.5 模拟器上单独运行约四五次出现一次，只出现在新进程的第一轮。

逐次记录代理调用后，失败那一次在上屏的 `setMarkedText(正文, {10, 0})` 之前，编辑器里仍是标记区 `Kt`、光标 3，与预期一致；这一次调用一返回，标记区已经没有了，光标在 12。键盘代码没有参与这一步：测试里绕过键盘、直接对 `UITextView` 先标记 `Kt`，主线程阻塞 1.5 秒后再做同样两次调用，也复现了一次。同样的调用不阻塞时连跑 16 次、光标停在正文中间时连跑十余次都正确。测试宿主进程里挂着系统键盘，日志里有它的 `[CandidateGeneration]` 输出；给测试编辑器关掉 `inlinePredictionType` 后，同一用例在同一模拟器上连跑 16 次全部通过，耗时也从失败时的 4 到 8 秒回到 1.5 秒左右。

据此判断这是测试宿主里系统键盘的行内预测与直接驱动的 `UITextView` 之间的相互作用，不是新写法的调用顺序问题：第三方键盘生效时宿主不跑系统键盘的预测。测试编辑器因此关掉行内预测，断言不放宽。没有在跨进程系统扩展里确认真实宿主不会出现同样的光标跳动，这一点仍要靠 `KeyboardExtensionEditorUITests` 和真机复验。

## Verification

- 修复前，`MarkedTextCommitHostTests` 在 iOS 27.0 模拟器上复现报告现象：整词上屏得到 `甲ni'hao你好丙`，数字页得到 `nini1`；修复后 5 项全部通过。
- 光标落到文档末尾的排查见上一节；关掉测试编辑器的行内预测后，该用例在 iOS 26.5 模拟器上连跑 16 次通过。
- 替身文档改为记标记区、原生编辑器用例不再当第一响应者后，CI 上变红的数字页韩文、藏文、双语滑行与九键常用语面板四项在 iOS 26.5 模拟器上通过。
- 真实 Flutter 宿主、闲鱼与 B 站真机、`KeyboardExtensionEditorUITests` 的跨进程系统编辑器用例均未验证，留给 CI 之外的系统扩展回归与报告者复验。
