# Agent Note: 安卓日语九键左列常驻 符，表情并入它的长按

Status: implemented

## Problem

Issue #10439 报告的缺陷：安卓日语九键没有符号面板入口。左列原来是 ◀、→、123、☺（只在顶部工具栏没有表情按钮时，`MSIMEInputService.japaneseSideEmojiKey`）、切换（Android 9 以下）。工具栏开着表情时左列没有 ☺，也没有符号键；关掉表情时多出来的是 ☺，同样没有符号键。日语 26 键底行一直有 符，两种日语键盘的入口不一致，九键用户只能从 123 层的少量括号和常用符号里挑。

## Decision

`ImeLayoutRows.rebuildJapaneseNineKeyRows` 在原来 ☺ 的位置常驻一个 符（`japaneseSymbolPanelKey`）：点按打开符号面板，记作 `SoftSymbol`，与 26 键底行的 符 同一个键位 id。左列因此固定是 ◀、→、123、符、切换（中英键与数字键对调时是 ◀、→、符、切换、语言键，见[日语九键交换后语言键在左下角](../feature/2026-11-02-android-japanese-nine-key-swapped-language-key-bottom-left.md)），最多五格，与原来工具栏关掉表情时的格数相同。

`japaneseSideEmojiKey()` 为真（工具栏关掉表情或整条隐藏）时，符 接上长按打开表情浏览，读屏描述写成「打开符号面板；长按打开表情浏览」；为假时不接长按，描述只写符号面板，避免和工具栏的表情按钮两处重复。这个开关一变照旧重建键行（`japaneseEmojiKeyChanged`），长按和描述跟着更新。

`check-host.sh` 锁住左列放的是 `japaneseSymbolPanelKey(s.japaneseSideEmojiKey())`、`rebuildJapaneseNineKeyRows` 不再出现 `SoftEmoji`，以及 符 的点按与长按动作。

## Historical audit

- [移动端数字层表情入口](../feature/2026-10-17-mobile-number-layer-emoji-entry.md)：无冲突，它只提到日语侧列决定不变，本篇接管日语九键左列的表情入口。
- [安卓日语五向提示](../feature/2026-10-10-android-japanese-flick-guide.md)、[日语连点切换](../feature/2026-10-23-android-japanese-toggle-input.md)：无关，只涉及假名格。
- 按键布局样式（[2026-10-27-touch-key-layout-styles](../feature/2026-10-27-touch-key-layout-styles.md)）和按键热力图都不画日语九键，`KeyHeatmapLayout`、`KeyLayoutStylesPolicy` 及其冒烟不用改。

## Alternatives considered

- 左列加一格 符，☺ 照旧在工具栏关掉表情时出现：最直观，表情也不用藏在长按里。但这时左列变成六格，每格高度从五分之一降到六分之一，在矮键盘上 123、符 这些字会被压扁；issue 要的正是「把表情键换成符号键」，日语 26 键的表情也只在工具栏。
- 照 iOS 用 ^_^ 一个键同时进颜文字和表情：iOS 复刻的是 Apple 日语键盘，表情面板里能找到颜文字，但安卓的符号面板和表情面板是两个，^_^ 解决不了符号入口的问题。

## Consequences

日语九键在任何工具栏设置下都能一键进符号面板，和日语 26 键一致；左列格数不变，不影响假名格的宽度和高度。代价是工具栏关掉表情的用户要改成长按 符 进表情，原来一点即达，现在多了一个要发现的手势；读屏描述写明了这一点。工具栏的符号快捷键开着时，符号入口在工具栏和左列各有一个，这与 26 键底行的 符 和工具栏同时存在的情况相同，不另做去重。

未在真机验收：工具栏表情开、关两种情况下左列的排法、符 的点按与长按、读屏描述，以及 Android 9 以下带 切换 时五格的高度。
