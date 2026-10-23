# Agent Note: Android 按键动画面板读取当前选项

Status: implemented

## Problem

#9336 在键盘设置的反馈页切换按键动画后，行尾摘要已变化，重新打开面板时的对号仍停在进入页面时的旧值。`render` 的点击回调捕获首次读取的 `animation`，保存成功不重建页面。

## Decision

每次打开按键动画面板时，经现有 `HostTask` 在后台读取当前 `AndroidLocalSettings`，以实际保存的值生成选中标记；保留原来成功更新行摘要、失败刷新页面的行为。

## Related notes audit

检索 `按键动画`、`KEY_ANIMATION` 与 `pickAnimation`，只有设置搜索索引涉及该选项的可发现性，与本次选中状态无重叠。

## Alternatives considered

保存后重建整个页面：可以复用现有 `reload`，但改变局部选择的刷新范围，可能中断页面其他控件的交互。选择在弹窗打开时读取当前值，避免另建一份可漂移的内存缓存。

## Verification

- 旧 APK 在 API 35 同一反馈页切换动画、重开面板时确定失败：对号没有跟随刚保存的值。
- `check-host.sh` 通过 256 个 JVM smoke、216 个设备源码编译及 JNI/Java 契约；完整当前原生库与发布宿主 APK 构建通过，使用开发签名、校验 16 KB 对齐。
- `OptionSheetDeviceSmoke` 在 API 28/35 验证同页反复选择、取消、离开返回后的唯一选中标记与保存值一致，原有长面板取消行仍在导航栏上方并可点击。
- 设备夹具新建宿主窗口时带 `CLEAR_TASK`，避免前一项验收留下的宿主被 `startActivitySync` 复用、无法收到新建 Activity 的回报。
- 最终 quick `20261023-043449` 通过；笔记与差异检查通过。

## Consequences

收益是弹窗始终以持久化值决定唯一选中标记，局部摘要更新仍保留。代价是弹窗打开增加一次本地后台读取，沿用页面生命周期检查；快速连续打开仍依赖现有界面对话框的点击调度。
