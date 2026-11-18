# Agent Note: Harmony 候选窗跟随外观快照刷新

Status: implemented

## Problem

Harmony 候选背景图、画布场景、圆角和布局直接从 `KeyboardView.settings` 读取。设置重载会调用 `applySettings()` 并替换快照，`KeyboardSession.applyAppearance` 也建立新的对象，但视图字段是普通字段：仅替换该字段不能可靠通知依赖它的 ArkUI 节点重算。这与旧图片同名替换时是否重新读取文件是不同的问题。

## Decision

把视图内的快照声明为 `@State`。会话回调和赋值路径保持原样，避免新增偏好传输、进程通信或包名依赖。对象快照由会话整体替换，遵循 [ArkUI @State 对象观察规则](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-state/)；不依赖深层属性原地修改。候选画布原有的 `@Prop`、`@Watch` 负责在收到新场景后释放旧图、抑制晚到的旧解码。

该字段也供触屏键盘读取，修改可能使依赖它的触屏节点重绘；没有改变触屏设置取值或布局规则，但仍需要触屏构建和性能回归。更改未覆盖皮肤来源不可达、跨进程偏好未同步或同名图片未被重新读取等问题。

## Validation

`candidate-live-view-refresh.test.cjs` 检查快照赋值和接线，并在 Node 中执行真实的画布 reload、dropLease、repaint：旧图迟到不会覆盖新图、清除场景会释放和清空画布。该替身不运行 ArkUI，无法验证状态追踪、图层像素和系统组件更新。未做 HAP 构建或设备测试；既有本地综合版本确认过自定义与导入皮肤，但不能将结果归因于此处单项改动。需在官方包设备上验证更换、删除、重选皮肤及触屏键盘。

## Alternatives considered

- 只在 `applySettings()` 中主动调用局部刷新：改动范围较小，但无法覆盖所有直接读取快照的 ArkUI 节点，后续新增依赖也容易遗漏。
- 为每个候选外观字段增加独立的 `@State`：能够缩小重绘范围，但会重复拆分现有快照契约，且设置重载仍要维护多条赋值路径。本次沿用整体快照替换。

## Consequences

- 整体替换 `settings` 会让依赖该快照的候选背景、画布和触屏布局节点重新计算，解决设置重载后候选外观不刷新的问题。
- 触屏节点可能随快照一起重绘，必须在 Harmony 官方包和设备上补做外观切换与性能验证。ArkUI 状态追踪、实际像素和资源来源仍不在 Node 替身覆盖范围内。
