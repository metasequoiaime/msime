# Agent Note: iOS 键盘通过 inputView 承载并避免布局视图泄漏

Status: implemented

## Problem

[#11012](https://github.com/metasequoiaime/msime/issues/11012) 报告最新版本在所有 App 唤起水杉输入法时会切回系统键盘。问题出现在 [#10958](https://github.com/metasequoiaime/msime/pull/10958) 把控制器根视图从 `UIInputView` 改成普通 `UIView` 之后：`UIInputViewController` 的公开契约要求通过 `inputView` 提供主输入视图，直接写 `view` 绕过了系统键盘宿主取视图的路径。

但 #10958 同时解决了 [#10944](https://github.com/metasequoiaime/msime/issues/10944) 的内存问题：iOS 会长期保留 `UIInputView` 空壳及其布局引擎，整套键盘约束如果仍挂在这个根视图上，反复弹出键盘会持续增长。

## Decision

`KeyboardViewController` 通过 `inputView` 提供一个只负责满足 UIKit 宿主契约的 `KeyboardInputContainerView`。整套键盘约束仍挂在普通的 `KeyboardInputView` 上，容器只用自适应 frame 承载它，不在容器上建立布局约束。控制器释放时从容器摘掉普通视图，让系统可能保留的 `UIInputView` 只剩空壳；按键音协议同时由容器实现，确保 UIKit 检查实际可见输入视图时仍能读取偏好。

UIKit 的默认 `inputView` setter 会把控制器的 `view` 也替换成这个输入视图，单纯调整赋值顺序仍会把布局根视图放回 `UIInputView`。因此控制器覆写 `inputView` 的存取，把容器保存在自己的属性里并仍向系统暴露同一对象；`loadView` 继续把普通视图设为控制器根视图，释放时由控制器持有的容器摘掉内容。

这篇笔记补充并收窄 [iOS 键盘普通根视图](2026-11-05-ios-keyboard-plain-root-view.md) 的决定：保留普通视图承载布局，但恢复 `inputView` 的系统承载边界。

这同时保留两条边界：系统看到的主视图类型仍是 `UIInputView`，布局引擎的宿主仍是可释放的普通视图。回归测试固定 `inputView`、控制器根视图和容器子视图的关系，并继续保留原有的重复弹出释放测试。

#11389 的原始截图显示唤起期间九键被纵向拉长。外层容器不带内容约束，也不能依靠内部 `keyboardHeight` 约束参与系统尺寸协商；容器启用 `allowsSelfSizing`，控制器在原有高度更新边界同步目标高度，容器通过 `systemLayoutSizeFitting` 和固有高度返回它。系统仍决定宽度，初次显示、横竖屏、编辑条和键盘调整使用同一个高度来源，不给外层容器挂整套键盘约束。Apple 的 [UIInputView 尺寸协商契约](https://developer.apple.com/documentation/uikit/uiinputview/allowsselfsizing)说明，只有开启自定尺寸时系统才采用该拟合结果；截图与这一缺口相符，但不能把单张截图当作报告者环境复验。

## Alternatives considered

- **继续把普通 `UIView` 直接写入 `view`**：能避免布局引擎泄漏，但绕开 `UIInputViewController.inputView` 的公开宿主契约，无法解释 #11012 的系统级回退，因此不采用。
- **把整套键盘恢复为 `UIInputView` 根视图**：符合 UIKit 的默认用法，但会重现 #10944 的布局引擎累积，iPhone XR 上重复弹出后会被系统结束，因此不采用。
- **只调整 `inputView` 与 `view` 的赋值顺序**：UIKit 的 setter 仍会替换控制器根视图，第二轮 Simulator 的内存回归证明它不能保留 #10944 的边界，因此不采用。
- **使用私有 UIKit 字段拆解 `UIInputView` 的引用**：依赖未公开实现，审核和系统升级风险不可接受，因此不采用。
- **把原有高度约束移到外层输入视图**：可让系统直接读到目标高度，但会重新让外层输入视图承担布局约束，违背 #10944 的内存边界；只同步一个高度数值并参与公开的尺寸协商。

## Consequences

- 系统键盘恢复从 `inputView` 取得水杉键盘的路径，覆盖所有宿主 App 的共同入口。
- #10944 的内存边界仍由普通布局根视图和释放时摘除保证。
- 真实 iPhone、系统键盘入口和多个 App 的验证范围在对应 PR 如实记录，不把缺少设备复验当作代码实现未完成。

## Verification

- #11389 新增 `KeyboardInputSizingTests`：在控制器尚未显示时查询实际 `inputView`，用零、系统键盘和整屏高度的提议验证首轮拟合；独立容器用例覆盖连续高度改变、带优先级拟合、内部视图按 frame 跟随、宿主必需高度以及拒绝无效更新。原有内存释放测试保持不变。
- 使用 Mac Catalyst SDK 编译并执行真实 `KeyboardInputView.swift` 的独立 UIKit 探针，只替代无关的按键音偏好；四轮目标高度、三种提议高度、两种拟合入口、内容 frame 跟随、必需高度和无效更新均通过。基线真实容器在首次拟合返回零尺寸，负对照退出码为 1。该探针不包含完整控制器，不等于整个 `MSIMEKeyboardTests` 已运行。
- 尚未在报告者的微信与实体 iPhone 中复验闪动，不把源码分析或尺寸用例等同于系统转场已确认恢复。
