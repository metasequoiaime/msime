# Agent Note: Android 按键路径拆段计时

Status: implemented

## Problem

#10457 的报告者在 vivo V2046A（Android 11）上用九键全拼打字，觉得候选出来得慢。诊断包里每键只有一条 `candidate_shown`：从手指抬起（`ImeKeyFeedback` 调 `ImeDebugOverlay.onKeyPressed`）量到 `MSIMEInputService.apply` 里的 `showDiagnostic`。这一刻引擎已经在主线程上同步算完、`bridge.apply` 已经把组字写进输入框，但 `render()` 还没开始。所以这个数既分不出引擎和写输入框各占多少，也没算上 `render()` 改候选条和键盘视图的那一段（每次结尾还有 `imeStyler.applySkin()` 整树上色）。用户实际等的时间比它长，而要决定下一步先优化引擎还是先优化 `render()`，正需要这两段分开的数。

## Decision

加两个只进本机导出诊断包的性能种类，追加在 `DiagnosticEventKind` 末尾（构造参数 `false`，不改已有种类的序号），`crates/client-services/src/diagnostics.rs` 的 `EVENT_KINDS` 同步加上，不进 `upload.rs` 的 `UPLOAD_EVENT_KINDS`：

- `engine_call`：一次按键在主线程上同步调引擎（JNI 进出）的耗时。`character`、`touchCharacter`、`gridKey`、`command` 四个入口在 `NativeClient` 调用前取 `ImeDebugOverlay.engineCallStart()`，返回后调 `onEngineCall`。只有正在量一次按键（`pendingKeyAt >= 0`）时才取时刻、才记；`showDiagnostic` 清掉 `pendingKeyAt` 之后同一次处理里的后续调用不再记，所以每条 `engine_call` 都对应同一次按键的那条 `candidate_shown`。
- `candidate_rendered`：从按键到 `render()` 改完视图的耗时。`showDiagnostic` 把按键时刻转给 `renderPendingAt`，`render()` 拆成外层和 `renderViews()`，外层在 `renderViews()` 返回后调 `onRendered()`。`render()` 在批处理里被推迟（`renderBatchDepth > 0`）时不记，等批处理结束补的那次 render 再记。手指按着候选、候选条推迟重画（`candidateRenderDeferred`）时也不记，等松手后补的那次。下一次按键和 `clearDiagnostic` 会清掉没记出去的时刻，免得把下一次按键算进来。

`candidate_shown` 的口径不变，三条放在一起看：`engine_call` 是引擎本身，`candidate_shown − engine_call` 是写输入框和 `apply` 里其余的工作，`candidate_rendered − candidate_shown` 是 `render()`。`candidate_rendered` 不包括之后这一帧的测量、布局和绘制，那一段仍然看不到。

行格式和其余性能记录一样，只有 `{t_ms, kind, duration_ms}`，写入方法不收任何字符串参数（`ImePrivacyGateSmoke` 反射核对），隐私门和日志开关与原来相同。

模糊音的影响用 `crates/engine/examples/ninekey_latency.rs` 量：`NINEKEY_FUZZY=1` 打开全部 11 条模糊音规则（报告者的设置），`NINEKEY_WORD_LATTICE=0` 关整句。2026-11-02 在 Mac Studio（M1 Max，`/Applications/MSIME.app` 带的桌面词库，release，20 轮去掉第一轮预热，按「关、开、开、关」顺序各跑一次，负载 9–13）上，模糊音全开时所有数字键的 p50/p95：

| 整句 | 模糊音关 | 模糊音全开 |
|---|---|---|
| 标准 | 2.74–2.82 / 6.26–6.43 ms | 4.23–4.37 / 9.85–10.22 ms |
| 关闭 | 1.50–1.51 / 2.87–3.03 ms | 2.46–2.47 / 5.54 ms |

开模糊音比关着慢 1.5 到 1.9 倍，组字越长差得越多（整句关时第 47 位 p50 4.3 → 7.9 ms）。引擎这一侧的改动见[模糊行缓存命中不再展开路径](../testing/2026-11-02-fuzzy-row-cache-path-count.md)。

## Alternatives considered

- **只延长 `candidate_shown` 量到 `render()` 结束**：一个数就够看总时长，但服务端白名单里的 `candidate_shown` 是能上传的种类，改了它的口径，新旧版本的上传数据就没法比；而且仍然分不出引擎和 `render()`。
- **`candidate_rendered` 量到这一帧交出去（帧提交回调）**：更接近用户看到候选的时刻，键盘首帧（`keyboard_first_draw`）就是这么量的。但每键都挂一个帧提交回调，按键没引起重画时回调要等到下一帧才来，会把不相干的等待算进去；API 28 还没有这个回调。先量主线程上能确定的这一段，帧内的开销等这两个数出来再决定要不要量。
- **把引擎查询挪到会话线程异步执行（照 iOS #10172）**：这是解决主线程占用的办法，不是量的办法；iOS 真机数据显示它改善帧间隔，候选完成时间的 p95 没有变快。规模大，维护者决定暂缓，先用这两个种类看清楚时间花在哪。

## Consequences

收益：报告者升级后重新导出诊断包，就能按键对上 `engine_call`、`candidate_shown`、`candidate_rendered` 三条，判断该先优化引擎还是 `render()`/`applySkin`。不开「记录输入日志」时什么也不写，每键只多两次 `SystemClock.uptimeMillis()`。

代价：开着日志时每键多写两行 perf 记录（原来一行），`perf.jsonl` 的 1 MiB 环形上限能存的按键数减到约三分之一。诊断包的种类清单多两项，`EVENT_KINDS`、`DiagnosticEventKind`、`ImePrivacyGateSmoke` 的种类表和 `msime_client.h` 的注释要一起改。经 `apply` 以外的入口（选候选、九键拼音列等）出候选的按键只有 `candidate_shown` 和 `candidate_rendered`，没有 `engine_call`。

验证：`cargo test -p msime-client-services diagnostics`（含 `engine_session_timing_records_stay_in_local_bundles` 加上的两个种类）和 `platforms/android/check-host.sh`（`ImePrivacyGateSmoke` 的种类表、行格式和「不收文字」反射检查）在 Mac Studio 上通过。`DiagnosticsBundleDeviceSmoke` 加了两行新种类的合成 perf 记录并核对 `performance_logs` 保留 3 行，但这台机器上没有跑设备测试；真机上三段耗时各是多少，要等报告者在新版本上导出诊断包。
