# Agent Note: macOS 输入控制器的最后一次释放只落在主线程

Status: implemented

## Problem

0.52.0 的崩溃报告（`水杉输入法-2026-10-10-095444.ips`）里输入法进程以 SIGTRAP 退出，崩溃线程在 `com.apple.root.utility-qos` 上：`-[MSIMEInputController reloadPreferences]` 的后台块 → `-[MSIMEInputController dealloc]` → `-[MetasequoiaFloatingToolbarPanel deactivateForDelegate:]` → `orderOut:` → AppKit「Must only be used from the main thread」。输入法进程一崩，当前应用里切到水杉就没有反应，直到系统重新拉起它，这可能是 #6667 里「切到水杉没反应」的来源。

后台块用 `MSIMEInputController *current = weakSelf;` 把弱引用提升成强引用去读偏好文档。读的过程中 IMK 放掉了这个控制器，后台的 `current` 就成了最后一个强引用，块结束时控制器在读取线程上释放，`-dealloc` 收起浮动工具栏、候选窗口时在那条线程上碰了 AppKit。

同一类问题还有一个入口：打开设置应用的几个菜单动作把强引用 `self` 的回退块交给 `-[NSWorkspace openApplicationAtURL:configuration:completionHandler:]`。SDK 写明这个完成回调在并发队列上调用，回调持有回退块，回调在那条队列上被释放时，若 IMK 已在设置应用启动期间放掉控制器，最后一次释放同样落在后台。

## Decision

控制器 `-dealloc` 里的 AppKit 清理只在主线程执行，做法是让控制器的最后一个强引用不在后台线程放掉，`-dealloc` 本身不变：

- `reloadPreferences` 的后台块读完偏好后调用 `MSIMEReleaseControllerOnMain(&current)`，把强引用交给主队列释放，再像原来一样派发按弱引用取控制器的完成块。释放先入队，完成块看到的控制器和以前一样：IMK 已放掉它时取到 nil，不在一个已被放掉的控制器上应用偏好。
- `MSIMEReleaseControllerOnMain` 在主线程用 `CFRelease` 当场释放。原来的 `(void)CFBridgingRelease(owner)` 在未优化构建里会把返回值放进主线程 run loop 的自动释放池，控制器要活到这一轮回调结束，排在后面的完成块就会取到一个 IMK 早已放掉的控制器。候选释义的两处既有调用（`synchronizeCandidateGloss`、`synchronizeTargetGloss`）因此也改为当场释放。
- `showCloudClipboard:`、`showCloudDictionary:`、`showDictionary:` 的回退块和 `restartCurrentInputMethod` 的完成块只捕获 `__weak` 引用，在主线程执行时再取强引用。控制器在设置应用启动期间被放掉时，回退不再执行，`restartCurrentInputMethod` 仍会结束进程。

广度扫描的结论：`InputController.mm` 里其余在后台线程出现的弱引用提升，要么在 `dispatch_async(main)` 的内层块里，要么回调本身由提供方派发到主线程（`CustomTranslationBatch`、`DesktopInputSession`、各语音请求、`VoiceInputService` 的权限和识别回调）；后台线程上被提升或最后释放的其他对象（`DesktopInputSession`、`DoubaoVoiceRequest`、`LocalVoiceRequest`、`HTTPVoiceRequest` 等）的 `-dealloc` 不碰 AppKit。`-dealloc` 里有 AppKit 操作的其他类要么是单例（浮动工具栏、账号窗口、云剪贴板窗口），要么只由控制器独占（`MSIMEVoiceWaveOverlay`、`MSIMEDictionaryWindowController`），控制器在主线程释放它们也就在主线程释放。

## Alternatives considered

- **在 `-dealloc` 里把 AppKit 清理派发到主线程** — 最强的理由是一处改动就能兜住所有现在和将来的后台释放入口，不必逐个找调用点。不用它是因为 `-dealloc` 里的 `[_toolbar deactivateForDelegate:self]` 靠 `self` 做所有者比较，派发出去的块不能再引用正在释放的对象，只能带一个裸指针做比较；`removeObserver:`、`NSEvent removeMonitor:` 和 `flushKeyPresses` 也都要求同步完成，拆成「部分同步、部分派发」会让 `-dealloc` 的行为取决于在哪条线程上被调用，比让最后一次释放固定在主线程更难推理。
- **把强引用带到主线程的完成块里再释放（`cc77e43248` 回退掉的那版做法）** — 它同样保证释放落在主线程，而且只改一处。但它让完成块在一个 IMK 已放掉的控制器上照常应用偏好，改变了完成时机，当时 `WaitForPreferenceCompletions` 因此失败而被回退。本次的做法把释放排在完成块之前，完成块的语义不变。
- **后台块完全不持有控制器，只捕获读偏好所需的数据** — 这是最干净的形态，但 `readPreferencesSnapshotInDirectory:error:` 和 `recoverPreferencesInDirectory:error:` 是实例方法，测试用子类覆盖它们来控制读取时机（`AsyncPreferencesController` 等），改成类方法或函数要连带改动一批测试，超出这次修复的范围。

## Consequences

- **收益**：偏好读取和打开设置应用这两条路径不会再在后台线程触发控制器的 `-dealloc`，消除这次崩溃报告里的 SIGTRAP；`MSIMEReleaseControllerOnMain` 的语义变成「主线程上当场释放」，后续新增的后台读取可以直接复用它而不改变排在后面的主线程块看到的状态。
- **代价与已知上限**：设置应用启动期间控制器被放掉时，云剪贴板、云词库、词库的原生回退窗口不再弹出（这一窗口期很短，启动失败才会走回退）。这只修了已知入口：以后新增的后台块若强引用控制器，仍会重新引入这类崩溃；再出现同类崩溃时，应考虑上面第一个备选方案。

## Verification

`platforms/macos/tests/input/ShortcutTest.mm`（ctest `shortcut`）：

- `TestPreferenceReadDoesNotDeallocControllerOffMain` 在偏好读取进行中放掉主线程上的全部引用，断言浮动工具栏的 `deactivateForDelegate:` 只在主线程调用一次，且完成块没有在被放掉的控制器上运行。修复前在 `!toolbar.deactivatedOffMain` 处失败；只修 `reloadPreferences`、`MSIMEReleaseControllerOnMain` 仍用 `CFBridgingRelease` 时在 `toolbar.completions == 0` 处失败。
- `TestDesktopLaunchDoesNotDeallocControllerOffMain` 把 `NSWorkspace` 的启动方法换成只收下完成回调的桩，在后台队列上释放回调，覆盖 `showDictionary:` 和 `restartCurrentInputMethod`。修复前在 `!toolbar.deactivatedOffMain` 处失败。
