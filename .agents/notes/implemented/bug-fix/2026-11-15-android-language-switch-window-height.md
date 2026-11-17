# Agent Note: Android 中英文切换合并布局重画

Status: implemented

## Problem

Android 双拼键盘切换中英文时，`toggleInputLanguage` 先用 `command(2/3)` 结束当前组字。`apply` 会在这一步把引擎视图写回宿主并立即重画，但宿主的 `dedicatedEnglish` 仍是旧状态；随后设置新语言、更新大小写并再次重画。候选栏和工具栏因此可能经历一帧混合状态，输入法窗口在社交应用里表现为高度短暂变化。

现有的工具栏行固定策略见[按键布局样式](../feature/2026-10-27-touch-key-layout-styles.md)，隐藏提示不撑高候选区的规则见[移除移动端拼写铅笔](2026-11-10-mobile-remove-spelling-pens.md)。本次处理的是语言切换内部的多次重画时序，不改变这两项布局决定。

## Decision

`toggleInputLanguage` 把结束组字、设置引擎语言、更新宿主语言状态和自动大写合并到现有 `renderBatch` 中。期间由 `apply` 或自动大写触发的 `render` 只记录待绘制状态，最终状态完整写入后才执行一次重画；`shouldRender=false` 的调用继续保持不重画。这样布局测量只会看到一致的 `dedicatedEnglish`、`view` 和键面状态。

设备冒烟在真实双拼会话中测量切换前、切换到英文和切回中文的候选/工具栏区域高度，三次必须一致。

## Alternatives considered

- **只给候选栏或工具栏补固定高度**：能遮住某个表现，但仍允许中间状态触发窗口重排，且会把不需要读音行的方案强行撑高，因此不采用。
- **删除 `command(2/3)` 的重画**：结束组字还会被其他调用路径使用，修改 `apply` 的全局重画语义会影响正常输入和候选更新；本次只在语言切换事务内合并重画。
- **等待系统窗口动画结束再修正高度**：依赖系统时序，无法阻止应用先收到错误的窗口边界，也会增加切换延迟，因此不采用。

## Consequences

中英文切换期间只产生一次最终布局，输入法窗口不会因旧语言视图和新语言状态交错而短暂变高或变矮；已有的英文大小写、组字提交和 `shouldRender=false` 调用语义保持不变。代价是语言切换的最终重画延后到整个同步事务完成，切换路径中的中间视图不再对用户可见，这正是稳定窗口边界所需的时序。

验证：远端 `rbuild env ANDROID_HOME=/Users/xiaomo/Library/Android/sdk bash platforms/android/check-host.sh` 通过，运行 354 个 Android JVM 冒烟并编译 289 个设备套件源码；设备真机/报告者环境尚未验收，留待 PR 的设备检查和报告者复测。
