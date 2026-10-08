# Agent Note: iOS 功能菜单的「键盘高度」与 Android 一样打开内联高度条

Status: implemented

## Problem

#5994 把 iOS 功能菜单第一页的「键盘高度」换成了「键盘布局」，并删掉了内联高度条 `InlineHeightBar`。理由是布局面板（`KeyboardLayoutPickerView`）也能拖动调同一个高度（`touch_keyboard_height_adjustment`），两处入口重复。结果这一格和 Android 不一样了：Android 点「键盘高度」后，工具栏那一行换成「取消 | 拖动柄 | 重置 | 完成」，按键照常可用，拖动时实时预览，点「完成」才保存。iOS 却要打开一整块布局面板，高度、键距和行距挤在同一套拖动手势里。用户要求这一格和 Android 一致。

## Decision

- 功能菜单第一页这一格恢复为「键盘高度」（`KeyboardTool` id `keyboardHeight`）。点了以后，`showInlineHeight()` 把 `InlineHeightBar` 放到工具栏的位置。拖动时只预览，点「完成」写入 `KeyboardLayoutPreference.heightAdjustment` 并 `commitTouchKeyboardGeometry()`；点「取消」或收起键盘时，恢复打开调节条时的高度。行为和 Android 的 `MSIMEInputService` 内联高度条相同。
- 「键盘布局」放回菜单末尾（在「表情」之前），键距和行距在键盘里仍然能调。布局面板打开前先按取消收起调节条，免得两处同时改高度。
- 恢复的代码就是 #5994 之前的实现（`InlineHeightBar.swift` 和控制器里的接线），以及 `MorePanelFoldTests` 里的两条测试。#5994 里同时做的候选行高度改动保持不变。

## Alternatives considered

- **维持 #5994 的做法，只留布局面板** — 入口最少，高度只有一处可改。但这一格的行为和 Android 不同，正是用户要改的；而且布局面板对只想调高度的人来说太重：整块盖在键盘上，还要先弄明白横拖、竖拖分别改什么。
- **恢复内联条，同时把「键盘布局」从菜单里彻底拿掉，完全照搬 Android 的菜单** — 和 Android 逐格一致。但 iOS 设置 App 的「键盘」页之外，键盘里就再没有调键距、行距的地方了，等于删掉一个现有能力；Android 能这样做，是因为它的键距、行距调节在别的入口。

## Consequences

- **收益**：「键盘高度」的位置、样子和保存时机与 Android 一致，调高度时按键照常可用，能边打字边看效果。
- **代价**：高度又有两个入口（内联条和布局面板）。两者写同一个设置，打开布局面板时先取消调节条，所以不会出现「取消」恢复成过时高度的问题。
- **仍与 Android 不同**：iOS 的高度范围仍是共享偏好的 −12…48pt（换算成百分比，见 `KeyboardHeightPercent`），说明文字也只显示百分比。Android 在 [键盘高度放宽到 160%](2026-10-08-android-keyboard-height-range.md) 之后是 75%–160%，还显示实际像素高度。这两项没有跟进。

## Verification

`MorePanelFoldTests` 的 `testEveryToolIsReachableByPagingInTheDesignsOrder` 钉住菜单顺序，`testKeyboardHeightTileAdjustsInline` 和 `testKeyboardHeightPreviewSurvivesKeyPanelsAndIsCancelledWhenTheKeyboardGoesAway` 钉住内联条的预览、取消、完成和收起键盘时的行为。
