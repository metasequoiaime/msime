# Agent Note: macOS 输入法被 SIGTERM 结束时照常结束使用统计会话

Status: implemented

## Problem

共享遥测把 `session` 定义为「宿主进程正常结束时排进一条」，只留下会话标记、没有崩溃记录的会话不产生任何事件（`crates/client-services/src/telemetry.rs`）。macOS 输入法只在 `NSApplicationWillTerminateNotification` 和 `[NSApp run]` 返回之后调用 `MSIMEUsageReportingStop`，而一个输入法进程一跑就是几天，几乎从不走这两条路：设置应用每次装好或更新输入法都用 `pkill`（SIGTERM）结束旧进程（`apps/desktop/src-tauri/src/platform/macos/macos_input_source.rs` 的 `stop_running_copies`），`scripts/install.sh` 也一样；进程没有 SIGTERM 处理，按默认处置当场结束，标记留在磁盘上，下次启动什么都不报。崩溃则不受影响：崩溃处理程序写下的记录在下次启动时一定变成 `crash` 加 `session_crash`。

线上数据（`admin_events`，最近 30 天，只读聚合计数）印证了这一点：macOS 的 `active` 有 14776 条而 `session` 只有 79 条，iOS 和 Android 分别有 60130 和 46398 条 `session`；macOS 每个版本的 `session_crash` 都与 `crash` 条数完全相等（0.52.0 为 717 与 717，0.54.0 为 6 与 6），说明 `session_crash` 没有误报，每一条背后都有一份崩溃记录，失真的是分母。0.52.0 的 717 条里 684 条是同一个崩溃签名（SIGTRAP），分布在 197 台设备上，0.53.0 起不再出现。本机 runningboardd 日志里，最近四天输入法进程的三次结束都是 `termination reported by launchd (2, 15, 15)`，即被 SIGTERM 结束，期间没有重启过系统。

结果是控制台的无崩溃会话率（`session/(session+session_crash)`）对 macOS 毫无意义：0.52.0 只有 21 个正常会话却有 717 个崩溃会话。

## Decision

`MSIMEUsageReportingStart` 在会话开始且装好崩溃处理程序之后，用 `DISPATCH_SOURCE_TYPE_SIGNAL` 的调度源接管 SIGTERM（`platforms/macos/src/core/UsageReporting.mm` 的 `InstallTerminationHandler`）。收到 SIGTERM 时在专用串行队列上调用 `MSIMEUsageReportingStop` 排进 `session`，然后恢复默认处置并 `kill(getpid(), SIGTERM)`，进程仍以 SIGTERM 结束，launchd 和 `pkill` 看到的结果与以前相同。

- 只接管处置本来就是 `SIG_DFL` 的 SIGTERM；继承来的忽略或别人装的处理函数保持原样。
- 调度源在 kqueue 登记完成的回调里才把 SIGTERM 设为忽略，登记完成前到达的 SIGTERM 仍按默认处置结束进程，不会有被吞掉的窗口。
- 结束会话最多等 2 秒，到点无论 `msime_client_telemetry_end` 是否返回都按 SIGTERM 退出，卡住的进程照样能被 `pkill`、注销和关机结束。
- 使用统计关闭时 `MSIMEUsageReportingStart` 提前返回，不接管 SIGTERM，行为与以前完全一样。
- 会话语义不变：一个输入法进程就是一个会话，与 Linux 的 IBus 宿主、Windows 的 Server 和 Android 的 `:ime` 进程相同；只是「正常结束」现在包括 macOS 上最常见的那种结束方式。SIGKILL、断电仍只留下标记，按共享层的规则不产生事件。

## Alternatives considered

- **在共享层把「只有标记」的上一会话记成 `session`** — 一处改动就能让所有平台都不再丢会话，SIGKILL 结束的进程也算进去。但被看门狗或内存不足杀掉的进程同样只留下标记，iOS 键盘扩展被 jetsam 结束、Android 被低内存杀掉都会被算成正常会话，无崩溃率被系统性高估；这也改变了六个平台共用的语义，`PRIVACY.md` 和服务端口径要一起变，超出修 macOS 的范围。
- **改用 `[NSApp terminate:]` 走正常退出流程** — 能让其他 `NSApplicationWillTerminateNotification` 观察者也有机会收尾。但它依赖主线程还在跑，主线程卡住时进程就再也结束不了，`pkill` 失效；而且会让 SIGTERM 的退出状态变成正常退出，改变了系统和安装流程看到的行为。这里只补使用统计这一件事，退出方式保持原样。
- **缩短会话粒度，例如每次切入输入法或每天轮换一次会话** — 会话数会接近 iOS 的量级，进程被 SIGKILL 时也只丢最后一段。但这让 macOS 的会话含义与另外几个桌面宿主不同，崩溃发生在哪一段也要另外处理，而线上数据显示真正缺的是 SIGTERM 这一条路径。

## Consequences

- **收益**：macOS 每个被 SIGTERM 结束的输入法进程都会在下一次启动时报出一条 `session`，`session` 与 `session_crash` 回到「同一个会话二选一」的关系，无崩溃会话率对修复后的版本有意义。
- **代价与已知上限**：一个进程就是一个会话，macOS 每台设备每月的会话数取决于更新、注销和重启的次数，会比 iOS（每次弹出键盘一个会话）少得多，平台之间的会话率不宜直接比较。已经装着的 0.52.0 到 0.54.x 行为不变，它们的 `session` 依然接近零；从这些版本升级时，旧进程最后那个会话也照旧不报。服务端的无崩溃率需要排除 `session_crash` 多于 `session` 的版本，这一点不在本仓库内。

## Verification

CTest 的 `usage-reporting-termination`（`platforms/macos/tests/core/UsageReportingTerminationTest.mm`）起一个真的子进程运行 `MSIMEUsageReportingStart`，宿主函数换成只写标记文件的桩，然后向它发 SIGTERM：一是检查 `msime_client_telemetry_end` 被调用、子进程仍死于 SIGTERM；二是让 `msime_client_telemetry_end` 卡住 60 秒，检查子进程在宽限期内仍死于 SIGTERM。第一项在修复前的 `UsageReporting.mm` 上失败（`SIGTERM did not end the usage session`），修复后两项都通过；构建与运行用 `ctest --test-dir target/macos-isolated -R '^usage-reporting'`。
