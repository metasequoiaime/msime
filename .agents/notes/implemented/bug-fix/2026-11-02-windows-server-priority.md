# Agent Note: Windows 登录任务把 Server 降级的问题在本仓库的 Rust／共享内核版本里同样存在，按 msime-windows 的做法补齐

Status: implemented

## Problem

msime-windows 的 PR 1328（`fix(server): 恢复交互优先级并修正登录任务降级`，2026-10-30 合并）修的是这条链：安装器用 `schtasks /Create` 建登录任务 `Metasequoia IME Watchdog`，没有设置 Priority，任务调度器默认 7（Below normal）。按 Windows 文档，Priority 7 同时把进程降到 `BELOW_NORMAL_PRIORITY_CLASS`、`MEMORY_PRIORITY_LOW` 和低 I/O 优先级；Watchdog 继承它，`CreateProcess` 拉起的 Server 又继承调度类。任务还带着默认的 `DisallowStartIfOnBatteries=true`、`StopIfGoingOnBatteries=true` 和 72 小时执行时限。他们在 issue #927（开机阶段输入法不可用）和 #1239（高负载下卡顿）里定位到这一点。

本仓库（msime）的 Windows 产品是同一套进程结构：`platforms/windows/installer/msime_setup.iss` 用同样的 `schtasks /Create /SC ONLOGON /RL LIMITED /IT` 建任务，Server 与 Watchdog 仍是原生 C++ 宿主，打字路径最终落在共享的 Rust 宿主 DLL（`crates/host-api`、`crates/engine`）上。改动前全仓没有一处 `SetPriorityClass`、`SetThreadPriority` 或 `SetProcessInformation`，登录任务的三个默认设置也原样留着。在本机读安装好的任务可以直接看到差异：旧产品 `Metasequoia IME Watchdog` 已经被 PR 1328 改成 `Priority=4`、电池上照常运行、无执行时限，本仓库的 `Metasequoia IME Watchdog (Full)` 还是 `DisallowStartIfOnBatteries=true`、`StopIfGoingOnBatteries=true`，没有 `Priority` 和 `ExecutionTimeLimit` 节点（即默认 7 和 72 小时）。

所以问题不是「Rust 版有没有」，而是同一份安装器和同一棵进程树本来就有；Rust 那一半不自建线程池，按键查询跑在 C++ 的输入队列线程上，进程被降级时它跟着降级。

## Decision

按 PR 1328 的三段处理，落到本仓库自己的结构上。

**安装器**（`msime_setup.iss`）：`CreateWatchdogLogonTask` 改成返回 `Boolean`，创建成功后再跑新的 `ConfigureWatchdogLogonTask`：用 `schtasks /Query /XML` 导出任务，把 `Priority` 设为 4、`DisallowStartIfOnBatteries` 与 `StopIfGoingOnBatteries` 设为 false、`ExecutionTimeLimit` 设为 `PT0S`，再用同一份 XML `schtasks /Create /F` 重建并读回校验。

- 用 XML 而不是 `Get-ScheduledTask`/`Set-ScheduledTask`：后者要 ScheduledTasks 模块，Windows 7 随系统的是 PowerShell 2.0，没有这个模块，而 Win7 安装包与本安装器是同一份脚本（`/DWindows7Architecture`）。
- 导出经 `cmd.exe /c ... > file` 转发，`/Query` 的 UTF-16 输出原样落盘；用 PowerShell 自己的 `>` 在 PS 2.0 上是 Unicode、在 7 上是 UTF-8，两端行为不一致。
- 任务名里有空格和括号（`Metasequoia IME Watchdog (Full)`），脚本里用 `[char]34` 拼引号，不在 Inno 的 `-Command "..."` 参数里嵌双引号。
- 配置失败不中断安装，只写日志并提示：与本仓库「计划任务建不成本身不让安装失败」的既有决定一致（360、火绒会拦），任务已经能开机启动，只是还带着降级设置。创建本身失败时连配置也不跑，避免对着不存在的任务再报一次错。

**进程**（`src/system/ProcessPriority.*`）：`wmain` 最前面、任何业务线程创建之前调用 `restore_interactive_process_priority()`，恢复 `NORMAL_PRIORITY_CLASS` 和 `MEMORY_PRIORITY_NORMAL`。两项独立尝试，失败只把错误码写进 `server.log`，不阻止启动。`SetPriorityClass` 不恢复内存优先级，必须分开设置。`SetProcessInformation` 从 Windows 8 才有，经 `GetProcAddress` 取，Windows 7 上视为没有可恢复的状态；静态导入会让 Win7 上的 Server 起不来。

**输入线程**（`src/system/ThreadPriority.*` + `InputQueue`）：`InputQueue::submit` 增加 `InputTaskPriority` 参数，默认 `Background`。`InputQueue::run` 执行任务时对 `Foreground` 任务套一个 `ScopedThreadPriorityBoost(THREAD_PRIORITY_ABOVE_NORMAL)`，离开作用域恢复进入时的值（不是硬编码 Normal）。

标记为 `Foreground` 的是用户正在等结果的路径：`SessionPump` 的连接、每包 `dispatch`、按键、按键直接引发的界面/焦点事件、送达确认，以及 `SessionController` 的点选、候选动作、翻页。字典维护（quiesce/resume）、偏好发布、展开取完整列表、富化结果回填、模式读取保持 `Background`，理由与 PR 1328 一致：提权是为了让用户的按键抢得过后台计算，不是让后台计算抢按键。

## Alternatives considered

- **只恢复进程优先级，不加线程提升**：改动最小，且进程从 Below normal 回到 Normal 已经解决主要问题。不采用：PR 1328 的验收标准是三条一起（进程、任务、Foreground 线程），而且输入队列线程与管道 I/O、UI 线程同属一个 Normal 进程，统一 Normal 时打字线程仍与它们平级；微软的调度指导也是「处理用户输入的线程用 Above normal」。
- **输入队列所有任务都提升**：不用给调用点分类，代码更少。不采用：同一个队列上还有字典维护（要打开/关闭词库）和偏好快照发布（要解析 JSON），把它们一起提到 Above normal 会在用户敲字时反而压住前台应用，这正是 PR 1328 明确避免的。
- **用 `Get-ScheduledTask`/`Set-ScheduledTask` 配置任务**：更短、更「官方」，旧产品在有模块的系统上就是这么做的。不采用：Windows 7 的 PowerShell 2.0 没有该模块，而安装器一份两用；XML 往返在本机（Win11）实测把三项都写进去并读回，`LogonType=InteractiveToken`（`/IT`）和 `RunLevel=Limited` 保持原样。
- **任务配置失败让安装失败**：PR 1328 的做法。不采用：那边创建任务本身就是硬失败；本仓库已经决定任务建不成不让安装中止（安全软件会拦），配置只是把非关键路径调好，不该因此让用户重装。
- **同时恢复进程的 I/O 优先级**：没有受支持的进程级 API，PR 1328 也放弃了；新安装的任务 Priority 4 会让 Watchdog 与后续拉起的进程从一开始就有正常 I/O 优先级。

## Consequences

收益：登录后由看门狗拉起的 Server 不再以 Below normal + 低内存优先级运行，电池上不会被任务调度器拦下或掐断，连续开机三天以上不会被强杀；前台打字任务在输入队列里比同进程的后台任务高一级。

代价与未验证的部分：

- 旧安装升级时任务会被 `/F` 重建再配置，用户在任务计划程序里对这条任务做过的自定义设置会被覆盖（与升级更换可执行文件路径的既有行为一致）。
- 任务配置要用一次 PowerShell 启动（和已有的 `StopProcessesUnder` 一样），被组策略禁止运行脚本的机器会走失败分支：安装继续，任务保持降级，日志有一行。
- 真实打字 A/B 与高负载下的延迟收益没有测；「只在 Foreground 提升」由 `windows-input-queue` 的优先级用例和代码审查覆盖。
- Windows 7 上 `SetProcessInformation` 不存在，内存优先级那半项按成功处理；CPU 调度类恢复照常。

## Testing

- `windows-process-priority`（`tests/runtime/process_priority.cpp`）：把当前进程降到 `BELOW_NORMAL_PRIORITY_CLASS` + `MEMORY_PRIORITY_LOW` 并读回确认，调用恢复逻辑后读回 Normal；环境不允许（job 对象等）时以 77 跳过。本机 MSVC 实跑：`restore cpu_ok=1 cpu_error=0 memory_ok=1 memory_error=0 class=0x20 memory=5`。
- `windows-thread-priority`（`tests/runtime/thread_priority.cpp`）：先从 Normal 起步证明这台主机接受 Above normal，再验守卫启用期间读回 Above normal、析构回到进入值；从 Below normal 进入再提升一次，证明恢复的是捕获值。本机实跑：`boost active=1 priority=1`。
- `windows-input-queue` 增加用例：后台任务看到的就是工作线程进入时的优先级，前台任务运行期间读回 Above normal，之后再跑一个后台任务确认已恢复；主机拒绝优先级时按跳过处理。
- `installer/tests/watchdog-task.ps1`：静态核对配置脚本设置四项、按 XML 重建、读回失败要 `exit 1`、只在创建成功后才配置；在提权的 Windows 会话里把安装器里的脚本原样抽出来跑一遍，再用 `Get-ScheduledTask` 读回 `Priority=4`、`ExecutionTimeLimit=PT0S`、两项电池设置。本机（非提权）通过了静态部分；抽脚本、跑脚本、读回三项的流程用同一段代码在探针任务上手跑过，结果一致。
- `installer/tests/install-smoke.ps1` 增加安装后任务的 `Priority`、电池与执行时限核对；`lifecycle.ps1` 的 `EnsureImeUserDataDir` 块结束标记跟着改成 `function CreateWatchdogLogonTask: Boolean;`。
- `src/input/InputQueue.cpp`、`src/ipc/SessionPump.cpp`、`src/ipc/SessionController.cpp` 用 MSVC `/W4 /EHsc` 单独编过；同环境下有 `WordCharacterPolicy.h` 触发的既有 C4244，与本改动无关，固定工具链上没有。
