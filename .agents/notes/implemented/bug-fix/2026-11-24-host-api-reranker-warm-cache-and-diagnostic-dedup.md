# Agent Note: 逐键重排器热缓存复用、非输入会话不释放共享模型与模型诊断去重

Status: implemented

## Problem

Fcitx5 失焦时关闭 `HostSession`、聚焦时重建（`platforms/linux/fcitx5/tests/native.cpp` 锁定的生命周期），所以窗口间每切一次焦点就是一次 `create_session`。共享 `host-api` 在这条路径上有三处重复开销，影响所有平台：

- 预览会话（`SessionUse::Preview`）和不挂逐键重排的镜像会话（`!keyboard_reranker_allowed`）在建会话或更新偏好时也走 `keyboard_reranker(false, ...)`，即 `RerankerMount::Off`，会 `onnx_rerankers().release(root)`，把别的输入会话正在共用的进程级 ONNX 模型放掉，下一个输入会话只能重新读盘、重新自测。
- 即使模型已在进程缓存里（`OnnxRerankers` 已加载或已量出超限，随包模型已在 `SENTENCE_MODELS` 与 `MEASURED` 中），`load_keyboard_reranker` 每次仍起一条 `msime-keyboard-reranker` 线程去取。
- `report_keyboard_reranker` 与 `report_settled_model`（见[落定重排模型接线状态写进诊断日志](2026-10-31-settled-model-diagnostics.md)）每建一次会话就写一行完全相同的诊断。

这部分最初随 #11867（Fcitx5 空闲轮询修复）一起提交，因为它改变所有平台的行为，按审查意见拆成单独的 PR。

## Decision

- **非输入会话不释放共享模型**：`crates/host-api/src/ffi/session.rs` 的 `create_session` 与 `crates/host-api/src/lib.rs` 的偏好重建里，预览会话和镜像会话只 `runtime.set_candidate_scorer(None)` 清掉自己的打分器；只有允许挂模型的输入会话关掉开关时才调 `keyboard_reranker(false, ...)` 释放进程缓存。
- **热缓存命中不起线程**：`crates/host-api/src/ffi.rs` 新增 `cached_keyboard_reranker_for`、`OnnxRerankers::attach_cached` 与 `cached_bundled_keyboard_reranker`，只看已加载的重排器、已记下的超限结论和已量过的随包模型；能直接回答时 `load_keyboard_reranker` 返回预先填好的 `KeyboardRerankerSlot`，仍由 `apply_pending` 在输入空闲时换上（[逐键重排器在后台挂，不堵输入线程](2026-11-03-async-keyboard-reranker-attach.md)的契约不变）；还需要读盘或自测时照旧起后台线程。
- **模型诊断进程级去重**：逐键重排器的 `(category, detail)`（比较时去掉末尾的 `" (shared)"`）与上一行相同时不再写，`RerankerMount::Off` 时清掉记录；落定重排模型按 `(category, detail, 模型文件的大小与修改时间)` 去重，`neural_desktop` 关闭时清掉记录。两份记录都按会话的资源目录（HostOptions 的 `resources`）分开记，开关关闭时也只清这个资源目录的那一条，同一进程里资源目录不同的会话互不冲掉对方的记录。去重是进程级的：Windows server 和 macOS 输入法这类一个进程服务所有输入框的宿主上，第一个会话之后状态没变的会话不再各写一行。把文件的大小和修改时间放进键里，是因为资源包重装到同一个发布目录时路径不变，只按路径比会把换过的模型也藏掉。

## Alternatives considered

- **热缓存命中时在 `create_session` 里直接把重排器挂进 `runtime`**
  - 最强理由：模型已经在内存里，省掉一层 `KeyboardRerankerSlot` 和之后 `apply_pending` 的换入。
  - 否决原因：`crates/host-api/src/tests.rs` 断言建会话返回后 `keyboard_reranker_loading.is_some() && !runtime.has_reranker()`，换入统一发生在 `apply_pending`；返回预先填好的槽同样不起线程、不读盘，又不改这条契约。
- **落定重排模型的诊断只按路径去重，在文档里说明同路径换包不再上报**
  - 最强理由：实现最简单，不多一次 `stat`。
  - 否决原因：#10341 加这些诊断就是为了分辨「装了但没用上」之类的状态，同路径重装资源包正是用户排障时会做的事；多一次 `stat` 只发生在会话建立和开关切换时，不在按键路径上。

## Consequences

- 窗口间切换焦点时，只要模型已在进程缓存里，就不再起后台加载线程、不再重读模型文件，也不再写重复的 `settled model` / `keyboard reranker` 诊断行。
- 预览会话、镜像会话不再让共享的 ONNX 模型被提前放掉。
- 诊断行少了：之前每个会话都有一行，现在状态变化时才有一行。排障时看到的是最近一次状态变化，不是每个会话各自的状态。
- 测试 `crates/host-api/src/tests/model_reload_dedup.rs` 覆盖热缓存预填槽且不起线程、镜像和预览会话不释放缓存、诊断去重、开关重开和同路径换模型后重新报告。`SENTENCE_MODELS` 是容量 8 的进程级 LRU，整套 `cargo test -p msime-host-api` 并行跑时别的测试会把这里的随包模型挤出去，下一个会话只好重新起线程；随包模型的两段断言因此在起了线程时重新热身重跑（最多 20 轮），真回归每一轮都会起线程，照样失败。为了断言「没有起线程」「没有走 `RerankerMount::Off`」，`load_keyboard_reranker` 和 `keyboard_reranker_for` 里留了 `#[cfg(test)]` 的线程局部计数器：这两件事没有别的可观测结果。
