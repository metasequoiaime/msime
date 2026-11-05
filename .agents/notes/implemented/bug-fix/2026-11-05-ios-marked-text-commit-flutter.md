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

## Verification

- 修复前，`MarkedTextCommitHostTests` 在 iOS 27.0 模拟器上复现报告现象：整词上屏得到 `甲ni'hao你好丙`，数字页得到 `nini1`；修复后 5 项全部通过。
- 真实 Flutter 宿主、闲鱼与 B 站真机均未验证，留给报告者复验。
