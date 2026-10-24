# Agent Note: iOS 的 MetricKit 崩溃诊断按进程号对回键盘会话，系统强制结束单独归类

Status: implemented

## Problem

iOS 最近 7 天（`admin_events`，2026-10-25 只读聚合）有 942 台设备报过 `active`，其中 350 台报过 `crash`（37%），共 2006 条 `crash`，而 `session_crash` 只有 2 条，`session` 有 37725 条。逐条按来源和归类聚合后，这个比例不是「键盘崩了却没记到会话上」，而是 `crash` 里装的大多不是崩溃：

- 2006 条里 2004 条是 App 转写的 MetricKit 诊断（摘要以 `Crash exception` 开头），只有 2 条是键盘自己的信号处理函数写的 `Fatal signal SIGSEGV`。这 2 条各有同一会话 id 的 `session_crash`；近 30 天的 6 条 `session_crash` 里能对上的 5 条也全是键盘自己记下的崩溃。键盘进程内的崩溃记录路径一直是对的。
- 30 天 3080 条 `crash` 里约 3020 条是 SIGKILL（`exception 10 code 0 signal 9`），即进程被系统强制结束，不是自己出错。最大的两组的栈都是键盘扩展（主线程停在 `ExtensionFoundation`/`PlugInKit` 的 runloop 里，`mach_msg` 等消息）：`35f67d4365f36f0c`，终止原因 `Namespace RUNNINGBOARD, Code 0xdead10cc`（挂起时持有共享容器里的文件锁），7 天 1212 条、181 台设备；`6e83932b43d88846`，没有终止原因的 SIGKILL，7 天 542 条、62 台设备。App 自己也有 `0xdead10cc`（`74649bd43d63e8f3`，169 条）和看门狗 `0x8badf00d`。
- 真正的进程出错（SIGABRT、SIGTRAP、`NSInvalidArgumentException`）30 天只有约 55 条，多数在 App 里（`cc07e090101da760`，栈里有 `MSIMEApp` 和 `SwiftUI`）。

所以无崩溃会话率里分子分母大体是对的，失真在「报过崩溃的设备比例」和崩溃组列表：系统强制结束和 App 的崩溃都以 `crash` 形式、以 `Crash` 开头的标题出现，和真崩溃混在同一组里（`cc07e090101da760` 同时含 signal 6 与 signal 9）。

此外有两处与共享语义不一致的缺口，眼下数量小，但会随 MetricKit 覆盖面变大：

- `CrashDiagnostics` 把每条 MetricKit 诊断都写成新 id 的记录，从不对回会话。键盘在展示中出错而信号处理函数没来得及写（或根本收不到，例如 abort 之前栈已坏）时，这次崩溃只有 `crash`，没有 `session_crash`；处理函数写过的那次，MetricKit 之后又送来一份，同一次崩溃报两条 `crash`。
- 键盘按标记结束会话、写未捕获异常的记录（`end()`、`recordCrash()` 不带会话 id）。同时有两个键盘进程（iPad 分屏、换宿主应用时新进程先展示）时，标记会被后开始的进程改写，结束的是别人的会话。#9590 为此给 Host API 加了按 id 的用法，iOS 当时没有跟进。

## Decision

- 共享层（`crates/client-services/src/telemetry.rs`）的会话标记多记一项开始它的进程号 `pid`（只在本机，不随事件发送），`telemetry-state.json` 另记最多 16 个已补报过 `session_crash` 的进程号 `crashed_processes`。新增 `TelemetryStore::record_diagnosed_crash(pid, message, stack)`：`pid` 在已补报列表里，或它的未结束会话已经有崩溃记录时，不再写（`Duplicate`）；它有未结束的会话时写成最近开始的那个会话的记录（`Session`），下次开始会话时照常补报 `crash` 加 `session_crash`，版本取那个会话的；都不是或 `pid` 为 `None` 时写成新 id 的单独记录（`Outside`），只报 `crash`。
- Host API 的 `msime_client_telemetry_record_crash` 多两个互斥的可选字段：`pid` 对应上面带进程号的用法，`outside_session: true` 对应不带进程号的用法；与 `session` 同时给时报错。返回值仍是是否写下了记录。其余平台的调用不变。
- iOS App 的 `CrashDiagnostics` 用 `UsageReporting.diagnosticSummary` 生成摘要并归类：信号是 SIGKILL 的摘要以 `Killed` 开头，按共享语义「被系统杀掉不算崩溃」不对回会话（`outside_session`）；其余以 `Crash` 开头，带 `MXMetaData.pid`（iOS 17 起提供，部署目标正是 17）调用，`pid` 不可用时同样按单独记录处理。写记录改由共享层完成，原来 Swift 自己写文件的 `storeCrashDiagnostic` 由此退出，符号链接检查改由共享层的 `create_directory_and_check` 负责。
- 键盘的 `KeyboardUsageReporting` 记住 begin 返回的会话 id（放在锁里，未捕获异常处理函数可能在任何线程读），收起时按 id 结束，未捕获异常按 id 写记录。

系统强制结束仍然上报，只是标题不同：`0xdead10cc` 是我们自己在挂起前没放掉 App Group 里的文件锁，看门狗是我们卡住了主线程，都是要修的缺陷，不能因为它们不是崩溃就丢掉。服务端的事件种类只有 `crash` 与 `session_crash`，新增种类要先改服务端的约束，所以这里只让它们在崩溃组里自成一组（签名由规范化后的首行和首个非系统帧算出，`Killed` 与 `Crash` 的首行不同）。

## Alternatives considered

- **在 App 里按诊断里的二进制名区分键盘与 App，只把键盘的诊断记成会话崩溃**：最强的理由是不需要往会话标记里加任何东西。但「是键盘」不等于「是正在展示的那次会话」：`0xdead10cc` 正是键盘收起、会话已经正常结束之后被挂起时发生的，按二进制名会把它们全部记成会话崩溃；而且它分不出这次崩溃是否已经由信号处理函数记过，去不了重。进程号同时回答了「哪个会话」和「是否已经报过」。
- **把 SIGKILL 的诊断直接丢掉，与「只留标记的会话不产生事件」完全一致**：能让报过 `crash` 的设备比例立刻接近真实崩溃率。但这样 `0xdead10cc` 和看门狗这些我们自己造成、用户会遇到（下次弹出键盘是冷启动、App 回到前台被重启）的问题就从遥测里消失了，等于隐藏缺陷。
- **服务端新增一种事件（例如 `termination`）**：分类最干净，看板也能直接排除。但需要先改 msime-cloud 的表约束和接口，客户端先发新种类会被 `400` 拒绝并丢弃。客户端先用标题区分，服务端要不要再拆种类是后续的事。
- **键盘扩展自己订阅 MetricKit**：诊断不必等 App 打开。但扩展的生命周期很短、随时被挂起，诊断投递到哪个进程没有实测过；线上数据已经证明 App 能收到键盘扩展的诊断，这次不改投递方。

## Consequences

- 收益：MetricKit 报来的键盘真崩溃会补上它所在会话的 `session_crash`，与信号处理函数记下的那一份不重复；系统强制结束在崩溃组里独立成组，标题以 `Killed` 开头，不再和 App 的 SIGABRT 混在一起；两个键盘进程交替展示时各自结束自己的会话。
- 代价：已有的崩溃组 `35f67d4365f36f0c`（已标为 known）、`6e83932b43d88846`、`74649bd43d63e8f3` 在新版本上会换成以 `Killed` 开头的新组，旧组的状态不会带过去。「报过 `crash` 的设备比例」在服务端不按标题区分之前仍会算上这些强制结束。
- 已知上限：进程号会被重用，已补报列表和未结束会话列表各只记 16 个，跨很久的重用可能把一次崩溃判成重复；按进程号命中的若是同一进程里更早被强制结束的会话，取的是最近开始的那个。单独记录的版本仍取下次启动的键盘版本，不是诊断里的 `applicationVersion`，升级之后才送到的诊断会记到新版本上。MetricKit 只在 App 被打开时投递，只用键盘、从不打开 App 的用户的诊断一直到不了。
- 没有处理的：`0xdead10cc` 本身。MetricKit 给的是崩溃线程（空闲的主线程）的栈，看不出哪个线程当时持着锁；仓库里已经修过几处挂起前不放锁的路径（[收起时释放词库快照租约](2026-10-14-ios-snapshot-worker-lock-release.md)、`suspendDictionarySession` 收起时销毁会话交还词库租约），剩下的候选包括收起后仍在后台队列上取 App Group 文件锁的工作（使用统计的 `end`、打字统计落盘等），需要真机复现或带上其他线程的栈才能定位。

## Verification

- `cargo test -p msime-client-services --lib telemetry`：38 项通过，其中 `a_diagnosed_crash_belongs_to_the_open_session_of_its_process_once` 覆盖按进程号对回会话、MetricKit 重复送达、信号处理函数已写过（认领前后各一次）、没有会话的进程和强制结束；`a_diagnosed_crash_takes_the_newest_session_of_a_reused_process_id` 与 `clear_forgets_the_crashed_processes` 覆盖重用的进程号和关闭上报。
- `cargo test -p msime-host-api --lib telemetry`：`telemetry_abi_attributes_a_diagnosed_crash_to_the_session_of_its_process` 经 C ABI 走一遍 `pid`、`outside_session` 与互斥校验，补报的 `session_crash` 正是那个会话的。
- `cargo clippy -p msime-client-services -p msime-host-api --all-targets -- -D warnings` 无警告。
- `platforms/ios/tests/core/UsageReportingTests.swift` 的新用例（摘要归类、诊断对回会话且只记一次、会话外的记录权限与单行摘要、不跟随符号链接）链接真实的 `libmsime_host_api` 跑过；完整的 `MSIMEClientTests` 由 PR 上的 `ci-ios.yml` 跑。

## Related notes

检索了活跃笔记里的 MetricKit、`session_crash`、崩溃记录、`0xdead10cc` 与文件锁：[遥测会话按 id 归属](2026-10-24-telemetry-session-ownership.md) 部分重叠，本篇沿用它的按 id 认领与未结束会话列表，并把 iOS 宿主接到按 id 的用法上；[macOS 输入法被 SIGTERM 结束时照常结束使用统计会话](2026-10-24-macos-session-on-sigterm.md) 部分重叠，本篇沿用它「被系统杀掉的进程不算会话崩溃」的取舍来归类 SIGKILL；[收起时释放词库快照租约](2026-10-14-ios-snapshot-worker-lock-release.md)、[词库维护定时器的文件读取移出主线程](2026-10-22-ios-idle-maintenance-off-main.md) 与 `0xdead10cc` 的成因相关，本篇不改它们的结论；没有被本篇吸收或需要否决的旧笔记。
