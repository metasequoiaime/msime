# Agent Note: Linux IBus 宿主的 glib 回调不得对 null 视图调用 value()

Status: implemented

## Problem

#6675：Linux 0.11.0 的 IBus 宿主在 Ubuntu 上两天崩溃 65 次，全部是 `std::terminate: nlohmann::json_abi_v3_11_2::detail::type_error 306`（`cannot use value() with null`）。

#7694 再次报告 Linux 0.11.0 的同类异常，最新样本使用 nlohmann 3.11.3。`306` 能确定是对非对象调用 `value()`，GLib 主循环中的栈也符合未捕获的回调异常；但报告没有可匹配的 ELF Build ID，检查过的官方 0.11.0 `.deb`、`.rpm` 与报告偏移不匹配，不能只凭异常码断言它一定来自输入模式提示。已有辅助函数单测和静态检查缺少真实宿主回调验证，也没有覆盖对象销毁后的超时。

`show_input_mode_hint()` 在焦点进入和中英切换时显示「中」/「英」，并注册 1.2 秒后收起提示的 `g_timeout_add_full` 回调。回调要判断用户是否已经开始组字，直接对 `State::view` 调了 `value("editing_text", ...)` 和 `value("candidates", ...)`。`State::view` 在会话打开之前是 JSON null，`State::close()` 也会把它置回 null，而 `close()` 之后的 `open()` 在被拦截的输入框（密码、PIN、数字）、英文模式或词库维护期间会直接返回、不建会话；因此焦点与输入框属性交错时可能留下 null 视图。nlohmann 的 `value(key, default)` 只在对象缺键时回退默认值，对 null 直接抛异常；glib 定时器回调外面没有异常捕获，异常会一路冒到 `entrypoints/ibus_main.cpp` 的 `std::set_terminate`，进程 abort。线上报告仍缺少可匹配的 Build ID，下面的时序用例证明的是这个崩溃前提和修复行为，不把它当作线上栈的符号化结果。

`scripts/test-linux-rendered-view-guard.py` 已经为同样生命周期的 `rendered_view` 建了静态检查，但它只看 `rendered_view`，`view` 没有任何检查。

对 #7694 的后续审计发现，崩溃面不只在 glib 定时器：IBus 的 `process_key_and_count` 直接调用 `process_key`，而 `process_key` 在进入内部 `guarded()` 处理段之前还有两个快捷键判断。会话关闭或词库维护期间，`State::view` 可能仍为 JSON null；Ctrl+Shift+J 判断和候选维护快捷键的代次比较会在这个状态直接调用 `value()`，同样抛出 `type_error.306`，而按键入口没有外层异常捕获。代码审计确认了未捕获读取点和防守位置，但没有把密码框或普通英文模式描述成这两个按键的直接触发条件。

这次继续审计确认了同一生命周期的第三个入口：默认英文模式在 `focus_in()` 时不创建 Engine 会话，`State::view` 保持 null；禁用中文输入的 native Compose 分支却在 `NativeCompose::feed()` 完成时调用无来源覆盖的 `commit_text()`。`typing_source()` 随后对 null 视图调用 `value()`，真实 D-Bus `ProcessKeyEvent` 回归在宿主进程中复现 `type_error.306`。

## Decision

- 组字判断收进 `platforms/linux/src/core/ViewComposition.h` 的 `view_is_composing()`：不是对象、键缺失或类型不对一律回答「没有组字」，用 `find()` 读取，不会抛异常。输入模式提示的回调改用它。null 视图意味着会话已经不在，没有候选页码需要保护，收起提示是对的行为。
- 候选右键提示（1.5 秒）的回调同样没有 `guarded()`，在读 `s.view.value("generation", ...)` 之前补上 `s.view.is_object()`。按现有不变量（`session` 非零时 `view` 一定是对象，回调已先检查同一会话）它目前到不了 null，但这个保证来自条件求值顺序和别处的不变量，补一个判断比依赖它们稳。
- `process_key` 的 Ctrl+Shift+J 判断和候选维护快捷键代次比较也先检查 `s.view.is_object()`。null 视图时把快捷键交还给应用，或直接拒绝候选维护动作，不进入后续读取；这样即使按键与会话关闭交错，也不会让未捕获的 `value()` 异常终止宿主。
- Native Compose 在禁用中文输入时明确传入 `TypingSource::English`。Compose 是系统布局产生的英文直通文本，且这条路径允许没有会话；显式来源避免从 null 视图推导统计来源，同时保持英文模式与普通英文字符路径一致。
- 新增 `scripts/test-linux-glib-callback-view-guard.py`：对每个以 lambda 注册的 `g_timeout_add*` / `g_idle_add*` 回调体，要求其中对 `s.view` / `state(...).view` 的 `value(` / `at(` 读取之前出现过 `s.view.is_object()`，或者位于回调体内的 `guarded(...)` 调用之中。它在修复前的源码上报出三处（输入模式提示两处、候选右键提示一处），修复后通过。
- `platforms/linux/tests/runtime/mode_hint_lifecycle.cpp` 在私有 D-Bus 上实例化真实 IBus 宿主，走原生焦点与输入框属性入口，并实际运行 GLib 主循环。英文模式不创建 Engine 会话，因此不依赖词库；三个独立 ctest 用例分别覆盖 null 视图提示收起、提示等待中切到密码框执行 `close()`、提示等待中销毁上下文。提示收起后再获得焦点，确认宿主继续响应；销毁后不得再向辅助区域发信号。这些用例无条件进入 `BUILD_TESTING`，普通 Linux 容器门禁就会执行。

### 同类回调的排查结果

以下回调读过或可能读到视图，逐个确认不会对 null 视图抛异常，没有改动：

- 候选隐藏（24 ms，`apply_candidate_hide`）、候选操作属性（400 ms）：只读 `rendered_view` / `rendered_candidates`，且已带 `is_object()` / `is_array()` 判断；两者都在 `close()` 里取消。
- 语音失败提示（6 秒，见 [Linux 语音浮层对齐 macOS](../feature/2026-10-20-linux-voice-overlay-mac-parity.md)）：只动波形浮层，不读视图。另有浮层显示期间的 16 ms 波形帧和显示连接的 fd 监听，同样只动浮层，并在引擎销毁时随 `State` 析构移除。
- 语音进度、电平、转写、结果的 idle 回调：先过 `voice_result_current()`（要求 `voice_active`），`render()` 在 `voice_active` 时提前返回、不碰视图；`render_after_voice()` 只在 `session` 非零时读视图。
- 云候选、AI、翻译延时（500/650/500 ms）和稳定重排定时器：由 `invalidate_providers()` 在 `close()` 里取消，而 `view` 置 null 只发生在 `close()`；重排回调读取前先查 `session` 且包在 `try` 里。
- 升级重启 idle 回调：已经用 `is_object()` + `find()`。
- `reload_preferences`：涉及视图的部分都在 `guarded()` 里。
- `process_key` 的其余视图读取位于内部 `guarded()` 处理段；这次补的是进入该段之前的两个读取。

Fcitx5 插件的 `view_` 默认是 `Json::object()`、重置时也回到空对象，定时器是随状态销毁的 `EventSourceTime` 成员，没有这个问题。

## Alternatives considered

- **把整个回调体包进 `guarded()`**：issue 提出的方案之一，最强的理由是一次兜住回调里所有可能的异常。但 `guarded()` 的兜底是 `close()` + `clear()` + `publish_mode()`：为了一次「收起提示」的判断失败去关掉会话、清掉组字，比跳过这次判断代价大得多；而且它把「视图可能是 null」这个可预见的状态当成异常处理。
- **让 `view` 永远是对象（像 Fcitx5 那样默认 `Json::object()`，`close()` 置空对象）**：从根上消掉这类异常。但 IBus 宿主里已有代码用 `view.is_object()` 区分「有没有 Engine 视图」，例如 `voice_commit_context()` 在英文模式下靠它改取宿主自己的方案；改不变量要逐处重审几十处读取的语义，风险远大于这次修复本身。
- **只在回调里写 `s.view.is_object() && (...)`**：与 issue 的建议一致，也能修好。没有直接这么写，是因为同样的判断需要一个不依赖 IBus 的单测来钉住，抽成头文件里的小函数才能在 `platforms/linux/tests` 下直接测。
- **把时序用例加进需要词库的 `engine_smoke.cpp`**：能复用现有候选和 D-Bus 观察器，也能覆盖中文组字；但 `MSIME_ENGINE_RESOURCES` 未配置时普通编译门禁不注册它。英文模式本身就能稳定产生 null 视图，所以独立的无词库原生用例可以让每次门禁都覆盖超时路径，候选组字判断仍由原有辅助函数单测覆盖。

## Consequences

收益：输入模式提示和候选右键提示的定时器，以及两个按键快捷键入口，都不会再因为会话在等待期间被关掉而让宿主 abort；静态检查还会拦住以后在 glib lambda 回调或这两个入口中新增的未经判断的视图读取。

代价与缺口：静态检查只看以 lambda 注册的回调体本身，不跟进它调用的函数，也不覆盖以命名函数注册的回调；这两类仍靠评审和上面的排查结论。原生用例运行真实 IBus 对象、私有 D-Bus 和定时器，每项增加约 1.5 秒；它覆盖无会话与销毁时序，不依赖桌面 daemon、真实编辑器或中文词库。#7694 的具体栈归属仍需匹配崩溃设备的 Build ID 与符号，不能把这组回归测试等同于线上栈已符号化。

## Verification

- `platforms/linux/tests/core/view_composition.cpp`（ctest `linux-view-composition`）：先断言 null 视图上的 `value("editing_text", ...)` 抛 `type_error` 306（即崩溃前提），再断言 `view_is_composing()` 在 null、非对象、空视图、字段类型不对时都返回 false 且不抛，在编辑串或候选非空时返回 true。
- `scripts/test-linux-glib-callback-view-guard.py` 在修复前的 `ClientEngine.cpp` 上报出三处并以非零退出，修复后通过。
- `scripts/test-linux-glib-callback-view-guard.py` 在加入按键入口契约后，先在未修复源码上因两个缺少 `is_object()` 的判断失败；补上两个守卫后通过。
- `bash platforms/linux/build-container.sh` 在容器里编译 IBus engine、Fcitx5 插件与全部单测并跑 ctest。
- `ibus-engine-smoke --ctrl-space` 在未修复代码上先以默认英文模式创建无会话宿主，输入 `dead_circumflex` + `e`，稳定退出并输出 `type_error.306`；修复后同一真实 D-Bus 路径提交 `ê` 并通过。
- 无词库的 `ibus-native-compose-no-session` ctest 也完成红绿验证：恢复旧调用时失败并输出 `type_error.306`，显式来源修复后通过；因此普通 Linux 容器门禁会执行这条回归，而不依赖 `MSIME_ENGINE_RESOURCES`。
- 原生 ctest `ibus-mode-hint-english`、`ibus-mode-hint-blocked`、`ibus-mode-hint-destroyed` 通过私有 D-Bus 观察提示显示与隐藏，并等待比提示晚到的 GLib 哨兵确认超时获得执行机会。
- 反向验证只把输入模式提示回调的组字判断临时恢复为旧的两次 `s.view.value()` 读取：英文与密码框两项都在真实 GLib 超时执行时返回非零，输出 `[json.exception.type_error.306] cannot use value() with null`。恢复现有判断并重新构建后，三项全部通过。测试入口捕获异常以报告失败，生产入口没有这层捕获，因而同样的异常会进入 `std::terminate`。
- `bash platforms/linux/build-container.sh` 在本次改动上退出 0，86 项测试全部通过，包含三项无词库回调用例；同时完成五笔版构建与安装路径隔离检查。
- 继续核对官方 0.11.0 完整版 x86_64 便携包：归档 SHA-256 为 `18fffff4f75ddfadd0ac523cb0f2c5ae327b540c549674dda43ae034973b1365`，其中 IBus ELF SHA-256 为 `78da578f6727ee1570ed6dd6c2a3511bb91bf9b8fc2173389da77027a00524e8`，GNU Build ID 为 `d7cd00c86d8880de32523fb324a9f4036a39a7bc`。按完整 `.text` 指令边界反汇编，报告偏移 `0x12c0e`、`0x9ff9e`、`0x2ce13` 均落在指令内部，不能作为该 ELF 的正常回溯返回地址。因此便携包也不匹配，不用它的地址映射冒充线上符号化。
