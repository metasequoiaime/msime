# Agent Note: Windows 中英切换提示不必等轮询，换显示器时也不再闪一下

Status: implemented

## Problem

单按 Shift 切中英文之后，Windows 的中英切换提示要慢一拍才出现，换显示器时还会闪。两处原因都在 `InputModeHudWindow` 这一侧：

- **提示等轮询。** 提示由工作线程发布进 `InputStateHudFeedback`，界面线程在消息循环里每轮取一次，而这一轮在无事可做时会 `MsgWaitForMultipleObjectsEx(0, nullptr, 50, QS_ALLINPUT, ...)`。管道和输入队列都跑在别的线程上，没有事件会叫醒界面线程，所以刚按下 Shift 那一下最多要等满 50 毫秒，提示才出现在光标旁。
- **换显示器时闪。** `show` 要先知道目标显示器的 DPI 才能算尺寸，而 `window_dpi` 只认窗口自己所在的显示器，于是它先把窗口挪到目标显示器的工作区左上角量一次，再挪到光标旁。这一步是显示着走的：0.6 秒内还显示着（或正在淡出）的上一枚提示会先出现在左上角，再跳到光标旁；两边缩放不同时这一挪还会触发 `WM_DPICHANGED`，把刚摆好的窗口和合成表面一起丢掉重建。

## Decision

**发布即叫醒界面线程。** `InputStateHudFeedback` 增加 `set_wake`，`publish` 存下提示后调用它（在锁外）。`server_main` 在构造信箱之后注册一个 `PostThreadMessageW(界面线程, WM_NULL, 0, 0)`：消息队列被投进一条空消息就从等待里返回，`PeekMessage` 取走它、`DispatchMessage` 什么也不做，界面线程随即跑完这一轮并看见新提示。

**量 DPI 之前先收起。** `InputModeHudWindow::show` 在发现目标显示器不同的时候先 `hide()`，再挪到工作区左上角，之后照旧一次 `SetWindowPos` 摆到光标旁。窗口在量 DPI 那一刻还没画出这一枚提示（`hide()` 同时杀掉两个计时器、把不透明度复位成 1），所以收起不丢任何用户该看到的东西；`WM_DPICHANGED` 分支收到消息时窗口已经隐藏，`DiscardTarget()` 也不再打断一次正在显示的动画。

## Alternatives considered

- **把界面线程的等待改成等一个事件，工作线程 `SetEvent`** — 少一条消息，也不用往消息队列里塞空消息。但 `MsgWaitForMultipleObjectsEx` 现在一个句柄都不等，加句柄要连带改停止信号和退出路径；`PreferenceMonitor` 已经是「条件变量加待处理标志」的写法，这里照消息队列的既有用法更小。
- **把提示的淡出和定位挪到独立线程** — 从根上不受界面线程那一轮有多重的影响。但 `InputModeHudWindow` 和候选窗、工具栏共用一条 UI 线程上的 Direct2D 设备与调色板，独立线程要重新划一遍 DPI、主题和生命周期的边界，超出这次修缺陷的范围。
- **用 `GetDpiForMonitor` 直接问显示器的 DPI，完全不挪窗口** — 最干净，一次 `SetWindowPos` 就到位。但它在 shcore.dll 里，`compat::DpiApi` 只按名字从 user32 取函数，要再加一层 shcore 的动态加载和 Win7 回退；为一个已经能用的路径多引一个系统库，收益只是少一次挪窗口。
- **量 DPI 的 `WM_DPICHANGED` 期间不 `hide()`，只 `DiscardTarget()`** — 少一次隐藏。但窗口正显示着，会停在左上角直到下面那次 `SetWindowPos` 才跳走，反而把「闪一下」变成「先错位再跳」。

## Consequences

- 单按 Shift 之后提示不再等满消息循环的那一拍；换显示器时不再出现在左上角，也不会因为 `WM_DPICHANGED` 丢掉合成表面。
- 每次发布多一次 `PostThreadMessage`，界面线程多处理一条空消息；没有设置叫醒时行为与以前完全一样。
- 这条只覆盖「等轮询」和「量 DPI 的挪动」两处。界面线程那一轮本身有多重（候选、工具栏、主题检查都在里面）、淡出帧和候选窗退场动画抢同一条线程、以及 `announce_input_mode` 在开读屏时的跨进程调用，都仍然可能让淡出掉帧，需要在真机上量过一次再决定改不改。
- `InputStateHudFeedback` 的叫醒有主机侧回归（`platforms/windows/tests/ui/input_mode_hud_layout.cpp`）；`InputModeHudWindow` 自己的挪窗口没有自动化测试，它依赖 DirectComposition 和真实显示器。验收方式是在两台缩放不同的显示器之间连续单按 Shift，看提示只在光标旁出现、不在左上角闪。
