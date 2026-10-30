# Agent Note: Android 符号集插件读取的会话生命周期

Status: implemented

## Problem

Android 符号面板打开时会在 `emojiWorker` 上读取当前偏好目录下的符号集插件。输入框切换或面板关闭会销毁当前会话，但旧读取结果仍会回到主线程，原实现没有检查面板代次、可见性或偏好目录，可能把旧输入框的插件分类写进新面板，或在关闭后修改隐藏面板的分类状态。

## Decision

`ImePanels` 为每次插件读取记录独立代次；关闭符号面板时推进代次，新的读取也推进代次。回调只有在代次仍相同、符号面板可见且读取时的 `preferencesDirectory` 与当前目录相同，才调用 `SymbolPanelView.setPluginCategories`。目录为空的结果一律不接受。判断集中在无 Android 依赖的 `PluginSymbolLifecyclePolicy`，供 JVM 冒烟测试覆盖边界。

## Alternatives considered

- 只在回调中检查符号面板是否可见：可以避免关闭面板后的写回，但重新打开同一面板或切换输入框后仍可能接受旧目录结果，不能隔离会话。
- 在切换输入框时直接清空插件分类：能隐藏旧内容，但无法阻止旧回调在新面板打开后覆盖当前目录，而且会让面板多一次无意义的闪烁。

## Consequences

插件目录读取仍复用现有单线程 `emojiWorker`，没有取消 JNI 读取的接口；过期任务会自然完成，但结果只在生命周期条件满足时写回。符号面板关闭、输入框切换和重新打开都不会再把旧插件分类带入当前会话。新增 JVM 冒烟覆盖同代、关闭、换代、换目录、空目录和代次溢出。
