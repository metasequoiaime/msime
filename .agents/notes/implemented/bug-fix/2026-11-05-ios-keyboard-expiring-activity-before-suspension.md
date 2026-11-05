# Agent Note: iOS 键盘持锁期间开着 expiring activity，挂起前放掉 App Group 的锁

Status: implemented

## Problem

#10922 让两秒一轮的维护定时器发现「键盘视图不在窗口里」时按收起处理，放掉词库锁（见 [iOS 键盘只在可见期间持有引擎会话](2026-11-02-ios-keyboard-session-only-while-shown.md) 的「宿主被结束」一条）。它在模拟器上有效，因为模拟器的扩展在宿主被结束后不会被挂起。2026-11-05 在 iPhone XR（iOS 18.7.11）上复验，键盘显示时结束宿主 10 次，10 次都被 RunningBoard 以 `0xDEAD10CC` 结束（#10910 重新打开）：

- 宿主在 .355 被 SIGKILL，扩展在 .394 被挂起（约 40 ms 后），.462 被结束；系统点名的锁是 `MSIME/user/.msime-dictionary-access.lock` 和 `MSIME/user/dictionaries/<代次>/.msime-dictionary-access.lock`。
- 诊断日志在 `keyboard_shown` 之后什么都没有，没有任何 UIKit 回调。唯一一次记到 `keyboard_window_lost` 的进程，在写那一行日志时就被挂起了。
- 正常的显示、收起不受影响：设置页和试用页之间切换 50 轮，每次收起都有 `dictionary_suspended`，0 次 `0xdead10cc`。

扩展在宿主没了之后几十毫秒就被挂起，任何轮询都来不及；锁只要还在手上，就要在挂起之前放掉，或者根本不跨过空闲期持有。

## Decision

键盘持着 App Group 里的锁期间，开着一段 `ProcessInfo.processInfo.performExpiringActivity(withReason:using:)`；系统要挂起扩展之前以 `expired == true` 调它，那次调用返回之前在主线程上同步放掉全部锁。

- `AppGroupLockActivity`（`platforms/ios/KeyboardExtension/Sources/keyboard/AppGroupLockActivity.swift`）包着这个 API。`begin(expire:)` 开活动：第一次调用（`expired == false`）的块停在信号量上，活动持续到 `end()` 发信号为止；到期调用（另一条线程上的第二次调用，或系统一开始就不给活动时唯一的一次）把 `expire` 派到主线程同步执行，最多等 `expiryWait`（3 秒），再结束活动。`begin` 幂等；持有者被释放时 `deinit` 结束活动，不留下永远停着的线程。系统入口是可替换的静态 `perform`，单测换成假的，按真机上的线程关系去调块。
- `KeyboardViewController` 在 `viewWillAppear` 建会话之前 `begin`，在 `viewWillDisappear`、`viewDidDisappear` 放掉会话之后 `endLockActivityIfReleased()`：会话没放掉（`dictionary_suspend_failed`）时活动不关，系统要挂起之前还会再放一次。活动覆盖的是可见期内全部的 App Group 锁：会话的两把 `.msime-dictionary-access.lock` 共享租约、下载代次的 `generations/<代次>.lock`、快照工作者的 `worker.lock`，以及可见期内维护操作临时取的独占租约。
- 到期时执行 `releaseLocksBeforeSuspension()`：还显示着就走 `viewWillDisappear`、`viewDidDisappear` 的全套（与 `maintenanceTick` 的 `keyboard_window_lost` 同一条路），否则停快照工作者和维护、放会话。前后各记一行诊断：`suspension_imminent presented= window= session_held=`、`suspension_released ms= session_held=`。
- #10922 的两秒定时器保留。真机上它仍是平时放锁的那条路，活动只是让扩展在宿主没了之后不被立刻挂起，等得到它；定时器没赶上时由到期调用兜底。

### 真机上量到的 RunningBoard 行为

iPhone XR（iOS 18.7.11）的 syslog：

- 扩展开活动时 RunningBoard 记「will be created as inactive as start-time-defining assertions exist」：宿主在、键盘显示着的时候这段活动不计时。键盘在活着的宿主里空闲显示 150 秒，没有任何到期警告，之后第一个键照常组字。
- 宿主被 SIGKILL 后 2 ms 内「Activating delayed assertion」，活动转为生效（`FinishTask`），扩展保持 `running-active`，没有在 40 ms 后被挂起。
- 定时器在宿主被结束后约 1 秒内发现视图不在窗口里、放锁（`dictionary_suspended ms=428`）、结束活动；随后扩展被挂起，「check if suspended process is holding locks」之后没有「Found locked file lock」。
- 把定时器的判断临时关掉（只在本机做的实验构建）时，宿主被结束约 26 秒后 RunningBoard 发出「Assertions for process will expire soon」，到期调用在 8–62 ms 内放掉锁，约 5 秒后扩展被挂起，同样没有找到锁。

### App Group 里的 SQLite

另查了会不会还有别的 App Group 文件锁触发 `0xdead10cc`。引擎的 SQLite 在 iOS 上一律用回滚日志：`user_dictionary::journal::prefer_write_ahead_log` 在 `target_os = "ios"` 时不切 WAL，代码里其余的 `journal_mode=WAL` 都在测试里。回滚日志只在事务进行期间持锁，两次事务之间不持；可见期内的事务被活动覆盖，会话销毁经输入线程串行，排在进行中的按键之后。真机两轮 syslog 里系统点名的只有上面两把租约锁，没有 `.db` 文件。

## Alternatives considered

- **只在实际读写文件时才取租约，不跨空闲期持有**（issue 给的方向 b）——最强的理由是从根上不持锁，不依赖系统给不给活动。不用：租约的语义是「会话在用这些数据，维护不能同时改」。会话从建立到销毁一直开着词库和学习日志的 SQLite 连接、引用着代次目录里的文件；App 的维护（改个人词库、重置学习、换代次后回收旧代次）靠独占租约确认没有会话在用。按操作取锁，两次按键之间维护就能替换会话正在用的文件，引擎要在每次重新取锁后核对、重开这些状态，这是六个平台共用的 `msime-engine` 契约（`access.rs` 文件头写明锁文件名和规则不能改）。即使这样改了，按键进行中正好遇到宿主被结束，几十毫秒的窗口仍可能落在持锁期内，还是需要挂起前的通知。
- **把锁文件挪出 App Group**（同属方向 b）——RunningBoard 只查共享容器里的锁，挪出去就不会被点名。不可行：锁要在键盘扩展和 App 两个进程之间互斥，两边都能看到的位置只有 App Group 容器。
- **缩短定时器间隔，或在别的回调里更早发现宿主没了**——改动最小。不用：扩展在宿主被结束后约 40 ms 就被挂起，没有任何回调（#10922 的笔记里试过 `didMoveToWindow`、`NSExtensionHost*` 通知，都不来），多快的轮询都是在赌。
- **整个扩展进程生命期都开着活动，不随可见期开关**——实现更简单。不用：键盘收起之后锁已经放掉，开着活动只会让系统没法挂起一个空闲的扩展，白白耗电；收起时 `viewWillDisappear` 一直是可靠的（50 轮 50 次），没必要。

## Consequences

- **收益**：宿主在键盘显示时被结束，扩展不再带着 App Group 的锁被挂起；不改引擎、不改任何平台共用的锁契约。
- **收益**：放锁有了第二条与定时器无关的路径；同一进程里没收到回调的控制器各自开着自己的活动，到期时各自放锁（实验构建里量到一次两个控制器同时到期，都放掉了）。
- **代价**：键盘显示期间一直有一条 GCD 线程停在活动的信号量上。宿主被结束后扩展多活最多约 30 秒（平时约 1 秒，由定时器结束活动）。
- **代价**：到期时主线程上要做完 `viewWillDisappear` 的全套，XR 上量到 8–62 ms（会话这时通常没有组字）；正常路径（定时器）里销毁会话量到约 430 ms。到期调用最多等主线程 3 秒，主线程被别的同步调用占满 3 秒以上时仍可能带锁挂起。
- **重访信号**：诊断日志里出现 `suspension_imminent window=1`（宿主还在、键盘还在窗口里却被要求挂起），说明活动在可见期内也会到期，现在的处理会在键盘显示着时销毁会话；出现 `suspension_released session_held=1` 说明到期时没放掉。Console 里再次出现 `0xdead10cc` 且点名 `.db` 文件，说明有 SQLite 事务跨过了挂起。
- **未覆盖**：App 进程（`MSIMEApp`）自己的 `0xdead10cc`（崩溃诊断里另一组，见 [崩溃诊断归因](2026-10-25-ios-crash-diagnostic-session-attribution.md)）不在这次范围。

## Verification

- `MSIMEKeyboardTests/AppGroupLockActivityTests`：到期调用返回之前 `expire` 已在主线程上执行完、第一次调用随之返回；`end()` 结束活动、`begin` 幂等；系统一开始就不给活动时立刻放锁；持有者释放时活动结束。
- `MSIMEKeyboardTests/NineKeyKeyboardTests/testKeyboardReleasesTheLeaseBeforeTheSystemSuspendsIt`：换上假的系统入口，显示的键盘开着一段活动、持锁、能组字；移出窗口后在后台线程上调到期的块，块返回时两把 `.msime-dictionary-access.lock` 都能取得独占 flock，维护定时器已停；再出现时开新的活动，第一个键照常组字；正常收起时活动随锁结束。
- 真机（iPhone XR，iOS 18.7.11，签名的 Release 构建，XCUITest 驱动、只用合成输入）：键盘显示时 SIGKILL 宿主，`MSIMEKeyboardTestHost` 独立编辑器 12 次加水杉 App 试用页 3 次，syslog 里 15 次「check if suspended process is holding locks」、0 次「Found locked file lock」、0 次 `0xdead10cc`，没有新的键盘扩展崩溃报告；每次之后新宿主里的第一个键都组字。同一构建再跑 16 次，没有新的键盘扩展崩溃报告。关掉定时器判断的实验构建 18 次，0 次 `0xdead10cc`，到期路径如上。键盘在活着的宿主里空闲 150 秒没有到期警告。设置页和试用页之间切换 50 轮，键盘 50 次都出现，诊断日志里 50 次出现各对应一次 `dictionary_suspended`，0 次 `0xdead10cc`。
