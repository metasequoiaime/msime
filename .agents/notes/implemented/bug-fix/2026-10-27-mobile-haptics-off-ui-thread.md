# Agent Note: 开了按键振动后打字不跟手

Status: implemented

## Problem

用户反馈三端开了按键振动后快速打字跟不上手：字上屏变慢、偶尔丢键，按下态和按键动画也晚出来。关掉振动就恢复。

三端的振动都是在 UI 线程上、按键处理的路径里同步发出的，每按一下键都要等一次跨进程调用：

- Android：默认档位是「中」，`KeyboardHaptics.play` 在主线程上依次调用 `hasVibrator`、`areAllPrimitivesSupported`、`vibrate`，这几个都是到 system_server 的同步 binder 调用。部分厂商 ROM 的振动服务一次要几到几十毫秒，时间直接压在下一次触摸事件和下一帧上。
- HarmonyOS：`vibrator.startVibration` 虽然返回 Promise，NAPI 层却先在调用线程上把请求同步发给振动服务，之后才返回。`tapFeedback` 在 `press` 处理按键之前调用它。
- iOS：`UIImpactFeedbackGenerator` 只能在主线程上调用，绕不开。但每按一次键，按下时的 `prepareKeyFeedback` 和松开时的 `playInputClick` 加起来要读振动开关和档位五六次，而 `KeyboardFeedbackPreference` 每读一次都新建一个 App Group 的 `UserDefaults`。

档位怎么振见[振动档位](2026-10-08-keyboard-haptic-levels.md)，本篇不改那里的映射，只改振动在哪个线程上发、每按一下读多少次偏好。

## Decision

- Android `KeyboardHaptics`：轻、中、强三档的 `Vibrator` 调用都放到单独的 daemon 线程 `msime-haptics` 上，`play` 交出去后立即返回，不等它。「跟随系统」仍在主线程上调用 `View.performHapticFeedback`，它必须在视图所在的线程上调。后台线程上发现没有振动器时，也经 `view.post` 回主线程交给系统。
- HarmonyOS：振动请求包成 `taskpool.Task(startKeyVibration, …)`，以 `Priority.HIGH` 发出。打字统计写盘也在 TaskPool 里跑，要拿文件锁、整份重写，振动排在它后面就晚了。`KeyVibration.ets` 里只引入 `@ohos.vibrator`。效果支不支持仍在 UI 线程上查，每个效果只查一次（`supportsEffect`）。TaskPool 这条路失败过一次，就改回在 UI 线程上发（`vibrateHere`）：本机和 CI 都没有鸿蒙真机，无法确认每台设备的 worker 里都能用振动模块，退回原来的做法至少还能振。
- 两端都不让振动排队：上一下还没交给振动服务时又按一下，这一下就不振了（Android 的 `pending`、鸿蒙的 `vibrationPending`）。振动服务慢的设备上，排起来的振动会在手指离开后才一串串地振，比少振一下更糟。
- iOS：`KeyboardViewController` 留着一个 App Group 的 `UserDefaults`（`feedbackDefaults`），按键时经 `KeyboardFeedbackPreference.hapticsEnabled(in:)` 和 `hapticStrength(in:)` 从这个实例读，不再每读一次就新建一个实例。值不缓存，App 里改了振动，下一下按键就生效。`impactOccurred` 仍在主线程上同步调用。

## Alternatives considered

- **Android 和鸿蒙改用 `Handler.post` / `setTimeout` 把振动挪到按键处理之后**：改动最小，这一下的字能先上屏。但振动仍然占着 UI 线程，只是推迟了，下一次触摸事件和下一帧照样要等它，快速连按时与原来没有区别。
- **振动排队，一下都不丢**：每次按键都能振。但服务慢的时候振动会越积越多，在手指早已离开后才振完，手感比偶尔少振一下更乱。
- **iOS 改用 Core Haptics 或 `AudioServicesPlaySystemSound`，在后台线程上发**：可以完全离开主线程。但 Core Haptics 在键盘扩展里能不能启动引擎没有把握；系统声音 id 只有固定的几种振感，接不上现有的轻、中、强三档，等于改了用户选的手感。这两条都要先上真机比对，本次不做。
- **iOS 把振动开关和档位的值缓存在键盘里，只在键盘出现、偏好重读时重读**：按键时连 `UserDefaults` 都不用碰。这个 PR 第一版就是这样做的，审查时撤掉了：App 的「键盘布局」设置页里也有输入框，键盘开着时在同一页改振动，要等键盘收起再弹出才生效，而原来是即时生效的。
- **iOS 给 `KeyboardFeedbackPreference.defaults` 换成静态缓存的实例**：所有读偏好的地方都会受益。但这个类型 App 和键盘两边共用，改它影响的是全部偏好，而不只是按键路径。

## Consequences

- **收益**：Android 和鸿蒙的按键路径上不再有振动的跨进程调用；iOS 每按一次键少新建五六个 `UserDefaults`，振动设置仍然即时生效。
- **代价**：振动服务慢时，快速连按会少振几下。鸿蒙多了一条 TaskPool 失败后退回 UI 线程的分支要维护。
- **未验证**：三端都没有在真机上量过改动前后的按键延迟；iOS 的 `impactOccurred` 本身占主线程多久没有测过，如果真机上它就是主要开销，还需要另外处理。鸿蒙的 TaskPool 路径只在 DevEco 里编译通过，没有在设备上确认真的振了。

## Verification

- Android：`bash platforms/android/check-host.sh`。
- HarmonyOS：`bash platforms/harmony/tests/run.sh`（钉住振动经 TaskPool 发出、不排队的接线），`hvigorw assembleHap` 编过 ArkTS。
- iOS：键盘扩展在模拟器目标上编译通过。
