# Agent Note: Fcitx5 空闲零轮询与定时器紧循环修复

Status: implemented

## Problem

Linux Fcitx5 宿主在真实桌面下暴露出两类空转：

- **Mode-HUD 单次定时器 ~50 kHz 紧循环**：在没有 `MSIME_FCITX5_CUSTOM_IM_INFORMATION` 的旧版 Fcitx5 开发头回退路径中（见[Linux Fcitx5 低版本开发头保留模式提示](2026-11-09-linux-fcitx5-mode-hud-compat.md)），`mode_hud_aux_timer_` 的回调只调用 `timer->setOneShot()` 而没有推进截止时间。在 `fcitx::EventSourceTime` / `sd-event` 下，不推进 `time()` 直接 `setOneShot()` 会按已经过期的 `CLOCK_MONOTONIC` 时间戳立即重新武装 `timerfd`，导致主循环以 ~50 kHz 持续唤醒；失焦或关闭会话时也没有同步清掉该定时器与辅助行状态。同类边界风险也存在于 `wave_overlay_frame_timer_`、`caret_forward_event_`（初值 0 延时且无总时长上限）与 `NativeCanvasX11`（0 ms 动画帧延时且回调未显式关闭）。
- **每上下文 8 Hz+ 常驻空闲轮询**：尽管[Fcitx5 下失焦的输入框不再每拍跑整套刷新](2026-11-13-linux-unfocused-context-idle-work.md)在失焦时跳过了外部 ABI 往返，每个 `FcitxState` 在构造时仍无条件启动每 250 ms 一拍的 `preferences_timer_`，且无论是否空闲都在回调末尾重新武装 250 ms 定时器。只要用户打开过多个窗口，进程仍持续以 4–8 Hz+ 被唤醒。

## Decision

### 1. 停止 `sd-event` 单次定时器重武装与周边定时器加固

- `showInputModeHud` 的 `#else` 分支（`platforms/linux/fcitx5/FcitxEngine.cpp`）及 `platforms/linux/fcitx5/tests/mode_hud_deferred_update.cpp` 改用 `timer->setEnabled(false);` 停止已触发的单次定时器，不再在过期时间戳上调用 `timer->setOneShot()`。
- `FcitxState::close()` 与 `FcitxState::leaveFocus()` 在 `#ifndef MSIME_FCITX5_CUSTOM_IM_INFORMATION` 下同步递增 `mode_hud_aux_id_++`、重置 `mode_hud_aux_timer_.reset()` 并清空 `mode_hud_aux_text_.clear()`，防止失焦后残留过期回调。
- `wave_overlay_frame_timer_` 保留 `auto *const self = this;` 与 `if (self->wave_overlay_frame_timer_.get() == timer)` 契约（`platforms/linux/tests/core/fcitx5_contract.py` 校验项），仅在 `self->wave_overlay_surface_ && self->wave_overlay_visible_ && self->wave_overlay_.animates()` 时续排下一帧，否则显式调用 `timer->setEnabled(false);`。
- `caret_forward_event_` 将首次调度延时从 0 改为 `fcitx::now(CLOCK_MONOTONIC) + 1000`（1 ms），并以 `caret_forward_deadline_` 将 Wayland Shift 释放轮询的总生命周期封顶在 2 秒，同时在 `leaveFocus()` 与 `close()` 中重置该事件源和截止时间。
- `NativeCanvasX11::scheduleAnimation`（`platforms/linux/src/candidates/NativeCanvasX11.cpp`）在回调开头显式调用 `timer->setEnabled(false)`，并用 `std::max<std::uint64_t>(1, *next)` 钳制帧间隔，避免 0 ms 帧造成紧循环。

### 2. `preferences_timer_` 改为按需调度与 `FcitxIdleWatch` 事件驱动

- `FcitxState` 构造函数不再无条件启动 `preferences_timer_`。
- 新增按需调度 `scheduleAsyncPoll()`（以及 `hasActiveAsyncPollWork()`、`stopOrScheduleAsyncPoll()`、`pollAsyncState()`）：仅当存在在途异步任务或防抖时才武装 250 ms；若仅剩待刷盘按键（`KeyPressCounter::flush_delay_microseconds()`，`platforms/linux/src/system/TypingStatistics.h`，周期 `KeyPressCounter::kFlushAgeMicroseconds` = 30 s）或主题/画布 10 s 重试，则直接按精确剩余截止时间单次武装；完全空闲时及 `leaveFocus()` / `close()` 且无在途偏好任务时调用 `timer->setEnabled(false)`，实现空闲 0 Hz 唤醒。
- 保持 `refreshPreferences()`、`refreshProviderSockets()` 与 `stepSystemTheme()` 的既有宿主契约不变（`platforms/linux/tests/core/fcitx5_contract.py` 校验 `setNextInterval(stepSystemTheme())`）。
- 新增无 `nlohmann/json` 依赖的独立模块 `platforms/linux/fcitx5/FcitxIdleWatch.h` 与 `platforms/linux/fcitx5/FcitxIdleWatch.cpp`（`FcitxWatchConfig`、`buildBaseFcitxWatchConfig()`、`appendContextWatchPaths()`）：整个 Fcitx5 插件共用单个 `inotify_init1(IN_NONBLOCK | IN_CLOEXEC)` 实例，通过 `fcitx::EventLoop::addIOEvent` 监听外部配置与运行时路径（`runtime-options.json`、`options_path_`、`dictionary_user_data_` 静默租约、`$XDG_RUNTIME_DIR/msime-client` 套接字、`$XDG_CONFIG_HOME/fcitx5/conf/classicui.conf`、`clipboard_path_`），触发后经 50 ms 单次防抖（回调内 `setEnabled(false)`）通知 `FcitxEngine` 刷新活动上下文或托盘目标上下文（经 `notifyIdleWatch()` 同步）；仅在 `inotify_init1 < 0` 时退回兜底定时器。
- `scheduleAsyncPoll()` 只把 `preferences_timer_` 的截止时间往前挪：计时器已启用、且截止时间不晚于这次算出的时间时不动它。`render()` 和按键统计每次视图更新都会调它，若每次都 `setTime(now + 250ms)`，按键间隔短于 250 ms（正常打字 125–200 ms）时 `pollAsyncState()` 永远等不到，在线和 AI 候选的收集、云查询防抖、翻译、剪贴板、表情和云剪贴板任务都要等停手才推进。
- `FcitxIdleWatch` 的偏好目录监听忽略私有文件原子写入的临时文件 `.msime-private-*`（`crates/client-base/src/storage.rs` 的 `private_temporary_name`，改名落地时目标文件名上的事件照常算）。`typing-statistics.json` 插件自己每次刷新统计都会改写，但它同时载着统计总开关，设置应用关掉统计时也只改这一个文件，所以不能直接忽略：只有它变了时走 `setStatisticsCallback` 注册的回调，`FcitxEngine::onTypingStatisticsChange()` 在后台线程里重读统计开关（`TypingStatisticsSwitch::refresh`），不重读偏好和服务套接字；同一个防抖窗口里还有别的外部变化时只跑整套重载，它本身会重读统计开关。词库同步和设置同步的状态在偏好目录的子目录 `dictionary-sync/`、`settings-sync/` 里，监听不递归，本来就不触发。

## Alternatives considered

- **偏好目录监听直接忽略 `typing-statistics.json`**
  - 最强理由：它是插件自己约每 30 秒写一次的文件，忽略后连续打字时不再顺带触发整套偏好重载，实现只多一个文件名。
  - 否决原因：统计总开关存在这份文档里，不在 `preferences.json`；设置应用关掉统计时只改它。直接忽略后，关掉统计要等到下一次不相干的偏好重载才被插件看到，这期间上屏的字仍会记进统计。改走只重读开关的回调，代价是一次 `stat` 加一次后台读取。
- **在 `SystemTheme.cpp` 中改用 GIO `g_dbus_connection_signal_subscribe` 订阅主题信号**
  - 最强理由：通过 D-Bus 信号推送替代定时探测 `org.freedesktop.portal.Settings`，可以连带去掉系统明暗主题的周期唤醒。
  - 否决原因：Fcitx5 插件主循环运行在 `sd-event`（`fcitx::EventLoop`）而非 `GMainContext` 上，GIO 信号回调无法安全直接派发到插件事件循环；同时 `platforms/linux/tests/core/fcitx5_contract.py` 显式校验 `setNextInterval(stepSystemTheme())` 契约，不能绕开该步进接口。

## Consequences

- **收益**：
  - Fcitx5 空闲无输入时不再有 8 Hz+ 轮询或 `mode_hud_aux_timer_` ~50 kHz 空转，外部文件改动经单个插件级 `inotify` IO 事件源在 50 ms 防抖后按需唤醒刷新。
  - 连续打字时异步轮询仍按 250 ms 一拍推进：`scheduleAsyncPoll()` 只把截止时间往前挪，已排好的一拍不会被每次按键往后推。
- **代价与约束**：
  - 外部对监听目录或文件的极端不触发 `inotify` 的改动（如某些特殊网络文件系统或容器挂载，且 `inotify_init1` 本身未失败时）依赖后续聚焦/按键路径或用户操作触发刷新；`inotify_init1 < 0` 时保留兜底定时器。
  - `FcitxIdleWatch` 独立于 `nlohmann/json`，确保测试可在仅安装 `Fcitx5Core`/`Fcitx5Utils` 的宿主环境直接用 `g++` 编译运行，后续扩充监听路径需同步维护它的路径登记接口。
- **关联演进**：本笔记细化并部分取代[Fcitx5 下失焦的输入框不再每拍跑整套刷新](2026-11-13-linux-unfocused-context-idle-work.md)中「保留每上下文 250 ms 定时器并在空转拍跳过」的折中做法，将该文未做的「消除定时器常驻周期唤醒」与全插件事件监听一并落地。

## Verification

- `platforms/linux/fcitx5/tests/mode_hud_deferred_update.cpp`：以 `g++ -std=c++20 -Wall -Wextra -Werror` 独立编译运行通过（`mode_hud_deferred_update: PASS`），验证单次定时器回调停止后不会再次武装。
- `platforms/linux/fcitx5/tests/timer_idle_and_oneshot.cpp`（与 `platforms/linux/fcitx5/FcitxIdleWatch.cpp`）：以 `g++ -std=c++20 -Wall -Wextra -Werror` 链接真实 `libFcitx5Utils.so.2` 运行通过（`Fcitx5 一次性定时器不重装、空闲零唤醒与 inotify 防抖/回退回归通过`），验证 `sd-event` 单次定时器不重武装、空闲零唤醒以及 `FcitxIdleWatch` 的 `inotify` 50 ms 防抖与降级回退。
- `python3 platforms/linux/tests/core/fcitx5_contract.py`：输出 `Fcitx5 addon metadata passed`，验证 `wave_overlay_frame_timer_`、`stepSystemTheme()`、`refreshProviderSockets()` 等契约守卫通过。
- `platforms/linux/fcitx5/tests/native.cpp --async-poll`（ctest `fcitx5-async-poll-while-typing`）：每 100 ms 模拟一次按键、反复调 `scheduleAsyncPoll()`，1.2 秒内 `pollAsyncState()` 至少要跑 3 次；旧实现一次也跑不到。
- `platforms/linux/fcitx5/tests/timer_idle_and_oneshot.cpp` 同时覆盖 `.msime-private-*` 临时文件不触发回调、打字统计文档只触发统计开关回调、与其他外部变化同窗口时只触发整套回调。
- `npm run verify-notes`：校验 `.agents/notes/` 目录结构、相对链接与格式规范全部通过。
