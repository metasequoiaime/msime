# Agent Note: iOS App 自己记下未捕获异常的异常名和抛出点，归档的 dSYM 留作 artifact

Status: implemented

## Problem

崩溃组 `cc07e090101da760`（iOS，`Crash exception 10 code 0 signal 6`）近 7 天 49 次、35 台设备，前 7 天 28 次。它是[MetricKit 崩溃诊断按进程号对回键盘会话](2026-10-25-ios-crash-diagnostic-session-attribution.md)里说的「App 里的真崩溃」那一组，但组内样本没法定位：

- 最新样本的栈是 `__pthread_kill` ← `abort` ← libc++abi 的 terminate ← libobjc 的 `_objc_terminate` ← `objc_exception_rethrow` ← CoreFoundation ← GraphicsServices ← UIKitCore ← SwiftUI ← `MSIMEApp + 3952532` ← `dyld`。也就是主线程 runloop 回调里抛出的 ObjC 异常被 CFRunLoop 捕获后重新抛出，再 abort；唯一的 App 帧紧挨着 `dyld` 的 `start`，就是 `main`。抛出点在重新抛出时已经不在栈上，MetricKit 只给 abort 时的线程栈。
- 0.52.0 的摘要已经会带 `MXCrashDiagnostic.exceptionReason?.exceptionName`，这里却没有，连异常种类都不知道。
- 服务端分组掩去消息里的数字、去掉帧偏移，首个自有帧只剩 `msimeapp`，所以这一组装着 App 里所有不带异常名的崩溃（abort、Swift trap、非法访问），不一定是一个故障。分组的修正在 msime-cloud（metasequoiaime/msime-cloud#1565）。
- 即使拿到 `MSIMEApp + 偏移`，`ios-v0.52.0` 上也只有模拟器包，没有 dSYM；`upload-testflight.sh` 用 `uploadSymbols` 把符号交给了 App Store Connect（Xcode Organizer 能用），我们自己的遥测栈却没有可用的符号。

键盘扩展早就装了未捕获异常处理器（`KeyboardUsageReporting`），App 没有。

## Decision

- App 的 `CrashDiagnostics.start()` 在工具队列上读上报开关，无论启动时开关如何都安装 `NSSetUncaughtExceptionHandler`（保留并调用此前的处理器）。处理器经 `UsageReporting.recordUncaughtException` 按当前已经准备的捕获身份写崩溃记录；没有当前身份时不回退随机 id：消息是 `Uncaught <异常名>`，栈是 `NSException.callStackReturnAddresses` 经 `dladdr` 换成的「序号 二进制 + 偏移」，偏移从映像起点算，与 MetricKit 的 `offsetIntoBinaryTextSegment` 同一基准，同一份 dSYM 能符号化两者。下次键盘启动时它照常成为 `crash`。
- 异常回调只读取内存权限：`UsageReporting.setEnabled` 保存成功后同步更新，关闭后不再记录、重新开启后恢复；启动读盘用代次保护，不能把旧的开启状态盖回刚保存的关闭选择。回调取锁失败时放弃记录，避免崩溃线程重入同一把锁。 App 与键盘的独立观察队列还会在活跃期间刷新跨进程偏好；App 处理器每次获取当前捕获身份，不保存启动时的固定快照。生命周期、临时读取失败和验证缺口由[原生采集笔记](../feature/2026-10-27-native-crash-capture-and-symbol-identity.md)持有。
- 去重在 Swift 里做：仅崩溃记录写入成功后，处理器把进程号追加到 App 私有目录的 `uncaught-exception-pids`；`didReceive` 收到同一进程号的 MetricKit 诊断时由 `UsageReporting.takeHandledProcess` 认出、跳过并删掉这个进程号。文件只留最近 32 个。
- `upload-testflight.sh` 把归档里的 dSYM 打成 `msime-ios-<版本>-<构建号>-dsym.zip`，`release-ios.yml` 以同名 artifact 保留 90 天。归档里没有 dSYM 时只告警，不挡发版。

## Alternatives considered

- **把 App 的未捕获异常也交给共享层的 `record_diagnosed_crash` 按进程号去重**：去重逻辑只在 Rust 里一份。否定：那条路径要加锁、读写状态文件，文档明确不能在崩溃处理程序里调用；改成不加锁的版本要改共享遥测的状态格式。App 没有会话，`record_crash_for` 只建一个文件，再加一个 App 私有的进程号列表就够了。
- **消息带上 `reason`**：最直接的定位信息。否定：`reason` 可能引用 App 里的数据，MetricKit 那条路径也是因此不取 `composedMessage`；有了异常名和抛出点的偏移，配合 dSYM 足以定位到函数。
- **dSYM 照 `macos-symbols` 放滚动 release 长期保留**：与其他平台一致，不会过期。否定：nightly 在版本号已经发布后也送外部测试组，同一版本号每天都有新构建，release 附件会无限增长（GitHub 每个 release 有附件数上限）；排查用到的是最近几个月的构建，较旧的还能从 App Store Connect 取。

## Consequences

- 收益：App 主线程上未捕获的 ObjC 异常以 `Uncaught <异常名>` 自成一组，栈里有抛出点；配合 artifact 里的 dSYM 能符号化到函数。MetricKit 的同一份诊断不再重复计数。
- 代价：处理器装在读完上报开关之后，启动后极短的窗口里抛出的异常仍只有 MetricKit 的诊断。进程号会被重用，列表里过期的进程号可能让很久以后同号进程的 MetricKit 诊断被跳过；列表只留 32 个，且对上一次就删除。
- 没有处理的：`cc07e090101da760` 现有样本的根因。它们缺抛出点，要等装了处理器的版本上报，或从 Xcode Organizer 里 Apple 符号化好的同一批崩溃（带 Last Exception Backtrace）去找。遥测不带构建号，同一版本号的多个构建要按上报时间对应到当时分发的构建。

## Verification

- `platforms/ios/tests/core/UsageReportingTests.swift` 新增两项：返回地址换成「二进制 + 偏移」并以测试二进制自身的映像起点核对偏移、零地址记为 `???`；未捕获异常写成会话外的记录、只带异常名，MetricKit 同一进程号只认一次，列表只留最近 32 个。补充覆盖：记录目录不可写但 PID 文件可写时不标记已处理；内存权限的关闭、恢复和启动旧读覆盖保护。
- 本地对 `SharedUI`、`CrashDiagnostics.swift` 和测试文件用 iOS 17 模拟器 SDK 做 `swiftc -typecheck`，新代码无错误；用例本身由 PR 上的 iOS Simulator 任务运行。
- 审查补修用合成目录和现有 `host-api` 静态库独立运行同一份 `UsageReportingTests.swift`，13 项通过；写失败复现的旧代码返回 `failed_write_marked_handled=true`（退出 1），修复后为 false（退出 0）。该运行在 macOS XCTest 上，偏好桥由合成内存适配替代，不等同于 iOS 装机验收。改动文件与 `CrashDiagnostics` 另经 iOS 17 模拟器 SDK 类型检查通过。
- 补修的本地 quick 已通过仓库契约与笔记检查，进入 Rust 工作区编译后因系统盘仅余 15 GiB 主动停止，整条 quick 未完成；不新建完整构建树，完整编译留待 PR CI，未验证设备中的真实异常回调。
- `bash -n platforms/ios/upload-testflight.sh`、`actionlint .github/workflows/release-ios.yml` 通过；`nightly.yml` 不下载 iOS 的 artifact，新 artifact 名不会被它的模式误取。dSYM 打包和 artifact 上传要到下一次 TestFlight 发版才真正跑到。

## Related notes

检索了活跃笔记里的 MetricKit、未捕获异常、dSYM 与符号：[MetricKit 崩溃诊断按进程号对回键盘会话](2026-10-25-ios-crash-diagnostic-session-attribution.md) 部分重叠，本篇沿用它的会话外记录与按进程号去重，补上 App 自身的异常；[macOS 版本发布页只放 DMG](../process/2026-10-20-macos-release-page-only-dmgs.md) 与 [Windows 符号包挪到 windows-symbols](../process/2026-10-20-windows-release-page-symbols.md) 是符号长期保留的先例，本篇因 iOS 同版本多构建而改用 artifact，不改它们的结论；没有被本篇吸收或需要否决的旧笔记。
