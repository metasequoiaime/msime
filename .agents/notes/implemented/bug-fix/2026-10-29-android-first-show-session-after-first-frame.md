# Agent Note: Android 键盘弹出时建会话排到第一帧之后，并记下弹出各阶段的耗时

Status: implemented

## Problem

#9916：vivo Android 11 上，每在一个新打开的应用里第一次点输入框，键盘要卡一秒左右才出来；收起后再点同一个输入框很流畅，换到另一个应用又卡一次。报告者附带的诊断包只有 `candidate_shown` 和 `ime_start` / `ime_finish`，看不出这一秒落在哪一段。

现象和会话的生命周期对得上。新打开的应用，窗口先拿到焦点时报的是 TYPE_NULL，会话推迟到键盘显示时再建（见[低端机功耗](2026-10-27-android-keyboard-low-end-power.md)）。用户点输入框时，onStartInput（真输入框）和弹出键盘几乎同时到。`scheduleEngineStartup` 先在 `preferencesWorker` 上读实时偏好、同步个人词库，再 post 回主线程跑 `startEngineSession`：`NativeClient.create`、`focus`、`setEnglishMode`、`applyPreferencesSnapshot`（含 `updatePreferences` 和皮肤）、`render`。会话句柄绑定线程（`host-api` 的 `SESSIONS` 是 thread_local），这一整段只能在主线程上做。工作线程常常赶在键盘第一帧之前做完，这段主线程工作就插到第一帧前面。系统要等输入法窗口画完第一帧才把它亮出来，所以键盘晚出来的时间就是这一整段的长度。收起再点同一个输入框时会话还在，不走这条路，所以不卡。

[会话建好之前工具栏照常可用](2026-10-29-android-toolbar-before-session.md)（#10086）已经记下 `engine_session_create` 和 `engine_session_ready`，但没有键盘进程冷启动、建键盘视图、第一帧、工作线程各段和销毁旧会话的耗时。

## Decision

不改会话的生命周期，也不把建会话挪出主线程，只调整它在主线程上的时机：键盘从收起到弹出时，建会话等这次弹出的第一帧画完再做。同时把弹出的各个阶段都记进本机诊断包。

- **开始等第一帧**：`onStartInputView` 在 `super` 之后马上调 `armFirstFrameWait`。条件由 `InputViewRefreshPolicy.armsFirstFrameWait(imeWindowVisible, keyboardRoot != null)` 判断：窗口还没显示（系统先调 onStartInputView、后调 onWindowShown），并且键盘视图已经建好。满足时记下 `firstFrameArmedAt`，在 `keyboardRoot` 的 `ViewTreeObserver` 上挂一个只触发一次的 `OnPreDrawListener`。第一次弹出时视图还没挂到窗口上，拿到的是临时观察者，挂上窗口后会并进窗口自己的那个，所以摘监听时重新取一次，并先确认它 `isAlive()`。窗口本来就显示着（同一应用里换输入框）时不等：这时没有新的一帧一定会来，等下去只会白白推迟会话。
- **建会话等第一帧**：工作线程 post 回主线程的那一步改成 `startEngineSessionAfterFirstFrame(build)`。`InputViewRefreshPolicy.waitsForFirstFrame(firstFrameArmedAt, now)` 为真时，把 `build` 存进 `sessionStartAfterFirstFrame`，否则马上建。`firstFrameDrawn` 在画帧前监听里把它 `main.post` 出去：监听还处在这一帧的遍历里，post 出去的那一步要等这一帧画完才执行。第一帧迟迟不来时（窗口刚显示就被收起、视图被重建），`firstFrameWaitTimeout` 在 `FIRST_FRAME_WAIT_MILLIS`（从 onStartInputView 算起 500 ms）时照常放行。`build` 自己核对启动代次、连接和会话，和原来 post 回主线程的那一步一样：换输入框、收起、销毁都会换代，换代后放行的 `build` 直接作废。`stop` 不清这个暂存，和原来已经 post 出去的那一步一样不受 `stop` 影响。`onWindowHidden` 把 `firstFrameArmedAt` 置为 -1，不再等这一帧，也不记这次弹出的耗时。
- **弹出各阶段的耗时**：`DiagnosticEventKind` 新增七种，只记种类和毫秒数，经 `ImeDebugOverlay.recordPerf` 写进 `perf.jsonl`。只有用户打开「记录输入日志」、隐私判断也允许时才写；都标为不上传，Rust 的 `diagnostics::EVENT_KINDS` 同步加上，所以会进本机导出的诊断包。
  - `ime_service_create`：`onCreate` 整段，键盘进程被回收后下一次弹出先走这里。
  - `input_view_create`：`onCreateInputView` 整段。
  - `keyboard_first_draw`：从开始等第一帧到第一帧画帧前。
  - `engine_startup_preferences`、`engine_startup_dictionary_sync`：工作线程上 `withLivePreferences` 和个人词库同步（连同 `DictionaryCollectionsStore.flushSent`）各自的耗时，在 post 回主线程、核对过代次后记。
  - `engine_session_main`：主线程上从进入 `startEngineSession` 到按实时偏好画完的整段，这段时间里点按都在排队。
  - `engine_session_destroy`：`stop` 里在主线程上提交组字并销毁旧会话（Rust 在这里写出选词和延迟学习），切应用时它排在新输入框弹出键盘之前。

## Alternatives considered

- **把 `NativeClient.create` 和应用偏好挪到工作线程**：主线程上的这一整段就没了，收益最大。没有采用，因为会话句柄绑定线程，`with_session` 在错的线程上直接报「unknown session or wrong thread」。要挪，就得让之后所有逐键调用都跟到同一个线程上，等于把输入链路整个搬走，不属于这次修复的范围。
- **把会话跨输入框保留，切应用时不重建**：从根上消掉这一段。没有采用，因为换输入框时销毁会话是词库维护拿独占锁的时机（`onFinishInput` 里的个人词库同步和快照处理），[会话建好之前工具栏照常可用](2026-10-29-android-toolbar-before-session.md)里也是因为这一点没有做。
- **常驻进程或前台服务保活**：报告者希望在「正在运行的服务」里看到常驻进程。没有采用：`:ime` 进程的存活由系统和厂商的进程管理决定，用前台服务强行保活不符合平台规范，还会增加耗电。而且它只能省掉进程冷启动那一段，切应用时的会话重建照样要做。
- **先只加耗时记录，等拿到数据再修**：最稳妥。没有只做这一步，是因为建会话插到第一帧前面这件事从代码里就能确定，而且改动只调整了时机：第一帧画完立刻建，用户在键盘出来之前本来也点不了键，最坏的情况是第一帧不来、兜底时间到了再建。耗时记录照样加上，用来判断剩下的时间花在哪一段。

## Consequences

- **收益**：在新应用里点输入框时，键盘第一帧不再等建会话这一整段。下次再有「键盘弹出来要等」的反馈，诊断包能把时间拆到冷启动、建视图、第一帧、工作线程上的偏好读取和词库同步、主线程建会话、销毁旧会话各段。
- **代价**：工作线程先做完时，会话要到第一帧之后才可用，`engine_session_ready` 里包含这段等待。第一帧画完后主线程仍要跑完建会话的整段，这段时间里的点按仍然排队，只是排在键盘出现之后。这段有多长要看 `engine_session_main` 的数据；如果它本身就要一秒，还得另立计划处理。
- **未覆盖**：验证只有 `InputViewRefreshPolicySmoke`、`ImePrivacyGateSmoke`、`check-host.sh` 的接线守卫，以及 `msime-client-services` 的诊断包测试。没有在设备上量过修改前后的第一帧，也没有在 vivo Android 11 上回归，需要 API 30 低配模拟器或真机，并请报告者在下一个版本里导出一次诊断包确认。iOS 和鸿蒙的同类情况没有核对。

## Verification

- `platforms/android/tests/core/InputViewRefreshPolicySmoke.java`：覆盖什么时候开始等第一帧、什么时候等、兜底的剩余时间。
- `platforms/android/tests/core/ImePrivacyGateSmoke.java`：钉住新增的七个事件种类。
- `crates/client-services/src/diagnostics/tests.rs` 的 `engine_session_timing_records_stay_in_local_bundles`：确认这些性能记录进本机诊断包，并且不在上传白名单里。
- `platforms/android/check-host.sh`：钉住宿主接线，包括 onStartInputView 一开头就开始等、工作线程交回的建会话经 `startEngineSessionAfterFirstFrame` 并在放行时核对代次、第一帧画完把它 post 到这一帧之后，以及各阶段耗时的记录点。
