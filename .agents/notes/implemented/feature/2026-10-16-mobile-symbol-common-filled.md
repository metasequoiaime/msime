# Agent Note: 符号面板「常用」用默认标点补满

Status: implemented

Android 的补满规则由 [Android 常用符号只显示实际使用记录](2026-10-20-android-common-symbols-history-only.md) 取代，恢复仅显示使用记录并保持默认打开「常用」。本篇的 iOS 决定及避免空白首屏的原始取舍保留。

## Problem

产品方反馈：点「符」打开的符号面板，第一页最近基本是空的，很难看。iOS 只要用过符号就把「最近」放在第一页，里面只有点过的几个；Android 的「常用」按 [#5671 的决定](2026-10-08-android-symbol-panel-recents.md) 只放点过的符号，没有记录时才改开「中文」，点过一两个之后打开的就是只有一两格的「常用」。鸿蒙的「常用」是写死的 30 个标点，没有这个问题。

## Decision

iOS 与 Android 的第一页都是「常用」：最近点过的符号在前，后面用默认的 30 个常用标点补到 30 个（六行），去掉重复。默认内容与鸿蒙「常用」同一份、同一顺序（成对符号不跨行，见[符号面板的成对符号不分在两行](../bug-fix/2026-10-31-symbol-panel-pairs-on-one-row.md)）：

，。？！、；：…—·“”‘’～（）《》￥【】＆＃＠％＋－＝／

- iOS：不再单独列「最近」，`KeyboardSymbolPanelView.commonSymbols(recents:)` 把 `KeyboardSymbolRecents` 与 `categories[0]` 合并成第一页「常用」。
- Android：`SymbolPanelModel.commonSymbols` 合并 `RECENTS_LIMIT` 内的记录与 `DEFAULT_COMMON`；`initialCategory` 总是返回「常用」。「中文」最前面的中文标点保留。
- 记录方式不变：只记本机、隐私模式与不许学习的输入框不记、刚上屏不重排正在显示的网格。

这推翻了 #5671 笔记里「只放点过的符号、没有记录时打开『中文』」的做法，该篇的使用记录规则仍成立，两篇互链。

## Alternatives considered

- **维持只放点过的符号，没有记录时打开「中文」。** 符合 #5671「常用里只有自己点过的」的原始诉求；但点过一两个符号后第一屏几乎是空的，正是这次反馈的问题。
- **记录少于一行时仍打开「中文」。** 能避开空白首屏；但打开的位置随记录多少变化，用户找不到刚用过的符号在哪一页。
- **单独保留「最近」一页，「常用」写死。** 两种需求各占一页；但仍会出现只有几个符号的一页，而且多一个分类。

## Consequences

- 收益：第一页永远是满的，点过的符号仍排在最前；三端「常用」的默认内容一致。
- 代价：没点过的人看到的「常用」是默认标点，不再只反映自己的习惯；Android 原来没有记录时显示的「点过的符号会出现在这里」提示不会再出现。

## Verification

- iOS：Mac Studio 上 `MSIMEClientTests` 的 `build-for-testing` 通过；本机 iPhone 17 / iOS 27.0 模拟器上 `KeyboardEmojiCatalogTests`、`KeyboardSkinTests`、`NineKeyKeyboardTests.testSymbolKeyOpensAPanelInsteadOfAMenu` 共 55 个用例通过，`testSymbolRecentsStayApartFromEmojiAndLeadThePanel` 断言记录在前、默认补满、不重复。
- Android：`check-host.sh` 通过，`SymbolPanelModelSmoke` 断言没有记录时「常用」就是这 30 个、有记录时记录在前并补满不重复、总是打开「常用」。未在设备上查看。
