# Agent Note: Android 九键首帧不被旧 26 键视图覆盖

Status: implemented

## Problem

Android 键盘切换到水杉时，`runtime-options.json` 里的启动缓存可能先创建出 26 键 `view`，实时偏好中的九键设置随后才到达。实时偏好在建会话前读到时会换进建会话的选项，引擎直接按九键起来；读不到时（读取失败、`scheduleEngineStartup` 的 `complete` 回退、没有绝对路径的偏好目录），会话按偏好副本的出厂默认全拼 26 键建，`displayedTouchLayout` 采信这份视图并把 26 键写回提示，于是九键用户在第一次弹出或切换输入法时短暂看到 26 键。诊断包能确认报告者的实时配置是 `nine_key`，但不包含首帧布局事件，因此修复边界必须由生命周期和视图状态保证。

## Decision

在源头让引擎和键面一致：`startEngineSession` 没拿到实时偏好时，经 `TouchLayoutHintPolicy.startupTouchKeyboardLayout` 看已记住的触屏布局，上次确认的是全拼九键、偏好副本又是全拼 26 键时，把副本的 `touch_keyboard_layout` 换成 `nine_key` 再建会话。host-api 建会话时按这份偏好打开九键网格，视图本身就是九键，提示也不会被写回 26 键。实时偏好随后到达时，`touch_keyboard_layout` 与已应用的副本比较，用户真实设置的布局照常生效。

`displayedTouchLayout` 保持原来的规则：只有还没有视图时用记住的布局，有视图时永远按视图画。键面只能画引擎此刻开着的网格：九键键位送出的是数字，引擎没开九键网格时这些数字不进组字，`character` 返回未处理，字母键全部失效。

## Alternatives considered

- **实时偏好到达前让提示压过 26 键视图**：首帧看着对，但画出的九键和引擎的 26 键会话不一致，点网格键什么都打不出来；偏好一直读不到或没有偏好目录时整段会话都打不了字，因此不采用。
- **建会话后用 `setKeyGrid` 打开九键**：宿主设下的网格覆盖只在方案和 `touch_keyboard_layout` 不变时保留。提示过时而用户真实设置恰好是出厂的全拼 26 键时，覆盖会一直留着，把 26 键用户锁在九键上；改偏好副本则由真实偏好的布局字段自然覆盖，因此不采用。
- **在首帧前同步阻塞读取完整偏好**：会把文件读取放到输入法主线程，增加键盘弹出卡顿，不改变会话启动时序。

## Consequences

没读到实时偏好时，记住九键的全拼用户从九键起来，键面与引擎网格一致，能直接打字；无历史提示、双拼、注音、日文和其他方案的副本不动，仍是原来的 26 键启动。提示过时（真实设置已改回 26 键）时，首帧是九键，实时偏好一到就换回 26 键，不会锁住。九键双拼记下的也是九键布局，这条回退路径上先按全拼九键起来（同一种九宫格，键面印字不同），实时偏好到达后换成九键双拼；14/18 键的用户仍先看到出厂布局，与之前一致。纯逻辑冒烟测试（TouchLayoutHintPolicySmoke）固定了只改哪种组合；`startEngineSession` 里的接线（只在没有实时偏好时改副本、用记住的布局、写在 `NativeClient.create` 之前）和 `displayedTouchLayout` 有视图时按视图画，由 `platforms/android/check-host.sh` 钉住；真实 Android 编辑器验收仍由设备门禁完成。
