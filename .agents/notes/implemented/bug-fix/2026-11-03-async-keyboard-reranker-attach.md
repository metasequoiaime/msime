# Agent Note: 逐键重排器在后台挂，不堵输入线程

Status: implemented

## Problem

2026-11-03 在 macOS 上装好新构建后马上打字，输入法进程刚起来的那几秒里，按键有的丢了，有的被原样插进文字（#10743）：

- 第 3 段上屏成字母 `ndehuodongyezhunb`；
- 第 5 段丢了开头的 `you`。

诊断日志里能看到原因：第一个会话打开「增强」时，`ffi::keyboard_reranker` 在输入线程上加载 ONNX 模型并自测。两个模型都装着时（#10732 之后的桌面），要先加载大模型自测，再加载小模型自测，各跑 4 次图，空闲的 Mac 上要 1.1 秒（12:19:51.004 → 12:19:52.126），这之后的第一个键处理花了 134 ms。机器忙时卡得更久，超过 IMK 的等待时限，应用就把键当作输入法没处理，原样插进了文字。

## Decision

逐键重排器改在后台线程里挂（`load_keyboard_reranker`），结果放进会话的 `keyboard_reranker_loading` 槽。挂好之前照常打字，只是不排序。槽有值之后，下一次输入空闲时由 `apply_pending` 换上，和落定模型（`load_settled_model`）同一个做法；组字中途不换，免得整句池突然一变、正在看的候选跟着重排。

有三个地方原来同步调用 `ffi::keyboard_reranker(true, …)`，现在都改成起后台线程：

- 会话建立（`ffi/session.rs`）；
- 偏好打开「增强」（`apply_preferences`）；
- 聚焦时模型来源变了（资源包刚装好或被移除）。

关掉「增强」时照旧当场放开。判断「已经挂着」时把正在挂的也算上，否则每次送偏好都会再起一个线程。

同一个进程里，模型只有第一次要加载和自测，之后再挂是共用已加载的那一份，几毫秒就完成。所以实际受影响的只是输入法刚启动的那一下。

## Alternatives considered

- **只把自测挪到后台，加载留在输入线程。** 加载本身（两个模型，几十 MB）也要几百毫秒，同样会堵键。
- **自测结果落盘，下次启动直接用。** 能省掉自测，但加载照样在输入线程上；机器换了、模型换了也要判断缓存失效。后台挂已经解决了堵键，落盘不再需要。
- **挂好后立刻换上，不等空闲。** 能早几个键用上模型，但组字中途换打分器，整句池会变（6 条 → 32 条），候选会在用户眼前跳一下。

## Consequences

- **收益**：输入法刚启动时按键不再被模型加载堵住。真机上，杀掉进程后马上打字，主线程不再有 1 秒多的停顿，最慢的一个键是 81 ms（首次打开词库），没有丢键。
- **代价**：刚启动时打的第一句（挂好之前那几个键）不经过逐键模型，按引擎自己的顺序排，上屏后的下一句就用上了。
- **边界**：关掉「增强」时后台线程如果还在加载，它仍会把模型放进进程的共用槽里，直到下次放开。

## Verification

- `crates/host-api/src/tests.rs`：`keyboard_reranker_follows_the_keyboard_model_switch` 断言会话刚建立时 `keyboard_reranker_loading` 有值、runtime 上还没挂；挂好、走过 `apply_pending` 后挂上。它和 `a_rerank_model_pack_installed_after_the_session_opened_is_attached_on_focus` 都改用 `settle_keyboard_reranker` 等后台挂完。
- `cargo test -p msime-host-api --lib` 带和不带 `onnx-rerank` 都全部通过（1123 / 1118）。
