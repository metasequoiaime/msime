# Agent Note: Windows 登录任务把 Server 降级的问题在本仓库的 Rust／共享内核版本里同样存在

Status: implemented

## Problem

msime-windows 的 PR 1328（`fix(server): 恢复交互优先级并修正登录任务降级`，2026-10-30 合并）修的是这条链：安装器用 `schtasks /Create` 建登录任务 `Metasequoia IME Watchdog`，没有设置 Priority，任务调度器默认 7（Below normal）。按 Windows 文档，Priority 7 同时把进程降到 `BELOW_NORMAL_PRIORITY_CLASS`、`MEMORY_PRIORITY_LOW` 和低 I/O 优先级；Watchdog 继承它，再拉起的 Server 又继承调度类。他们从 issue #927（开机阶段输入法不可用）和 #1239（高负载下卡顿）里定位到这一点。

本仓库（msime）的 Windows 产品是同一套进程结构：`platforms/windows/installer/msime_setup.iss` 用同样的 `schtasks /Create /SC ONLOGON /RL LIMITED /IT` 建任务，Server 与 Watchdog 仍是原生 C++ 宿主，打字路径最终落在共享的 Rust 宿主 DLL（`crates/host-api`、`crates/engine`）上。改动前全仓没有一处 `SetPriorityClass`、`SetThreadPriority` 或 `SetProcessInformation`，登录任务也原样留着默认值。本机（中文 Windows 11）实测：装好的 `Metasequoia IME Watchdog (Full)` 任务没有 `Priority` 和 `ExecutionTimeLimit` 节点（即默认 7 与 72 小时），运行中的 Watchdog `PriorityClass = BelowNormal`，Watchdog 经 `ShellExecuteExW` 拉起的 `MetasequoiaImeServer` 基础优先级是 6（Below normal）。

所以问题不是「Rust 版有没有」，而是同一份安装器和同一棵进程树本来就有；Rust 那一半不自建线程池，按键查询跑在 C++ 的输入队列线程上，进程被降级时它跟着降级。

## Decision

按 PR 1328 的三段处理，落到本仓库自己的结构上。电池策略不在其中，见「Consequences」。

**安装器**（`msime_setup.iss`）：`CreateWatchdogLogonTask` 改成返回 `Boolean`，创建成功后再跑新的 `ConfigureWatchdogLogonTask`。配置用 Task Scheduler 的 COM 接口（`Schedule.Service`）：读出任务定义，把 `Settings.Priority` 设为 4、`Settings.ExecutionTimeLimit` 设为 `PT0S`，用 `RegisterTaskDefinition($n,$d,6,$null,$null,3)` 重新注册（6 = `TASK_CREATE_OR_UPDATE`，3 = `TASK_LOGON_INTERACTIVE_TOKEN`），再从 `GetTask($n).Definition.Settings` 读回校验。

- 不用 `schtasks /Query /XML` 往返：在中文 Windows 上（控制台代码页 936）schtasks 重定向到文件时写的是当前代码页，XML 头里却声明 UTF-16；`[IO.File]::ReadAllText` 按 UTF-8 读会毁掉 `<Author>`、`UserId` 里的中文用户名或计算机名，再写回去轻则 Author 乱码，重则账户解析失败。本机把代码页切到 65001 时 schtasks 又写 UTF-8，行为随控制台走，没法靠一次测试锁定。COM 不经过文本，任务名含中文也照常工作（探针任务 `水杉 输入法 看门狗 探针` 实测通过）。
- 不用 `Get-ScheduledTask`/`Set-ScheduledTask`：那要 ScheduledTasks 模块，Windows 7 随系统的是 PowerShell 2.0，没有这个模块，而 Win7 安装包与本安装器是同一份脚本（`/DWindows7Architecture`）。COM 在 PS 2.0 上可用，`windows7-install-smoke.ps1` 早就用它读任务。
- 重新注册不会丢触发器或主体：定义是从任务本身读出来的，`RegisterTaskDefinition` 用同一份写回。本机实测 `LogonType` 仍是 `InteractiveToken`、`RunLevel` 仍是 `Limited`、触发器和动作不变。
- 失败兜底：注册失败后先看任务还在不在，只有真的没了才用同一份定义再注册一次重建。实测用坏 `UserId` 让注册失败时，任务和已生效的设置都还在（`Priority` 仍是 4），所以兜底只在任务被安全软件删掉这类情况下触发。
- 配置失败只写日志，不弹框：任务已经建好、开机自启照常，用户对「没调到正常优先级」做不了任何事，与 `StopProcessesUnder` 的处理一致。创建本身失败时连配置也不跑。

**进程**（`src/system/ProcessPriority.*`）：`wmain` 最前面、任何业务线程创建之前调用 `restore_interactive_process_priority()`，恢复 `NORMAL_PRIORITY_CLASS` 和 `MEMORY_PRIORITY_NORMAL`。两项独立尝试，失败只把错误码写进 `server.log`，不阻止启动。`SetPriorityClass` 不恢复内存优先级，必须分开设置。`SetProcessInformation` 从 Windows 8 才有，经 `GetProcAddress` 取，Windows 7 上视为没有可恢复的状态；静态导入会让 Win7 上的 Server 起不来。

**输入线程**（`src/system/ThreadPriority.*` + `InputQueue`）：`InputQueue::submit` 增加 `InputTaskPriority` 参数，默认 `Background`。`InputQueue::run` 执行任务时对 `Foreground` 任务套一个 `ScopedThreadPriorityBoost(THREAD_PRIORITY_ABOVE_NORMAL)`，离开作用域恢复进入时的值（不是硬编码 Normal）。

标记为 `Foreground` 的是用户正在等结果的路径：`SessionPump` 的连接、每包 `dispatch`、按键、按键直接引发的界面/焦点事件、送达确认，以及 `SessionController` 的点选、候选动作（置顶/删除/固定/清除）、翻页，和 Aux 管道里 DLL 只等 150 ms 的 `deactivate_terminal`。候选动作与 `deactivate_terminal` 的分级出处是上游 PR #1329（`d56e7d2f48`）：它把置顶、删除、固定、清除从 `UserAction` 挪进 `Foreground`，并把 `ClientDeactivated` 留在 `Foreground`，动机是候选操作与会话重建的 IPC 顺序。本仓库的输入队列本来就是先进先出，不需要那个顺序保证；这里保留 `Foreground` 只是为了提权，和上游的顺序动机无关。字典维护（quiesce/resume）、偏好发布、展开取完整列表、富化结果回填、模式读取保持 `Background`：提权是为了让用户的按键抢得过后台计算，不是让后台计算抢按键。

## Alternatives considered

- **只恢复进程优先级，不加线程提升**：改动最小，且进程从 Below normal 回到 Normal 已经解决主要问题。不采用：PR 1328 的验收标准是三条一起（进程、任务、Foreground 线程），而且输入队列线程与管道 I/O、UI 线程同属一个 Normal 进程，统一 Normal 时打字线程仍与它们平级；微软的调度指导也是「处理用户输入的线程用 Above normal」。
- **输入队列所有任务都提升**：不用给调用点分类，代码更少。不采用：同一个队列上还有字典维护（要打开/关闭词库）和偏好快照发布（要解析 JSON），把它们一起提到 Above normal 会在用户敲字时反而压住前台应用，这正是 PR 1328 明确避免的。
- **继续用任务 XML 往返**：实现更短，第一版就是这么写的。不采用：编码随控制台代码页走（见 Decision），中文用户名和中文计算机名会被破坏，而这条路径只在英文 CI 机器上试过。
- **直接用 COM 建任务，一次注册带全部设置**：能避开 schtasks 创建后再更新带来的第二次注册（安全软件可能拦两次）。不采用：它要把 `/SC ONLOGON`、`/IT`、`/RL LIMITED`、`/TR` 的语义用 COM 触发器/主体/动作重新写一遍，现有安装器测试和多年产线行为都建立在 schtasks 那条路径上；两次注册只在首次安装出现一次，且第二次失败不影响自启。
- **任务配置失败让安装失败**：PR 1328 的做法。不采用：那边创建任务本身就是硬失败；本仓库已经决定任务建不成不让安装中止（安全软件会拦），配置只是把非关键路径调好，不该因此让用户重装。
- **同时改电池策略**（`DisallowStartIfOnBatteries` / `StopIfGoingOnBatteries`）：PR 1328 的改动之一。不采用：那是笔记本上的出厂行为变化，属于单独的产品决定；本 PR 只修降级，等维护者定下来再单独改。
- **同时恢复进程的 I/O 优先级**：没有受支持的进程级 API，PR 1328 也放弃了；新安装的任务 Priority 4 会让 Watchdog 与后续拉起的进程从一开始就有正常 I/O 优先级。

## Consequences

收益：登录后由看门狗拉起的 Server 不再以 Below normal 运行，连续开机三天以上不会被任务调度器强杀；前台打字任务在输入队列里比同进程的后台任务高一级。

代价与未验证的部分：

- 旧安装升级时任务会被 `/F` 重建、再用 COM 更新一次；用户在任务计划程序里对这条任务做过的自定义设置会被覆盖（与升级更换可执行文件路径的既有行为一致）。装有安全软件的机器首次安装可能被询问两次。
- 任务配置要用一次 PowerShell 启动（和已有的 `StopProcessesUnder` 一样），被组策略禁止运行脚本的机器会走失败分支：安装继续，任务保持降级，日志有一行。
- 电池策略仍是任务调度器默认（电池上不启动、切换电池即停止），本次不动；要改是另一个决定。
- 真实打字 A/B 与高负载下的延迟收益没有测；「只在 Foreground 提升」由 `windows-input-queue` 的优先级用例和代码审查覆盖。
- 改动后的 `(Get-Process MetasequoiaImeServer).PriorityClass` 还没在本机装新包测：本机只装了旧版本，需要装本分支打出的包才能复测。改动前已实测为 Below normal（基础优先级 6）。
- Windows 7 上 `SetProcessInformation` 不存在，内存优先级那半项按成功处理；CPU 调度类恢复照常。

## Testing

- `windows-process-priority`（`tests/runtime/process_priority.cpp`）：先把进程降到 `BELOW_NORMAL_PRIORITY_CLASS` + `MEMORY_PRIORITY_LOW` 并读回确认，调用恢复逻辑后读回 Normal。内存优先级只在宿主真的两种值都写得进去时才验；Wine 那类只实现一半 API 的环境按不支持处理，不会因此判失败。环境不允许（job 对象、没有该 API）时以 77 跳过。本机 MSVC 实跑：`restore cpu_ok=1 cpu_error=0 memory_ok=1 memory_error=0 class=0x20 memory=5 lowered_memory=1`。
- `windows-thread-priority`（`tests/runtime/thread_priority.cpp`）：先从 Normal 起步证明这台主机接受 Above normal，再验守卫启用期间读回 Above normal、析构回到进入值；从 Below normal 进入再提升一次，证明恢复的是捕获值。本机实跑：`boost active=1 priority=1`。
- `windows-input-queue` 增加用例：后台任务看到的就是工作线程进入时的优先级，前台任务运行期间读回 Above normal，之后再跑一个后台任务确认已恢复；主机拒绝优先级时按跳过处理。
- `installer/tests/watchdog-task.ps1`：静态核对配置脚本走 COM、设置 `Priority=4` 与 `PT0S`、不碰电池、读回失败要 `exit 1`、失败后先查任务是否存在、只在创建成功后才配置；在提权的 Windows 会话里把安装器里的脚本原样抽出来，按两条真实调用路径各跑一遍——系统自带 `powershell.exe -Command "<脚本>"`（安装器用的就是这条，Win7 是 2.0，其它系统是 5.1）和 `pwsh -EncodedCommand`——每次都从默认 Priority 7 起步并用 `Get-ScheduledTask` 读回设置与主体，再用坏 `UserId` 让一次注册失败，确认任务和设置都还在。本机（非提权）通过了静态部分；抽脚本、内联 `-Command` 跑脚本、读回设置、失败保留、删任务后重建这五条流程都在探针任务上手跑过，结果一致（含中文任务名）。
- 没有自动化覆盖的路径：`ConfigureWatchdogLogonTask` 里「注册失败且任务已经消失，用同一份定义再注册一次重建」那一支。要触发它必须先让第一次注册失败、同时任务不在，安装器的正常输入做不到；本机手工验证过「删任务后用同一份定义 `RegisterTaskDefinition(6)` 能连同主体一起重建」，脚本这一支只有静态检查。
- `installer/tests/install-smoke.ps1` 增加安装后任务的 `Priority` 与执行时限核对；`windows7-install-smoke.ps1` 补同样的两项，外加 `Definition.Principal.LogonType`、`RunLevel` 不变（PS 2.0，走 COM；`IRegisteredTask` 本身没有 `Principal`，它在 `Definition` 下面）；`lifecycle.ps1` 的 `EnsureImeUserDataDir` 块结束标记跟着改成 `function CreateWatchdogLogonTask: Boolean;`。
- `src/input/InputQueue.cpp`、`src/ipc/SessionPump.cpp`、`src/ipc/SessionController.cpp` 用 MSVC `/W4 /EHsc` 单独编过；同环境下有 `WordCharacterPolicy.h` 触发的既有 C4244，与本改动无关，固定工具链上没有。
- 这些 CTest 用例不会在 CI 上执行：Windows GNU 交叉构建只链接不运行，MSVC 也不编主 CMake 工程。实跑靠维护者或本机。
