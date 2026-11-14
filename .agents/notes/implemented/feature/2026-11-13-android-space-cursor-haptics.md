# Agent Note: Android 空格滑动的逐步振动

Status: implemented

## Problem

空格横滑缺少逐步振动，而共用的光标移动方法还服务于底栏拖动，不能直接给所有入口加反馈。

## Decision

仅空格滑动调用 `ImeSpaceCursor`，沿用现有距离阈值和[文本边界过滤](../bug-fix/2026-10-29-android-text-edit-cursor-edges.md)。复用 `sendEditorKeyEvents()` 发键并返回整对事件是否成功；成功且连接与手势仍有效时才振动。缓存失效和失败时尽力松键仍由原发送方法负责。

复用振动总开关、强度和 `KeyboardHaptics.play()` 的[忙时丢弃策略](../bug-fix/2026-10-27-mobile-haptics-off-ui-thread.md)。不增加设置、默认值变化、声音、动画或逐步统计；底栏拖动及原空格手势仲裁不变。

已有[上滑复制](2026-10-23-android-space-swipe-copy.md)、[底栏](2026-10-08-android-keyboard-bottom-bar.md)和[触控板](2026-10-23-android-text-edit-trackpad.md)笔记部分相关，继续保留各自行为。已合并的 #11366 属于上游基线；空格不使用触控板的 `playSteps()` 队列。

## Alternatives considered

- 共用方法无条件加振动：改动最短，但会改变底栏拖动。
- 每个 MOVE 事件只振一次：接线简单，但一次事件可能跨越多个有效步。

## Verification

- `test-android-space-cursor-haptics.py` 执行真实空格反馈逻辑，模拟边界过滤与发送接口的结果，覆盖步数、失败、取消、连接变化及振动偏好；已有守卫、边界策略和振动调度回归继续覆盖底层行为。
- 更新基线前的 [Actions 43303563390](https://github.com/wxupjack/msime/actions/runs/43303563390) 通过 quick（含脚本规定的跳过项）、348 项 JVM 冒烟、281 个设备源码编译、振动专项、APK 构建和 lint。vivo V2545A / Android 17 上确认短按、长按语音、上滑复制、分离式空格及新增振动正常。
- 基线更新和本次精简后尚未复验行为（本机缺少 JDK），按用户要求未重跑 quick 或构建 APK；上述结果仅对应先前测试包。

## Consequences

不新增设置或发键实现，只维护空格入口的反馈判断。发送成功可能仅表示事件已投递，不保证第三方编辑器实际移动；快速拖动时允许丢弃繁忙期间的反馈，避免振动积压。
