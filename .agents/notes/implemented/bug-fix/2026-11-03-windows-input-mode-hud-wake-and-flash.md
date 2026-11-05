# Agent Note: Windows 中英切换提示不必等轮询，换显示器时也不再闪一下

Status: implemented

## Problem

单按 Shift 切中英文之后，Windows 的中英切换提示要慢一拍才出现，换显示器时还会闪（#10782）。两处原因：

- **提示等轮询。** 界面线程（`server_main` 的消息循环）在无事可做时会 `MsgWaitForMultipleObjectsEx(0, nullptr, 50, QS_ALLINPUT, ...)`，管道和输入队列都跑在别的线程上，没有事件会叫醒它。提示有两条来路，都要等界面线程下一轮：
  - 单按 Shift：TIP 收到键盘开关的变化后经 `WM_UpdateIMEStatus` 发一条 `StatusSnapshot`。`SessionPump` 在输入队列上同步模式、写进 `ModeMailbox`，界面线程每轮读 `mode_view`，由 `mode_authority_step` 认出 `user_changed` 才拼出「中」/「英」提示。这条路径**不经过** `InputStateHudFeedback::publish`。
  - 切简繁、切日语、清缓存、全半角和中英标点：由工作线程或管道的事件回调 `publish` 进 `InputStateHudFeedback`，界面线程每轮 `take`。
  所以刚按下 Shift 那一下最多要等满 50 毫秒（按默认 15.6ms 计时精度，实际 50–62.5ms），提示才出现在光标旁。
- **换显示器时闪。** `show` 要先知道目标显示器的 DPI 才能算尺寸，而 `window_dpi` 只认窗口自己所在的显示器，于是它先把窗口挪到目标显示器的工作区左上角量一次，再挪到光标旁。这一步是显示着走的：0.6 秒内还显示着（或正在淡出）的上一枚提示会先出现在左上角，再跳到光标旁；两边缩放不同时这一挪还会触发 `WM_DPICHANGED`，把刚摆好的窗口和合成表面一起丢掉重建。

## Decision

**模式事件落地、焦点锁放开之后叫醒界面线程。** `SessionPump::Presentation` 增加 `mode_event`：当前焦点的 `IMESwitch`、`PuncSwitch`、`DoubleSingleByteSwitch`、`StatusSnapshot`、`FocusRestored` 在输入队列上同步完、事件回调返回、`focus_.with_active` 放开焦点锁之后，在管道线程上调用一次。`SessionController` 把它转发给 `server_main` 传进来的同名回调。

位置必须在锁外：事件回调（`EventHandler`）跑在焦点锁里，而界面线程读模式的 `ModeMailbox::snapshot` 用的是 `FocusGate::try_with_active`，锁忙就返回空。在回调里叫醒，界面线程醒得快时正好撞上锁、`mode_view` 读空，这一轮既看不到新模式也不取提示（`take` 要求 `view` 有值或没有活动会话），又要等满一轮。[渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md) 当时放弃「信箱发布即叫醒」也是同一个理由。

**提示信箱发布即叫醒。** `InputStateHudFeedback` 增加 `set_wake`，`publish` 存下提示后在锁外调用它，覆盖工作线程发布的那几种提示。管道事件回调里发布的那几种（全半角、中英标点）在锁内，叫醒可能扑空，随后的 `mode_event` 会在锁外再叫一次。

两个回调在 `server_main` 里接到同一个 `PostThreadMessageW(界面线程, WM_NULL, 0, 0)`：消息队列被投进一条空消息，等待就返回，`PeekMessage` 取走它、`DispatchMessage` 什么也不做，界面线程随即跑完这一轮。线程号在 `wmain`（也就是消息循环所在的线程）里取。

**量 DPI 之前先收起。** `InputModeHudWindow::show` 在发现目标显示器不同的时候先 `hide()`，再挪到工作区左上角，之后照旧一次 `SetWindowPos` 摆到光标旁。窗口在量 DPI 那一刻还没画出这一枚提示（`hide()` 同时杀掉两个计时器、把不透明度复位成 1），所以收起不丢任何用户该看到的东西；`WM_DPICHANGED` 分支收到消息时窗口已经隐藏，`DiscardTarget()` 也不再打断一次正在显示的动画。

## Historical audit

- [渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md)：相关，同样是去掉轮询带来的等待；它记录了「焦点锁内叫醒会让 try_lock 读空」，本篇的 `mode_event` 因此放在锁外。不改它的决定。
- [工具栏、提示与托盘的 macOS 几何](../feature/2026-10-20-windows-toolbar-hud-tray-mac-geometry.md)：部分重叠，保留提示的尺寸、淡出和定位规则，补充换显示器先收起与叫醒的事实并互链。

## Alternatives considered

- **只在 `InputStateHudFeedback::publish` 里叫醒** — #10783 最初的做法。单按 Shift 的提示不经过这个信箱，叫醒根本不会发生，报告里的主场景不受影响。
- **在 `server_main` 的事件回调里收到 `StatusSnapshot` 时叫醒** — 改动最少，不用动 `SessionPump`。但回调在焦点锁里，理由见上。
- **`ModeMailbox` 自己带一个叫醒回调** — 和提示信箱对称。但它的 `event` 同样在焦点锁里调用，而且要经 `WindowsServer`、`SessionController` 两层转发一个新的设置入口；`SessionPump::Presentation` 已经是管道线程向外通知的地方。
- **把界面线程的等待改成等一个事件，工作线程 `SetEvent`** — 少一条消息。但 `MsgWaitForMultipleObjectsEx` 现在一个句柄都不等，加句柄要连带改停止信号和退出路径；投一条线程消息不需要改等待本身。
- **把提示的淡出和定位挪到独立线程** — 从根上不受界面线程那一轮有多重的影响。但 `InputModeHudWindow` 和候选窗、工具栏共用一条 UI 线程上的 Direct2D 设备与调色板，独立线程要重新划一遍 DPI、主题和生命周期的边界，超出这次修缺陷的范围。
- **用 `GetDpiForMonitor` 直接问显示器的 DPI，完全不挪窗口** — 最干净，一次 `SetWindowPos` 就到位。但它在 shcore.dll 里，`compat::DpiApi` 只按名字从 user32 取函数，要再加一层 shcore 的动态加载和 Win7 回退；收益只是少一次挪窗口。
- **量 DPI 的 `WM_DPICHANGED` 期间不 `hide()`，只 `DiscardTarget()`** — 少一次隐藏。但窗口正显示着，会停在左上角直到下面那次 `SetWindowPos` 才跳走，反而把「闪一下」变成「先错位再跳」。

## Consequences

- 单按 Shift 之后，提示在模式事件落地后的这一轮出现，不再等满消息循环的 50 毫秒；工作线程发布的提示同样如此。换显示器时不再出现在左上角，也不会因为 `WM_DPICHANGED` 丢掉合成表面。
- 每条模式事件和每次发布多一次 `PostThreadMessage`，界面线程多跑一轮。`StatusSnapshot` 在焦点变化和重连时也会发，所以换焦点时界面线程也会提前跑一轮，作用域为全局时推回模式也随之提前；这些事件的频率远低于按键。
- 界面线程正处在别的模态循环里（菜单、拖动）时线程消息会被丢掉，丢掉的只是一次叫醒，下一轮照旧按 50 毫秒轮询读到。
- 换显示器后再显示时，合成表面里如果还是上一枚的内容（同一 DPI、没有 `DiscardTarget`），`SWP_SHOWWINDOW` 和随后的 `WM_PAINT` 之间可能露出旧字，这在同一块显示器上连续切换时本来就存在，本篇没有改。
- 界面线程那一轮本身有多重（候选、工具栏、主题检查都在里面）、淡出帧和候选窗退场动画抢同一条线程、以及 `announce_input_mode` 在开读屏时的跨进程调用，都仍然可能让淡出掉帧，需要在真机上量过一次再决定改不改。

## Verification

- `platforms/windows/tests/runtime/session_pump.cpp` 的模式事件用例（`StatusSnapshot`、`IMESwitch`、`FocusRestored` 各跑一遍）给 `mode_event` 计数，并在回调里要求不在输入队列线程上、`gate.try_with_active(lease, ...)` 能拿到锁，即焦点锁已经放开。只加 `Presentation` 字段、`SessionPump` 不调用时，`windows-session-smoke` 在 Wine 下停在这条断言（`Session pump test failed at line 1215`）；加上调用后越过 `session_pump_tests`，停在后面一项与本改动无关、在 develop 上同样失败的检查（`scripts/known-failures.txt` 已列 `wine windows-session-smoke`）。
- `platforms/windows/tests/ui/input_mode_hud_layout.cpp` 覆盖 `InputStateHudFeedback` 的叫醒：每次发布叫醒一次，取走不叫醒，没有设置时不崩。
- `server_main` 里线程消息的接线和 `InputModeHudWindow` 的挪窗口没有自动化测试，它们依赖真实的消息循环、DirectComposition 和真实显示器，只经过 MinGW 交叉编译。验收方式是在 Windows 实机上单按 Shift 看提示是否立刻出现，并在两台缩放不同的显示器之间连续单按 Shift，看提示只在光标旁出现、不在左上角闪。
