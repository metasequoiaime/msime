# Agent Note: Linux IBus 宿主的 glib 回调不得对 null 视图调用 value()

Status: implemented

## Problem

#6675：Linux 0.11.0 的 IBus 宿主在 Ubuntu 上两天崩溃 65 次，全部是 `std::terminate: nlohmann::json_abi_v3_11_2::detail::type_error 306`（`cannot use value() with null`）。

`show_input_mode_hint()` 在焦点进入和中英切换时显示「中」/「英」，并注册 1.2 秒后收起提示的 `g_timeout_add_full` 回调。回调要判断用户是否已经开始组字，直接对 `State::view` 调了 `value("editing_text", ...)` 和 `value("candidates", ...)`。`State::view` 在会话打开之前是 JSON null，`State::close()` 也会把它置回 null，而 `close()` 之后的 `open()` 在被拦截的输入框（密码、PIN、数字）、英文模式或词库维护期间会直接返回、不建会话。于是「焦点进入普通输入框，1.2 秒内同一上下文的 `content_type` 改成密码框」这样的顺序就会让回调读到 null 视图。nlohmann 的 `value(key, default)` 只在对象缺键时回退默认值，对 null 直接抛异常；按键、焦点这些入口包在 `guarded()` 里，异常会被吞掉并重置会话，而 glib 定时器回调外面没有任何东西，异常一路冒到 `entrypoints/ibus_main.cpp` 的 `std::set_terminate`，进程 abort。

`scripts/test-linux-rendered-view-guard.py` 已经为同样生命周期的 `rendered_view` 建了静态检查，但它只看 `rendered_view`，`view` 没有任何检查。

## Decision

- 组字判断收进 `platforms/linux/src/core/ViewComposition.h` 的 `view_is_composing()`：不是对象、键缺失或类型不对一律回答「没有组字」，用 `find()` 读取，不会抛异常。输入模式提示的回调改用它。null 视图意味着会话已经不在，没有候选页码需要保护，收起提示是对的行为。
- 候选右键提示（1.5 秒）的回调同样没有 `guarded()`，在读 `s.view.value("generation", ...)` 之前补上 `s.view.is_object()`。按现有不变量（`session` 非零时 `view` 一定是对象，回调已先检查同一会话）它目前到不了 null，但这个保证来自条件求值顺序和别处的不变量，补一个判断比依赖它们稳。
- 新增 `scripts/test-linux-glib-callback-view-guard.py`：对每个以 lambda 注册的 `g_timeout_add*` / `g_idle_add*` 回调体，要求其中对 `s.view` / `state(...).view` 的 `value(` / `at(` 读取之前出现过 `s.view.is_object()`，或者位于回调体内的 `guarded(...)` 调用之中。它在修复前的源码上报出三处（输入模式提示两处、候选右键提示一处），修复后通过。

### 同类回调的排查结果

以下回调读过或可能读到视图，逐个确认不会对 null 视图抛异常，没有改动：

- 候选隐藏（24 ms，`apply_candidate_hide`）、候选操作属性（400 ms）：只读 `rendered_view` / `rendered_candidates`，且已带 `is_object()` / `is_array()` 判断；两者都在 `close()` 里取消。
- 语音失败提示（1.2 秒）：只动波形浮层，不读视图。
- 语音进度、电平、转写、结果的 idle 回调：先过 `voice_result_current()`（要求 `voice_active`），`render()` 在 `voice_active` 时提前返回、不碰视图；`render_after_voice()` 只在 `session` 非零时读视图。
- 云候选、AI、翻译延时（500/650/500 ms）和稳定重排定时器：由 `invalidate_providers()` 在 `close()` 里取消，而 `view` 置 null 只发生在 `close()`；重排回调读取前先查 `session` 且包在 `try` 里。
- 升级重启 idle 回调：已经用 `is_object()` + `find()`。
- `reload_preferences`：涉及视图的部分都在 `guarded()` 里。

Fcitx5 插件的 `view_` 默认是 `Json::object()`、重置时也回到空对象，定时器是随状态销毁的 `EventSourceTime` 成员，没有这个问题。

## Alternatives considered

- **把整个回调体包进 `guarded()`**：issue 提出的方案之一，最强的理由是一次兜住回调里所有可能的异常。但 `guarded()` 的兜底是 `close()` + `clear()` + `publish_mode()`：为了一次「收起提示」的判断失败去关掉会话、清掉组字，比跳过这次判断代价大得多；而且它把「视图可能是 null」这个可预见的状态当成异常处理。
- **让 `view` 永远是对象（像 Fcitx5 那样默认 `Json::object()`，`close()` 置空对象）**：从根上消掉这类异常。但 IBus 宿主里已有代码用 `view.is_object()` 区分「有没有 Engine 视图」，例如 `voice_commit_context()` 在英文模式下靠它改取宿主自己的方案；改不变量要逐处重审几十处读取的语义，风险远大于这次修复本身。
- **只在回调里写 `s.view.is_object() && (...)`**：与 issue 的建议一致，也能修好。没有直接这么写，是因为同样的判断需要一个不依赖 IBus 的单测来钉住，抽成头文件里的小函数才能在 `platforms/linux/tests` 下直接测。

## Consequences

收益：输入模式提示和候选右键提示的定时器不会再因为会话在等待期间被关掉而让宿主 abort；新增的静态检查会拦住以后在 glib lambda 回调里新增的未经判断的视图读取。

代价与缺口：静态检查只看以 lambda 注册的回调体本身，不跟进它调用的函数，也不覆盖以命名函数注册的回调；这两类仍靠评审和上面的排查结论。时序本身（焦点进入后 1.2 秒内会话被关掉）需要真实的 IBus 会话才能端到端复现，本地门禁没有这一层，单测只覆盖判断函数在 null 视图上的行为。

## Verification

- `platforms/linux/tests/core/view_composition.cpp`（ctest `linux-view-composition`）：先断言 null 视图上的 `value("editing_text", ...)` 抛 `type_error` 306（即崩溃前提），再断言 `view_is_composing()` 在 null、非对象、空视图、字段类型不对时都返回 false 且不抛，在编辑串或候选非空时返回 true。
- `scripts/test-linux-glib-callback-view-guard.py` 在修复前的 `ClientEngine.cpp` 上报出三处并以非零退出，修复后通过。
- `bash platforms/linux/build-container.sh` 在容器里编译 IBus engine、Fcitx5 插件与全部单测并跑 ctest。
