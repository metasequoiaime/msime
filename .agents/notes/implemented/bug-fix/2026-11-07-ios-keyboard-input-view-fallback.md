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

## Alternatives considered

- **继续把普通 `UIView` 直接写入 `view`**：能避免布局引擎泄漏，但绕开 `UIInputViewController.inputView` 的公开宿主契约，无法解释 #11012 的系统级回退，因此不采用。
- **把整套键盘恢复为 `UIInputView` 根视图**：符合 UIKit 的默认用法，但会重现 #10944 的布局引擎累积，iPhone XR 上重复弹出后会被系统结束，因此不采用。
- **只调整 `inputView` 与 `view` 的赋值顺序**：UIKit 的 setter 仍会替换控制器根视图，第二轮 Simulator 的内存回归证明它不能保留 #10944 的边界，因此不采用。
- **使用私有 UIKit 字段拆解 `UIInputView` 的引用**：依赖未公开实现，审核和系统升级风险不可接受，因此不采用。

## Consequences

- 系统键盘恢复从 `inputView` 取得水杉键盘的路径，覆盖所有宿主 App 的共同入口。
- #10944 的内存边界仍由普通布局根视图和释放时摘除保证。
- 目前只完成代码级回归断言和差异检查；尚未在真实 iPhone、系统键盘入口、多个 App 上验收，所以不能关闭 #11012 或声称问题已完全解决，必须在 PR 上补系统键盘验证。
