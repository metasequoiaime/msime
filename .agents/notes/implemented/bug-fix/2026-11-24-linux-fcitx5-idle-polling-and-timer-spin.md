# Agent Note: Fcitx5 空闲零轮询、定时器紧循环修复与模型缓存复用

Status: implemented

## Problem

Linux Fcitx5 宿主与共享 `host-api` 在真实桌面下同时暴露出三类空转与重复开销：

- **Mode-HUD 单次定时器 ~50 kHz 紧循环**：在没有 `MSIME_FCITX5_CUSTOM_IM_INFORMATION` 的旧版 Fcitx5 开发头回退路径中（见[Linux Fcitx5 低版本开发头保留模式提示](2026-11-09-linux-fcitx5-mode-hud-compat.md)），`mode_hud_aux_timer_` 的回调只调用 `timer->setOneShot()` 而没有推进截止时间。在 `fcitx::EventSourceTime` / `sd-event` 下，不推进 `time()` 直接 `setOneShot()` 会按已经过期的 `CLOCK_MONOTONIC` 时间戳立即重新武装 `timerfd`，导致主循环以 ~50 kHz 持续唤醒；失焦或关闭会话时也没有同步清掉该定时器与辅助行状态。同类边界风险也存在于 `wave_overlay_frame_timer_`、`caret_forward_event_`（初值 0 延时且无总时长上限）与 `NativeCanvasX11`（0 ms 动画帧延时且回调未显式关闭）。
- **每上下文 8 Hz+ 常驻空闲轮询**：尽管[Fcitx5 下失焦的输入框不再每拍跑整套刷新](2026-11-13-linux-unfocused-context-idle-work.md)在失焦时跳过了外部 ABI 往返，每个 `FcitxState` 在构造时仍无条件启动每 250 ms 一拍的 `preferences_timer_`，且无论是否空闲都在回调末尾重新武装 250 ms 定时器。只要用户打开过多个窗口，进程仍持续以 4–8 Hz+ 被唤醒。
- **焦点切换反复重载模型与诊断刷屏**：Fcitx5 失焦时会调用 `close()` 销毁 `HostSession`、聚焦时重建新会话（见 `platforms/linux/fcitx5/tests/native.cpp`）。预览会话（`SessionUse::Preview`）或禁用了逐键重排的镜像会话（`!keyboard_reranker_allowed`）在建会话或刷新偏好时调用了 `keyboard_reranker(false, ...)`，触发 `onnx_rerankers().release(root)` 过早驱逐进程级 `OnnxRerankers` 缓存；同时每次 `create_session` 都无条件起一条 `msime-keyboard-reranker` 操作系统线程去碰缓存，而 `report_keyboard_reranker` 与 `report_settled_model`（见[落定重排模型接线状态写进诊断日志](2026-10-31-settled-model-diagnostics.md)）在每次焦点重建时重复向 stderr 输出完全相同的状态行。

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

### 3. `host-api` 模型缓存保护、热缓存零线程槽与诊断去重

- **停止误驱逐进程级缓存**：在 `crates/host-api/src/ffi/session.rs` 与 `crates/host-api/src/lib.rs` 中，构建 `SessionUse::Preview`（`self.preview`）或 `!keyboard_reranker_allowed` 会话时仅执行 `runtime.set_candidate_scorer(None)`，不再调用 `keyboard_reranker(false, ...)` 触发 `onnx_rerankers().release(root)`；仅对允许逐键重排但用户关闭了开关的 `Input` 会话调用 `keyboard_reranker(false, ...)` 释放缓存。
- **热缓存命中直接返回预填充槽，不启操作系统线程**：在 `crates/host-api/src/ffi.rs` 新增 `cached_keyboard_reranker_for(...) -> Option<Option<MountedReranker>>`、`OnnxRerankers::attach_cached` 与 `cached_bundled_keyboard_reranker`。当 `OnnxRerankers` 或 `SENTENCE_MODELS` + `MEASURED` 已持有可用结论时，`load_keyboard_reranker`（`crates/host-api/src/lib.rs`）直接返回预填充的 `KeyboardRerankerSlot`（`Arc::new(Mutex::new(Some(scorer)))`），不创建后台线程，同时继续沿用[逐键重排器在后台挂，不堵输入线程](2026-11-03-async-keyboard-reranker-attach.md)的 `apply_pending` 挂载路径。
- **连续重复诊断去重**：在 `report_keyboard_reranker`（`crates/host-api/src/ffi.rs`）与 `report_settled_model`（`crates/host-api/src/lib.rs`）中通过 `should_report_keyboard_reranker_diagnostic` 等状态抑制连续完全相同的 `(category, detail)` 输出（比较逐键重排器细节时剥离末尾 `" (shared)"` 后缀，同时保持 `AttachedOnnxReranker.detail` 原值）；当 `RerankerMount::Off` 释放缓存（`clear_keyboard_reranker_diagnostic_dedup()`）或在 `rebuild` 中关闭 `neural_desktop`（`clear_settled_model_diagnostic_dedup()`）时清除去重状态，使状态切换后再次开启能重新报告。新增 `#[cfg(test)] pub(crate) fn reset_model_diagnostic_dedup()` 并在 `capture_diagnostics()`（`crates/host-api/src/tests.rs`）调用，保证现有诊断单测隔离。

## Alternatives considered

- **在焦点切换时保留所有 `InputContext` 的 `HostSession` 不关闭**
  - 最强理由：每个窗口保留各自存活的会话实例后，切回旧窗口无需重建 `HostSession`，天然不会再次加载模型、起线程或重复写模型诊断，实现上也最直接。
  - 否决原因：直接破坏 `platforms/linux/fcitx5/tests/native.cpp` 锁定的 Fcitx5 失焦关闭会话、聚焦重建会话的生命周期契约，且会让用户打开过的每个应用窗口长期持有各自的词库租约与引擎内存。
- **在 `SystemTheme.cpp` 中改用 GIO `g_dbus_connection_signal_subscribe` 订阅主题信号**
  - 最强理由：通过 D-Bus 信号推送替代定时探测 `org.freedesktop.portal.Settings`，可以连带去掉系统明暗主题的周期唤醒。
  - 否决原因：Fcitx5 插件主循环运行在 `sd-event`（`fcitx::EventLoop`）而非 `GMainContext` 上，GIO 信号回调无法安全直接派发到插件事件循环；同时 `platforms/linux/tests/core/fcitx5_contract.py` 显式校验 `setNextInterval(stepSystemTheme())` 契约，不能绕开该步进接口。
- **在 `create_session` 命中热缓存时同步把重排器直接挂入 `runtime`**
  - 最强理由：模型实例已经在内存缓存中，直接 `runtime.set_candidate_scorer(...)` 可省去一层 `KeyboardRerankerSlot` 包装与后续 `apply_pending` 的换入步骤。
  - 否决原因：会破坏 `crates/host-api/src/tests.rs` 中 `msime_client_create` 返回后即刻断言 `session.keyboard_reranker_loading.is_some() && !session.runtime.has_reranker()` 的既有不变量；改由 `load_keyboard_reranker` 返回预填充槽，既做到热路径零操作系统线程、零磁盘重载，又完整守住该不变量。

## Consequences

- **收益**：
  - Fcitx5 空闲无输入时不再有 8 Hz+ 轮询或 `mode_hud_aux_timer_` ~50 kHz 空转，外部文件改动经单个插件级 `inotify` IO 事件源在 50 ms 防抖后按需唤醒刷新。
  - 窗口间切换焦点即使销毁并重建 `HostSession`，只要 `OnnxRerankers` / `SENTENCE_MODELS` 已缓存且非预览/禁用会话释放，就不再生出后台加载线程，也不再向 stderr 刷重复的 `settled model` / `keyboard reranker` 行。
- **代价与约束**：
  - 外部对监听目录或文件的极端不触发 `inotify` 的改动（如某些特殊网络文件系统或容器挂载，且 `inotify_init1` 本身未失败时）依赖后续聚焦/按键路径或用户操作触发刷新；`inotify_init1 < 0` 时保留兜底定时器。
  - `FcitxIdleWatch` 独立于 `nlohmann/json`，确保测试可在仅安装 `Fcitx5Core`/`Fcitx5Utils` 的宿主环境直接用 `g++` 编译运行，后续扩充监听路径需同步维护它的路径登记接口。
- **关联演进**：本笔记细化并部分取代[Fcitx5 下失焦的输入框不再每拍跑整套刷新](2026-11-13-linux-unfocused-context-idle-work.md)中「保留每上下文 250 ms 定时器并在空转拍跳过」的折中做法，将该文未做的「消除定时器常驻周期唤醒」与全插件事件监听一并落地。

## Verification

- `platforms/linux/fcitx5/tests/mode_hud_deferred_update.cpp`：以 `g++ -std=c++20 -Wall -Wextra -Werror` 独立编译运行通过（`mode_hud_deferred_update: PASS`），验证单次定时器回调停止后不会再次武装。
- `platforms/linux/fcitx5/tests/timer_idle_and_oneshot.cpp`（与 `platforms/linux/fcitx5/FcitxIdleWatch.cpp`）：以 `g++ -std=c++20 -Wall -Wextra -Werror` 链接真实 `libFcitx5Utils.so.2` 运行通过（`Fcitx5 一次性定时器不重装、空闲零唤醒与 inotify 防抖/回退回归通过`），验证 `sd-event` 单次定时器不重武装、空闲零唤醒以及 `FcitxIdleWatch` 的 `inotify` 50 ms 防抖与降级回退。
- `python3 platforms/linux/tests/core/fcitx5_contract.py`：输出 `Fcitx5 addon metadata passed`，验证 `wave_overlay_frame_timer_`、`stepSystemTheme()`、`refreshProviderSockets()` 等契约守卫通过。
- `crates/host-api/src/tests/model_reload_dedup.rs`：覆盖热缓存预填充槽不新建后台线程、镜像/预览会话不驱逐 `OnnxRerankers` 缓存，以及诊断去重与状态翻转后重放（由 `cargo test -p msime-host-api --lib model_reload_dedup`、`settled_model`、`keyboard_reranker` 门禁执行）。
- `npm run verify-notes`：校验 `.agents/notes/` 目录结构、相对链接与格式规范全部通过。
