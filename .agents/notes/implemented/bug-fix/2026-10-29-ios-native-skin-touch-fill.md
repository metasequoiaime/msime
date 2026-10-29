# Agent Note: iOS 原生皮肤铺一层几乎透明的底色，让空白处的触摸送达键盘扩展

Status: implemented

## Problem

#10173：iOS 键盘选「原生」皮肤后，功能面板（更多工具）左右翻页、点格子，以及候选栏左右滑动都严重卡顿甚至没反应，换成其他皮肤就正常。

[原生皮肤](../feature/2026-10-09-ios-native-keyboard-skin.md)把背景交给系统键盘底板（iOS 26 起是 Liquid Glass）：`drawsNativeBackground` 为真时，键盘根视图（`KeyboardInputView`）和底下的 `skinBackdrop` 都是 `.clear`；功能面板、符号面板和候选栏也是透明的，功能格子只用 `CAShapeLayer` 画图标、自身无底色。于是除了按键、文字和图标本身，键盘里大片区域在扩展里什么都没画。经典兜底（应用主题解析不了时的 `system`）走同一条路。

键盘扩展的触摸先由系统跨进程命中，再交给扩展里的 UIKit 按视图树找目标。[工具栏系统命中](2026-10-13-ios-toolbar-native-hit-region.md)在 iPhone 17 真机上观察到：只挂自绘图层、没有原生内容的按钮，本地 `hitTest` 正常、模拟器也能点，真机却收不到触摸。由此推断：系统只把落在扩展实际画出内容上的触摸交给扩展。原生皮肤下从格子之间、候选之间的空白处起手的点按和滑动送不到扩展，用户看到的就是时灵时不灵的翻页和滑动。其他皮肤的根视图是不透明的皮肤底色，每一处都有内容，所以不受影响。

## Decision

`KeyboardTheme` 新增 `nativeBackgroundFill`：键盘自己的底色（原生皮肤是 `SystemKeyboardTokens.background`，经典兜底是 `NativeKeyboardTokens.background`）乘上 `nativeBackgroundFillAlpha`（0.01）。`recolorKeys` 在 `drawsNativeBackground` 时把根视图和 `skinBackdrop` 的底色都设成它，不再是 `.clear`。

- **只铺在最底下**：功能面板、符号面板、候选栏和 `installPanel` 打开的所有键区面板都是根视图 `view` 的子视图（扫过 `KeyboardExtension/Sources` 里所有 `addSubview`，没有挂到别的窗口或根视图之外的面板），根视图铺一层就覆盖了它们背后的每一处；系统把触摸交给扩展之后，UIKit 照常按视图树找到格子、候选或滚动视图。面板和候选栏自己的底色（`KeyboardMorePickerView`、`KeyboardSymbolPanelView.surface`、`stripBackground`）保持 `.clear`，不再各铺一层。
- **根视图和 `skinBackdrop` 两层都铺**：根视图是 `UIInputView`，万一它的底色在系统那边不算画出的内容，铺满全键盘的 `skinBackdrop` 仍有一层；反过来 `skinBackdrop` 关了 `isUserInteractionEnabled`，根视图那层也兜得住。两层叠起来约 2%。
- **透明度取 1%**：合成按 8 位通道量化，1% 约 2.55/255，量化后不为 0；0.1% 量化后可能变成 0，又成了「没画」。颜色取键盘自己的底色而不是黑或白：叠在本来就接近这个颜色的系统底板上，色差只有底板与 token 之差的 2%，c69ac27344 要的「iOS 26 那条系统底板看上去是键盘的一部分」不受影响。
- 其他皮肤的根视图仍是不透明的皮肤底色，App 里的皮肤预览（`KeyboardSkinBackgroundView` 本身）不变。

## Alternatives considered

- **每个透明面板和候选栏各自铺 `nativeBackgroundFill`（triage 给的方案）**：每块面板自己保证有内容，不依赖它们挂在哪个父视图下。但它们全都叠在根视图上，根视图一层已经覆盖；各铺一层只会在面板区域把透明度叠到 3%–4%，让打开面板时的底色与键盘其余部分不一致，还要在每个新面板上记得加。所以只铺在最底下。
- **只在功能格子上补原生内容（像工具栏那样 `setImage` 整格图像）**：贴近 #7682 已在真机验证的做法。但它只修格子，格子之间的间隙、候选之间的空白、符号面板的空白照样收不到，滚动视图起手的翻页和滑动问题还在。
- **原生皮肤改回铺不透明的系统底色**：最稳，命中一定没问题。但它推翻原生皮肤透出 Liquid Glass 的设计，iOS 26 在键盘上方画的那条系统底板又会变回一条突兀的灰带（c69ac27344 修的就是它）。
- **判定是合成开销（透明滚动内容叠在 Liquid Glass 上）而不是命中问题，滚动时给面板加近乎不透明的底**：triage 同样提出过这种可能。但「点格子空白边缘没反应」解释不了合成开销，而命中问题能同时解释点按和滑动；这条要在真机上用 Instruments 对照后才能排除，见下文。

## Consequences

- **收益**：原生皮肤和经典兜底下，键盘每一处都有画出的内容，从空白处起手的点按、翻页和候选滑动不再被系统丢掉；视觉上仍透出系统底板。
- **代价**：原生皮肤的底不再是严格的 `.clear`，而是 1% 的键盘底色（根视图和 `skinBackdrop` 两层约 2%）。日后新增的「把背景交给系统底板」的界面，如果不是 `view` 的子视图，要自己铺这层。
- **测试**：`KeyboardSkinTests` 的真实键盘用例改为断言根视图和 `skinBackdrop` 的底色透明度大于 0、不超过 `nativeBackgroundFillAlpha`、颜色是键盘底色；新用例 `testNativeKeyboardDrawsContentUnderEveryPointForTouchDelivery` 在透明画布上渲染整块原生键盘，平时、打开功能面板和打开符号面板三种状态下都要求没有一个透明像素，同时确认面板自身仍是 `.clear`。按修改前的代码（根视图、`skinBackdrop` 和面板全是 `.clear`），格子和候选的间隙会是透明像素；这一点是推算，没有对着修改前的代码实际跑过这条用例。本次在 Studio 上只完成了测试包的编译（`build-for-testing`），Studio 的 CoreSimulator 版本落后于 Xcode、经 SSH 起不了模拟器，用例本身交给 PR 上的 iOS CI 运行。
- **未验证**：「系统只把落在画出内容上的触摸交给扩展」是由 #7682 的真机观察推断的，1% 的底色在真机上能否被系统算作内容、是否真的修好 #10173，都还没在 iPhone 上复现和验收；Instruments 的 Core Animation 帧率对照（排除合成开销）也没做。如果真机证明 1% 不够，调 `nativeBackgroundFillAlpha` 即可；如果证明是合成开销，就改走上面最后一个备选。
