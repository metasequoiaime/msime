# Agent Note: macOS 输入法进程一律以 SIGKILL 收尾，不让系统攒满死亡次数

Status: implemented

## Problem

几十位用户反馈水杉的输入模式「用着用着就变灰」，输入菜单里选不了，修复输入源、卸载重装、升级都不恢复，只有注销重登或重启电脑才好。

原因是 macOS 的 `imklaunchagent` 对每个输入法 bundle 记死亡次数（macOS 27.2 反汇编）：以第一次死亡为起点开一个 1800 秒的窗口，窗口内第 11 次死亡时广播 `com.apple.inputmethodkit.abnormaldeath`，日志写「Input Method(...) died too many times recently, macOS system will prohibit it to be launched for any of the processes that are currently running」。此后每个正在运行的应用里的 IMKClient 都把这个输入法标成不可用，状态留在各个应用进程里，换掉磁盘上的 bundle 解除不了。

计数不分死因：崩溃、被 SIGTERM 结束、`exit(0)` 都算，只有 LaunchServices 报的退出状态等于 9（被 SIGKILL 结束）的那次直接忽略。本机实测一致：`kill -9` 之后 `imklaunchagent` 没有记 `IMDiedCallback`，`kill -TERM` 之后记了 `IMDiedCallback for <private>, exitStatus (15)`，而且同一次死亡记了两条。

攒满 11 次的来源有两类。一是连续崩溃：遥测里近 21 天有 7 台设备在 30 分钟内启动了 11 次以上，分布在 0.52.0（后台线程释放控制器）、0.55.0（SIGABRT、NSRangeException）、0.56.0、0.58.0（Caps Lock HID 重复调度，#10999），只有三到五成设备上报会话，折算下来与反馈人数相当。二是我们自己：设置应用每次安装、更新、修复都用 `pkill` 结束输入法（`macos_input_source.rs` 的 `stop_running_copies`），输入法接管 SIGTERM 后按 SIGTERM 原样退出，每次都记一次死亡，所以用户越点「修复输入源」离封禁越近。在旧版本上被封的用户升级到新版本后仍然是灰的，看起来像新版本也有这个问题。

## Decision

输入法进程的每条退出路径最后都给自己发 SIGKILL，退出状态恒为 9。实现在 `platforms/macos/src/core/ProcessExitGuard.{h,mm}`，`input_method_main.mm` 在确认本进程是输入法（不是独立设置窗口、本地翻译设置窗口或命令行辅助模式）之后立刻安装，早于后端 dylib 加载和崩溃记录器：

- **致命信号**（SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT、SIGTRAP、SIGSYS、SIGPIPE）：处理函数只调用 `kill(getpid(), SIGKILL)`。崩溃记录器后安装，把它当作前任，写完并 `fsync` 崩溃记录后才转交（`CrashRecorder.cpp` 的 `crash_signal`），所以遥测里的崩溃记录不受影响。未捕获的 Objective-C 异常和 Rust panic 最终都走 `abort()`，同样经过这里。
- **正常退出**（`[NSApp terminate:]`、`main` 返回、任何 `exit()`）：`atexit` 回调发 SIGKILL。登记得早，所以排在别的 `atexit` 回调之后运行。
- **SIGTERM**：仍由 `UsageReporting.mm` 的调度源接管，先结束会话，再调用输入法登记的退出函数（`MSIMEUsageReportingSetTerminationExit(MSIMEExitByKill)`）。没登记的进程（独立设置窗口、测试）照旧按 SIGTERM 原样退出。

这篇部分取代 [macOS 崩溃上报](../feature/2026-11-01-macos-crash-reporting.md) 里「`SIGTERM` 走正常结束」之后的退出方式：会话仍然正常结束，只是进程最后的退出状态从 15 变成 9。那篇其余内容不变。

## Alternatives considered

- **只在快到上限时改用 SIGKILL**（启动时数最近 30 分钟的启动次数，超过阈值才打开）：最强的理由是头几次崩溃还能留下系统的崩溃报告。不选它：一次死亡可能被系统记两条（实测），真实阈值不确定；计数状态要跨进程落盘，启动早期就崩溃的进程来不及读。封禁的代价是用户整台机器打不了字，宁可不留余量。
- **只修各个崩溃**：崩溃当然要修，但每个新版本都可能带来新的连续崩溃，而且正常的安装、更新、修复本身也在计数。
- **设置应用改用 `pkill -9`**：能让安装和修复不计数，但输入法来不及结束会话和落盘键入统计，崩溃也照样计数。
- **被封之后由设置应用重启 `imklaunchagent` 和各个应用来解封**：封禁状态在每个客户端应用进程里，要重启用户所有的应用，等同于注销；没有验证过，不做。

## Consequences

- 输入法不会再因为崩溃、更新或修复被系统封禁。已经被封的机器仍要注销重登或重启一次，这次改动解除不了已有的封禁。
- 系统不再为输入法的崩溃生成 `~/Library/Logs/DiagnosticReports` 下的报告，崩溃栈只剩本进程崩溃记录器写下的那份；关闭了使用情况报告的用户崩溃后不留任何栈。
- 排在守卫之前登记的 `atexit` 回调和主映像的全局析构不再运行。输入法的收尾（会话结束、词库同步停止、键入统计落盘）都在 `exit()` 之前显式完成，不依赖它们。
- 豁免来自对 macOS 27.2 的反汇编和实测，更旧的系统没有验证；在不认这条豁免的系统上，行为退回到与改动前相同的计数，没有额外损失。
- 连续崩溃的版本仍然会反复崩溃和重启，只是不再变灰；崩溃本身照常要修。

## Testing

- `process-exit-guard`：起真的子进程，装守卫后分别以 `main` 返回、`exit(0)`、`exit(1)`、`abort()`、空指针写、`__builtin_trap()` 和「后装的处理函数转交前任」结束，全部被 SIGKILL 结束；不装守卫的对照组都不是。
- `usage-reporting-termination`：登记了 SIGKILL 退出的子进程收到 SIGTERM 后会话照样结束、被 SIGKILL 结束，结束会话卡住时宽限期之后同样被 SIGKILL 结束；没登记的两组仍按 SIGTERM 退出。
- 没有在装了这次改动的输入法上看 `imklaunchagent` 的日志。

检索了活跃笔记里的 `imklaunchagent`、SIGTERM、变灰和崩溃上报：[macOS 崩溃上报](../feature/2026-11-01-macos-crash-reporting.md) 部分重叠，已在上文互链；[遥测会话按 id 归属](2026-10-24-telemetry-session-ownership.md) 讲的是 Linux IBus 宿主的 SIGTERM，无关。
