# Agent Note: Windows 中英切换提示不等轮询，换显示器时不在左上角闪

Status: implemented

## Problem

#10782：Windows 上单按 Shift 切中英文后，光标旁的「中」「英」提示要慢一拍才出现；在两台缩放不同的显示器之间切换时，提示会先在目标显示器工作区左上角闪一下，再跳到光标旁。

**慢一拍。** Server 的界面线程（`server_main` 的 `wmain` 消息循环）每轮末尾 `MsgWaitForMultipleObjectsEx(0, nullptr, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE)`，不等任何事件，无事可做时最多睡 50 毫秒（按默认 15.6 毫秒的计时精度实际是 50–62.5 毫秒）。管道和输入队列都在别的线程上，没有东西叫醒它。主线程上的低级键盘钩子会在物理按键时顺带叫醒它（见 [渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md)），但那一下发生在 TIP 处理 Shift 之前，模式报告还没到；报告到达时界面线程已经又睡下了。

单按 Shift 的提示不经 `InputStateHudFeedback::publish`：TIP 处理完 Shift 后只发一帧 `StatusSnapshot`（`SendIMESwitchEventToUIProcessViaNamedPipe` 在 `CompositionProcessorEngine.cpp` 里是注释掉的），`ModeMailbox` 记下新模式，界面线程每轮用 `server.mode_view()` 读出来交给 `mode_authority_step`，`decision.user_changed` 为真时由 `input_language_hud` 生成提示。所以只在 `publish` 里叫醒，只能覆盖全半角、标点、繁简、方案这类经信箱发出的提示，覆盖不到这个缺陷的主路径。

还有一层：事件处理器（`server_main` 交给 `WindowsServer` 的事件回调，以及 `SessionController` 里写 `ModeMailbox` 的那一步）在输入队列上、焦点闸门之内运行。界面线程读模式用不等待的 `FocusGate::try_with_active`，在闸门里叫醒它，它可能正好撞上闸门、读到空，再睡满一轮。[渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md) 当时就因为同一个原因否掉了「候选信箱每次发布都叫醒」。

**左上角闪一下。** `InputModeHudWindow::show` 要按目标显示器的 DPI 算尺寸，而 `window_dpi` 只认窗口当前所在的显示器，所以换显示器时先把窗口挪到目标显示器工作区左上角量一次，再挪到光标旁。上一枚提示还显示着（0.6 秒内又切了一次）或正在淡出时，这一挪是显示着走的，于是在左上角露一下；两边缩放不同时这一挪还触发 `WM_DPICHANGED`，在显示着的时候丢掉合成表面并收起。

## Decision

**叫醒界面线程，并且在焦点闸门放开之后叫醒。**

- `InputStateHudFeedback` 增加 `set_wake` 和 `wake()`。`server_main` 在构造信箱之后登记 `PostThreadMessageW(界面线程, WM_NULL, 0, 0)`：线程消息队列里多一条消息，`MsgWaitForMultipleObjectsEx` 立即返回，`PeekMessageW` 取走它，`DispatchMessageW` 对没有窗口的 `WM_NULL` 什么也不做，界面线程随即跑完一轮。`publish` 存下提示后在锁外调用 `wake()`，覆盖工作线程（繁简、方案切换）发布的提示。
- `SessionPump::Presentation` 增加 `event_settled`：管道线程上，一个带焦点路由的非按键事件被事件处理器接住、`enqueue` 返回之后调用，这时输入队列任务已经结束，焦点闸门已经放开。`SessionController` 把它原样转给构造时收到的 `Presentation`。`server_main` 用它在 `StatusSnapshot`、`FocusRestored`、`IMESwitch`、`PuncSwitch`、`DoubleSingleByteSwitch` 之后调用 `hud_feedback.wake()`。全半角和标点提示仍在闸门里 `publish`，那一次叫醒可能读空，`event_settled` 这一次保证界面线程在闸门放开后再跑一轮。
- 消息循环本身、50 毫秒的上限、提示的判定和开关都不变。

**换显示器量 DPI 之前先收起。** `InputModeHudWindow::show` 发现窗口不在目标显示器上时，先 `hide()`（杀掉两个计时器、不透明度复位成 1、`SW_HIDE`），再挪到工作区左上角量 DPI，之后照旧一次 `SetWindowPos(... SWP_SHOWWINDOW)` 摆到光标旁。这一刻还没画出这一枚提示，收起不丢用户该看到的东西；`WM_DPICHANGED` 收到的是已经隐藏的窗口，它照旧 `DiscardTarget()`，下一次 `WM_PAINT` 重建合成表面。最后那次摆放落在同一块显示器的工作区里（`input_mode_hud_placement` 按这块工作区夹紧），不会再换 DPI。已经在目标显示器上时不挪也不收起，连续切换的行为不变。

## Historical audit

- [渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md)：部分重叠。它记录了闸门内叫醒会读空、因此不在候选信箱发布时叫醒；本篇遵守同一条约束，叫醒放在 `event_settled`，那一篇的决定不变。
- [工具栏、提示、托盘对齐 macOS 几何](../feature/2026-10-20-windows-toolbar-hud-tray-mac-geometry.md)：部分重叠，它持有提示的尺寸、淡出和 DPI 换算；在其「中英切换提示」一段补上换显示器先收起和叫醒的事实并互链。
- [中英切换提示定位](2026-10-11-windows-digit-commit-and-hud-anchor.md)、[Windows 输入模式对齐](../feature/2026-10-10-windows-input-mode-parity.md)：相关，锚点和提示文案不变，不改。
- 已关闭的 #10783（作者 luojiyin1987）先提出了 `set_wake` 加 `PostThreadMessageW(WM_NULL)` 和量 DPI 前先 `hide()` 这两处修改，它的笔记没有合入；本篇接替它，补上不经信箱的 Shift 路径和闸门外的叫醒。

## Alternatives considered

- **只在 `InputStateHudFeedback::publish` 里叫醒（#10783 的做法）**：改动最小。但单按 Shift 的中英提示不经 `publish`，缺陷的主路径照旧要等满一轮；而且 `publish` 多半在焦点闸门里被调用，叫醒后界面线程可能读空。
- **在 Server 的事件回调里直接叫醒**：不用动 `SessionPump`。事件回调在焦点闸门之内运行，界面线程被叫醒后 `try_with_active` 可能失败、读到空的 `mode_view`，又回到 50 毫秒的等待；Windows 会给被消息叫醒的线程临时提升优先级，它可能抢在工作线程放开闸门之前跑到读模式那一步。
- **把界面线程的等待改成等一个事件，工作线程 `SetEvent`**：不往消息队列里塞空消息。但 `MsgWaitForMultipleObjectsEx` 现在一个句柄都不等，加句柄要连带改停止和退出路径；线程消息对没有窗口的 `WM_NULL` 是空操作，效果相同而改动更小。
- **用 `GetDpiForMonitor`（`monitor_effective_dpi`）直接取目标显示器的 DPI，完全不挪窗口**：少一次 `SetWindowPos`。但最后那次显示着的 `SetWindowPos` 就会跨显示器，`WM_DPICHANGED` 在同一次调用里到达并执行 `hide()`，要么提示被收起，要么还得改 `WM_DPICHANGED` 分支区分「自己挪出来的」；这些都依赖 DirectComposition 和真实显示器，没有 Windows 环境无法验证，收益只是少一次看不见的挪动。
- **`WM_DPICHANGED` 时只 `DiscardTarget()`、不 `hide()`**：少一次收起。但量 DPI 的那一挪如果显示着，窗口会停在左上角直到下一次 `SetWindowPos` 才跳走，把「闪一下」变成「先错位再跳」。

## Consequences

- 单按 Shift 后，模式报告一离开焦点闸门界面线程就跑一轮，提示不再多等最多一个 50 毫秒的轮询；全半角、标点、繁简、方案提示同样不等。
- 每个模式报告多一次 `PostThreadMessageW`，界面线程多跑一轮；模式报告只在切换和焦点变化时发，不在每个按键上发。没有登记叫醒时行为与以前相同。
- 叫醒之后如果紧接着的下一个输入队列任务又占住闸门，那一轮仍会读空、退回原来的等待，不会比以前更慢。专用英文模式经按键回复的 `delivered` 在闸门里 `publish`，只得到闸门内的那次叫醒，没有改。
- 换显示器时提示不再出现在工作区左上角，`WM_DPICHANGED` 也不再打断正在显示的这一枚；代价是换显示器那一次要重建合成表面，与以前相同。
- 「淡出偶尔像卡住」没有处理也没有测量：界面线程这一轮本身很重（候选、工具栏、主题检查都在里面），淡出帧还和其他界面抢这条线程，要在真机上量过再决定。

## Verification

- `tests/ui/input_mode_hud_layout.cpp`（CMake 目标 `windows-input-mode-hud-layout`，可移植 runner 也会跑）：发布叫醒一次、取走不叫醒、单独 `wake()` 叫醒、回调在锁外调用（回调里再取信箱不自锁）、没有登记时照常收发。没有 `set_wake` 的实现上这几条编译不过。
- `tests/runtime/session_pump.cpp` 的模式事件用例（随 `windows-session-smoke` 等目标在 Windows 原生作业里运行）：`StatusSnapshot`、`IMESwitch`、`FocusRestored` 各三次都调用 `event_settled`，且回调里 `try_with_active` 能拿到闸门，证明它在闸门之外。
- `server_main.cpp`、`InputModeHudWindow.cpp`、`SessionPump.cpp`、`SessionController.cpp` 只经 MinGW 语法检查和 PR 上的 Windows 构建；Windows 实机上连续单按 Shift、在两台缩放不同的显示器之间切换的验收还没有做。
