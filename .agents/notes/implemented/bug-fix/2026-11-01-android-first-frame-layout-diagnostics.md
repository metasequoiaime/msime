# Agent Note: Android 首帧布局诊断事件

Status: implemented

## Problem

#10389：选了全拼九宫格的用户从别的输入法切到水杉时，键盘先显示大约一秒 26 键，再跳回九键。[九键首帧不被旧 26 键视图覆盖](2026-10-31-android-nine-key-first-frame.md)（#10398）修了其中一条路径：建会话前没读到实时偏好，会话按出厂副本的全拼 26 键建起来。这条根因是推断出来的。报告者的诊断包里 `config_snapshot.json` 确认设置是 `nine_key`，但 `input_events.ndjson` 只有 `ime_start`、`ime_finish`、按键和语音几类事件，`performance_logs.ndjson` 只有 `candidate_shown`，没有任何关于建会话的记录。

同样的现象至少还有一条路径对得上，而 #10398 修不了：记住的布局（`keyboard-layout-hint`，存在 SharedPreferences 里，进程回收后还在）是 26 键，首帧按 26 键画，带着实时偏好的会话建好后才变成九键。记住的布局由 `displayedTouchLayout(view)` 每次按引擎视图写下，所以它是 26 键，说明之前某个会话的视图给出过 26 键，例如按出厂副本建起来的会话，或者停在英文模式的九键会话。只看现有的诊断包，分不清是哪一种。#10398 合并后报告者如果还能复现，下一份诊断包也还是分不清。

## Decision

建会话和第一次应用实时偏好时，往本机输入事件里记几条只有种类的事件。`DiagnosticEventKind` 在 `ENGINE_SESSION_DESTROY` 之后追加 33 种，都标为不上传。Rust `crates/client-services/src/diagnostics.rs` 的 `EVENT_KINDS` 同步加上，所以这些事件会进本机导出的诊断包；`upload::UPLOAD_EVENT_KINDS` 不加，云端上传器照旧把它们滤掉。

- **偏好来源**，在 `startEngineSession` 建好会话、按视图重建键行之后记一条：
  - `engine_session_live_preferences`：建会话前在工作线程上读到了实时偏好（`livePreferences != null`），会话按这份偏好建。记的是「读到」，不是「建好后作为快照应用成功」：应用可能失败（`liveApplied` 为 false），会话照样是按读到的偏好建的，这时 `live` 由之后的重读补记。
  - `engine_session_factory_preferences`：没读到，用 `runtime-options.json` 里的出厂副本原样建。
  - `engine_session_hinted_preferences`：没读到，出厂副本的 `touch_keyboard_layout` 已经由 `TouchLayoutHintPolicy` 按记住的九键改过。
- **触屏布局**，`touch_layout_<时机>_<布局>`：
  - `first_drawn`：会话到达之前键行画的布局（`imeLetterRows.drawnTouchLayout()`），在 `NativeClient.create` 之前取。`focus` 可能已经按会话重画了键区，所以不能等到会话建好再取。只在窗口显示着（`imeWindowVisible`）时记：真实输入框在键盘收起时获得焦点（切应用落到已聚焦的输入框），会话在 `onStartInput` 里先于窗口建好，这时键行是上一个输入框留下的（比如网址栏画的 26 键），用户没看到过，记下来会和「记住的布局是 26 键」混在一起。没有 `first_drawn` 的会话，用户看到的第一帧就是 `created`。
  - `created`：会话建好、恢复编辑器的中英状态之后，`displayedTouchLayout(view)` 给出的布局。
  - `live`：这个会话第一次应用上实时偏好之后的 `displayedTouchLayout(view)`。建会话时就应用上了（`liveApplied`），在 `startEngineSession` 的 `render()` 之后记；没应用上，就在之后第一次走到末尾、会话已经有快照的那次 `reloadPreferences` 里记。不看这次重读前有没有快照：键盘里的保存回调和 `acceptTextTrackpadPreferences` 也会换上快照，要是它们抢在第一次重读前面，只看「从没有到有」就会漏记。每个会话由 `liveTouchLayoutRecorded` 管着只记一次，`stopSessionState` 清掉标记。
  - 布局名对应 `KeyboardLayout` 的十个常量：`twenty_six_key`、`nine_key`、`japanese_nine_key`、`handwriting`、`korean`、`zhuyin`、`stroke`、`zhuyin_nine_key`、`fourteen_key`、`eighteen_key`。英文模式下画的是 26 键，记作 `twenty_six_key`。九键双拼和全拼九键是同一个触屏布局，都记作 `nine_key`。

种类由 `TouchLayoutDiagnostics` 换算：它只接受布尔值和布局常量，由固定片段拼出种类名，再经 `DiagnosticEventKind.fromWire` 换回枚举。不认识的布局返回 null，`MSIMEInputService.recordTouchLayout` 遇到 null 就不记。`ImeDebugOverlay.EventLog.line` 遇到 null 会抛异常，而这段代码在建会话的 `try` 里，抛出来会把刚建好的会话停掉。写入照旧经 `ImeDebugOverlay.recordEvent`，所以只有用户打开「记录输入日志」、当前输入框和隐私模式也允许时才写。

读诊断包时这样区分（这些事件只会出现在带 #10398 的版本上）：

- `hinted`，接着 `created_nine_key`、`live_nine_key`：实时偏好没读到，#10398 按记住的九键改了副本，生效了。这条路径上不再跳；用户还看到 26 键，跳变在别处，看同一会话有没有 `first_drawn_twenty_six_key`。
- `factory`，接着 `created_twenty_six_key`、`live_nine_key`（全拼九键用户）：实时偏好没读到，记住的布局又不是九键，#10398 改不了副本，会话按出厂的 26 键建起来，等实时偏好到了才换成九键。要看记住的布局为什么不是九键，往前找上一个会话的 `created_*` / `live_*`，最后一条 26 键的就是写下它的那次。
- `live`，接着 `first_drawn_twenty_six_key`、`created_nine_key`：实时偏好读到了，跳变来自记住的布局是 26 键，#10398 修不了。同样往前找是哪个会话写下了 26 键。
- 没有 `first_drawn`：会话在窗口显示之前建好，用户看到的第一帧就是 `created`。
- `first_drawn`、`created`、`live` 三者相同：建会话这一段没有画出别的布局，闪烁另有来源。
- 全是 `twenty_six_key`：专用英文的输入框（邮箱、网址这类，`dedicatedEnglish`）和停在英文模式的会话本来就画 26 键，九键用户在这些输入框里也会记出整串 26 键。光凭这些事件分不出它和真正的 26 键回退，要对照复现时用的是哪个输入框、什么时间。

## Alternatives considered

- **给事件加一个 `layout` 字段**：一行带上时机和布局，种类只要四五个，读起来最直接。没有采用，因为诊断包的白名单是有意定死的：输入事件只能有 `t_ms`、`kind`，最多再加 `duration_ms`，多一个键整行丢弃（`diagnostics.rs` 的 `input_event_record`）；Android 的写入方法也被 `ImePrivacyGateSmoke` 钉住，不接受任何文本参数。要加字段，就要放宽 Rust 校验、改 `msime_client.h` 里 `msime_client_diagnostic_bundle` 的契约，还要说明为什么这个字段不会带出输入内容，而 iOS 的开发者选项也经这份契约导出诊断包。布局编进种类名，和已有的 `voice_engine_local` / `voice_engine_streaming` 是同一种写法，字段白名单和 C ABI 的形状都不变。`msime_client.h` 的契约说明原来逐个列出 kind，早就漏了 `engine_session_*`、`voice_engine_*`，这次改成以 `EVENT_KINDS` 为准、只举类别，不再逐个列。
- **把布局编号塞进 `duration_ms`**：只加三种事件就够。没有采用：`duration_ms` 在所有读诊断包的地方都按毫秒解释，一个值为 1 的耗时和全拼九键读不出区别。
- **只记跳变，不记布局**：例如 `touch_layout_redrawn_at_session` 和 `touch_layout_redrawn_at_preferences`，种类少很多。没有采用：14 键、18 键、九键双拼的用户在同一条回退路径上看到的是什么（#10398 的笔记把它们写成已知未覆盖），只有记下布局本身才看得出来；跳变也能从相邻两条布局事件直接比出来。
- **布局只分 26 键、九键、其他三类**：种类从 33 个降到 12 个。没有采用，理由同上，「其他」恰好装着没覆盖的那几种用户。

## Consequences

- **收益**：#10398 合并后报告者如果还能复现，打开「记录输入日志」再导出一份诊断包，就能分清是实时偏好没读到、记住的布局是 26 键，还是闪烁出在建会话之外。14/18 键、九键双拼用户在回退路径上先看到的布局也能直接读出来。
- **代价**：`DiagnosticEventKind` 和 `EVENT_KINDS` 各多了 33 项，`ImePrivacyGateSmoke` 的种类清单跟着变长。以后给 `KeyboardLayout` 加新的触屏布局，要同时在 `TouchLayoutDiagnostics.layoutName`、`DiagnosticEventKind`、`EVENT_KINDS` 里各加三项；`TouchLayoutDiagnosticsSmoke` 会用反射找出 `KeyboardLayout` 的全部 `*_LAYOUT` 常量，漏加就失败。日志开着时，每建一次会话多写三到四行，换输入框才建会话，量和 `ime_start` 相当。
- **未覆盖**：只在 Android 上记。#10389 是 Android 切换输入法、会话重建时的偏好来源问题；iOS 和鸿蒙有没有类似的首帧回退，这次没有核对。会话先于窗口显示建好时不记 `first_drawn`，读的人只能从缺了这一条推断第一帧就是 `created`。专用英文输入框里的整串 26 键和真正的回退分不开。种类不区分 `withLivePreferences` 失败的原因（没有偏好目录、读取返回非 ok、抛异常、`complete` 回退），只记有没有读到。要是诊断包显示 `factory` 很常见，下一步再按原因拆。没有在设备上导出过一份带这些事件的诊断包。

## Verification

- `platforms/android/tests/core/TouchLayoutDiagnosticsSmoke.java`：覆盖偏好来源三选一；每个布局常量在每个时机都有不同的、不上传的种类，不认识的布局和缺了时机时返回 null；写出的行只有 `t_ms` 和 `kind`。
- `platforms/android/tests/core/ImePrivacyGateSmoke.java`：钉住完整的种类清单和顺序。
- `crates/client-services/src/diagnostics/tests.rs` 的 `first_frame_layout_events_stay_in_local_bundles`：这些种类作为输入事件进本机诊断包，带额外字段的行整行丢弃，不在上传白名单里。
- `platforms/android/check-host.sh`：钉住接线。会话到达前的布局在 `NativeClient.create` 之前取；偏好来源和两条布局在建会话里记；会话到达前的布局只在窗口显示着时取；实时偏好应用后的布局在建会话里（`liveApplied`）和 `reloadPreferences` 有快照时记，由 `liveTouchLayoutRecorded` 管着每个会话只记一次，`stopSessionState` 清掉；`recordTouchLayout` 遇到 null 不记。
