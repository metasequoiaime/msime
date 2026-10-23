# Agent Note: iOS 分页面板改回系统分页

Status: implemented

## Problem

iOS 键盘的输入方式面板和功能面板左右滑动大多翻不过去。[面板横划统一接管](2026-10-18-ios-panel-horizontal-swipe.md)关掉了 `UIScrollView` 自带的 pan，改由 `KeyboardPagedGridView` 自己的横划手势写 `contentOffset`，抬手时只按偏移取最近的整页，不看速度，所以手指要拖过半个面板宽才翻页。

模拟器（iOS 27，iPhone 17 Pro，面板宽 402pt）在功能面板上从图块开始横扫：向左 80、150pt，向右 190、215pt 都停在原页，向左 250pt、向右 260pt 才翻页。

那篇笔记认为系统分页滑不动，是因为图块（UIButton、UIControl、带长按手势的图块）先吃掉了触摸。把分页换回系统 `UIScrollView`、加日志实测，这个前提不成立：触摸落在 `KeyboardFunctionTileView` 上时滚动视图的 pan 照常开始（`gestureRecognizerShouldBegin` 返回真），随后 `touchesShouldCancel(in:)` 取消图块的触摸；向左快扫 70pt、抬手速度约 -912pt/s 时，`scrollViewWillEndDragging` 给出的目标偏移是下一页。左右 80、120pt 的快扫都翻页，速度为零的 50pt 慢挪留在原页，首尾页回弹。

## Decision

`KeyboardPagedGridView` 的翻页交还滚动视图自己的分页：`isPagingEnabled`，`delaysContentTouches = false`，`canCancelContentTouches = true`，`KeyboardPagingScrollView.touchesShouldCancel(in:)` 对图块一律返回真。不再有自定义的横划手势，抬手后停在哪一页由 UIKit 按速度和距离决定。

[反向滑动后的整页对齐](2026-10-18-ios-paged-grid-reverse-swipe.md)保留：拖动或减速结束时仍把偏移对齐到整页。表情面板的分类横划是另一个手势，不在这次改动里。

## Alternatives considered

- 保留自定义手势，在抬手时自己加速度和距离门槛：能修好这一次，但门槛只能自己定，系统分页的手感（速度换算、回弹、减速）得一项项仿写，而系统分页在实测里本来就能从图块上翻页，自定义手势要解决的问题并不存在。

## Consequences

翻页手感与系统分页滚动视图一致，快扫一小段就翻页。输入方式、功能、皮肤等复用这个网格的面板都适用。

## Verification

- 模拟器：修复前后用相同的横扫对照，见 Problem 一节的数据。输入方式面板在测试模拟器上只有一页（7 个格子），模拟器上实测的是同一个网格的功能面板；修复版上 80pt 的短扫共 10 次，9 次翻页，没翻的那次是工具合成的手势，没有日志看它的抬手速度。
- 真机：开发签名的 Release 包装到 iPhone XR 和 iPhone 17，报告人在真机上横扫面板，翻页顺畅。
- 单测：`MorePanelFoldTests`（含 `testPagedGridUsesSystemPagingFromTiles`）、`KeyboardEmojiCatalogTests`、`NineKeyKeyboardTests` 共 152 项在 iOS 27 模拟器上通过。
- 「完全滑不动」的原始报告没有复现，它的来源没有证据。
