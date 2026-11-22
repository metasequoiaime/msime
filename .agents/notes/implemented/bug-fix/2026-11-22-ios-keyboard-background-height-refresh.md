# Agent Note: iOS 键盘回前台重新协商外层高度

Status: implemented

## Problem

[#11780](https://github.com/metasequoiaime/msime/issues/11780) 报告宿主应用退到后台再回前台后，键盘顶部出现多余空白。键盘控制器通过 `KeyboardInputContainerView` 向 UIKit 提供自适应高度。目标高度没有变化时，`updatePreferredHeight` 原本直接返回，因此回前台不会再次触发 `UIInputView` 的固有尺寸失效。UIKit 可能继续使用退后台前保存的外层输入视图高度，普通键盘视图内部仍按当前约束布局，二者之间形成顶部空白。

## Decision

`KeyboardInputContainerView` 增加 `refreshPreferredHeight`。它只在已有目标高度时调用 `invalidateIntrinsicContentSize()`，不改目标高度，也不参与普通布局循环。`KeyboardViewController.viewWillAppear` 在更新键盘布局后调用它，让宿主每次重新出现时重新读取相同的目标高度。

回前台时高度本身改变仍沿用 `updatePreferredHeight` 的原有路径。显式刷新只处理值相同但宿主外框可能已过期的生命周期边界。

## Alternatives considered

- **移除 `preferredHeight != height` 判断**：每次布局回调都会让 UIKit 重新协商，容易在 `viewDidLayoutSubviews` 中产生多余布局和反馈循环，因此不采用。
- **在回前台时重建整个键盘控制器或输入视图**：可以丢弃旧外框，但会破坏会话和键盘状态，也绕开了已有的内存与会话生命周期约束，因此不采用。

## Consequences

- 键盘回前台时会重新协商一次 `UIInputView` 高度，目标高度和键盘内部约束保持同一份来源。
- 正常布局期间仍只在高度变化时失效固有尺寸，避免额外协商。
- 回归测试通过系统键盘宿主从后台回到前台检查工具栏和按键的相对位置。

## Verification

- `KeyboardExtensionEditorUITests.testKeyboardTopRowKeepsItsPositionWhenHostReturnsFromBackground` 覆盖退后台、回前台和键盘顶部几何关系。
- `KeyboardInputSizingTests` 保留容器目标高度、必需高度和无效高度更新回归。
- iOS Simulator 的完整 Xcode 工程测试待当前远端原生构建完成后运行，实体 iPhone 尚未复验。
