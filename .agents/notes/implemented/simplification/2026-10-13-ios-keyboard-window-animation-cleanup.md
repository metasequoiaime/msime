# Agent Note: iOS 键盘按钮共用离窗动画清理

Status: implemented

## Problem

工具栏按钮、品牌标记按钮和单手模式边缘按钮在离开窗口时各自执行相同的图层动画清理与变换复位。以后调整按压反馈清理时，三处容易产生差异。

## Decision

在键盘扩展的 `UIView` extension 中抽出 `resetKeyboardPressAnimationAfterWindowRemoval`。三个控件仍各自覆盖 `didMoveToWindow`，只把相同的两步清理交给公共方法。

## Alternatives considered

- 建立共同 `UIButton` 基类：三个控件现有继承结构不同，单为两步清理引入基类会扩大耦合。
- 保留三份实现：代码量虽小，后续修改仍需同步三处。

## Consequences

三处离窗清理共用一个实现，按压动画和变换在原来的生命周期节点清除。公共方法只处理动画状态，不接管各控件的其他布局或反馈逻辑。通过 diff 检查、notes 验证和调用点静态检查；本地 quick 门禁另行运行。
