# Agent Note: WOW64 宿主不做 TSF 进程外 WER 采集，x86 系统投递恢复阻塞

Status: implemented

## Problem

[TSF 进程外 WER 采集](2026-11-01-windows-tsf-wer-capture.md)在每个加载 TIP、且用户开启了崩溃上报的宿主进程里调用 `WerRegisterRuntimeExceptionModule` 登记 `MetasequoiaImeCrashWer.dll`。x86 TIP 加载在 32 位第三方应用里，这些进程在 x64 和 ARM64 Windows 上都跑在 WOW64 下。

#10865 的析因对照（2026-11-05，运行 41940490887、41945191887、41950498292，每台 runner 每种模式 150 次，停顿时用 cdb 非侵入抓全部线程栈并导出 Application 事件日志）把 CI 上 x86 系统投递反复超时的直接原因定位到 Windows 自带的 `C:\Windows\SysWOW64\WerFault.exe`：

- 只有「WER 真正加载了允许列表里的 helper」才停。不加载任何产品模块的 `baseline` 0 / 900；加载产品模块、跑采集线程但不登记 0 / 450；登记但不在允许列表 0 / 450。只登记真实 helper 230 / 900，CI 的 `external` 37 / 450；三个导出只写 `*ownership = FALSE`、没有 CRT 和 DllMain 的最小 helper 23 / 600，所以与 helper 代码无关。
- 换 WER 标志（不设、只设 `NO_UI`、只设 `NOHEAP`）和换系统（Server 2025 26100.33674、Server 2022 20348.5761）都不消除；停顿率按机器实例差异很大，有的 0 / 600，有一台 150 / 150。x64 目标不走这条路径，从未停过。
- A 形：目标主线程停在 `KERNELBASE!UnhandledExceptionFilter -> ntdll!NtRaiseHardError(0xD0000144)`，即 WER 失败后的经典「应用程序错误」框；同时 Application 日志记录 `SysWOW64\WerFault.exe` 在 `ntdll.dll` 以 `0xc0000374`（`STATUS_HEAP_CORRUPTION`）崩溃。
- B 形：目标停在 `KERNEL32!WerpReportFaultInternal` 等它自己拉起的 WerFault；WerFault 主线程停在 `faultrep!CCrashReport::GenerateCrashReport -> WerPluginsInitialize -> CIptPlugin::Initialize -> LoadLibraryExW -> LdrpDrainWorkQueue`，加载器工作线程停在 `RtlAllocateHeap -> RtlEnterCriticalSection`，等一把没人释放的进程堆锁。整条栈没有产品模块，WerSvc 宿主空闲，也没有超时。

CI 上这只是红灯；落到用户那里，就是开启崩溃上报的用户在 32 位应用崩溃时，按同样的概率看到一个本不该出现的错误框，或者崩溃的应用永远挂着不退出、只能去任务管理器结束。产品代码修不了系统 WerFault 的堆损坏。

## Decision

`platforms/windows/tsf/TsfCrashCapture.cpp` 的 `capture_supported_host()` 在令牌检查之前先调 `wow64_process()`：`IsWow64Process(GetCurrentProcess())` 为 TRUE 时判为不支持，查询失败也按 WOW64 处理。结果与 AppContainer、低完整性判定放在同一个原子缓存里，于是 WOW64 宿主的 `acquire()` 直接返回无租约：不建观察者、不读授权、不调 Host API 准备记录、不调 `WerRegisterRuntimeExceptionModule`，DLL 卸载也不等后台任务。

- `IsWow64Process` 对 x64 上的 x86 进程和 ARM64 上的 x86 进程都返回 TRUE，对原生 64 位进程（包括 ARM64 上模拟运行的 x64）和 32 位原生 Windows 返回 FALSE。它从 XP SP2 起就在 kernel32 里，Win7 TIP 可以直接导入，不需要 `GetProcAddress` 取 `IsWow64Process2`。
- 32 位原生 Windows 不是 WOW64，照常登记采集。对照只在 64 位 Windows 上做过，停顿出在 WOW64 目标改由 `SysWOW64\WerFault.exe` 处理的那条路径上；32 位原生 Windows 没有做过对照，Windows 11 也已没有 32 位版本。x64、ARM64 TIP 不受影响。
- 安装器照旧为 x86 helper 写允许列表值。只放进允许列表、不登记的配置在对照里是 0 / 450，32 位原生 Windows 仍要用它，所以不动。

测试边界与 `msime_test_wer_register` 同一个模式：`MSIME_TSF_CRASH_CAPTURE_TEST` 下，生产查询把系统的真实结果交给夹具的 `msime_test_wow64_process(actual)`，夹具按 `MSIME_TSF_TEST_WOW64`（`1` 假装 WOW64，`0` 假装不是，未设置照用真实结果）改写，并记下真实结果供用例核对。

- `tsf_capture_lifecycle.cpp` 先把它设成 `0`，原有采集用例在 x86 runner 上照旧覆盖；随后假装 WOW64，断言整轮没有快照、没有登记、没有授权读取和准备，且 DLL 立即卸载；再取消改写，断言夹具收到的真实结果与 `IsWow64Process` 一致，并按真实结果断言 x86 不武装、x64 武装并登记一次。
- 合成 Host API（`tsf_capture_host.cpp`）原来在清理请求到达时按当前授权判违约，而生产代码读到「已关闭」先解除武装、再请求清理；生命周期用例一看到解除武装就重新开启授权，清理晚一步到达就被误记一次违约。本 PR 的 x86 生命周期失败过一次（作业 127682113843），失败信息只有聚合计数：那次运行只用了 6.4 秒，10 秒的屏障超时不可能发生，begin 请求字段是确定的，剩下的只有这条清理竞态和写授权标记文件失败两种，前者是已知的时序窗口。现在按最近一次授权读取判断，这才是生产代码要守的契约；违约计数同时记下第一次违约的原因，再失败时信息里直接写出是哪一类。
- `tsf_capture_system.cpp` 新增 `wow64` 用例：在真实 WOW64 进程里不改写查询，确认 TIP 不登记后触发 TIP 自有故障，要求系统以 `0xC0000005` 结束进程、不留记录。`Invoke-SystemWer.ps1` 加 `-Cases` 参数，默认仍是原来的八个用例。

`ci-platforms.yml` 的 `Windows TSF system WER` 作业里，x86 系统投递改名为 `Run TSF system WER delivery (x86, WOW64 exclusion)`，只跑 `-Cases baseline, wow64`，去掉 `continue-on-error` 和报告步骤，恢复阻塞。x64 仍跑默认八个用例并阻塞。x86 测试进程本身就是 WOW64，生产判定下其余用例不会武装，所以不再在 x86 上跑；不登记 helper 时碰不到那个系统缺陷，这两个用例可以确定地阻塞。

本条取代 [x86 系统投递非阻塞](../../archived/testing/2026-11-05-windows-system-wer-x86-nonblocking.md)：那篇在产品是否排除 WOW64 未决时，把 x86 系统投递降成只报告不阻塞，开放事项里写明产品排除 WOW64 后应重新评估阻塞。

## Alternatives considered

- **保持登记，只在 CI 里把 x86 系统投递降为非阻塞观测（#10920 的做法）**：不动产品代码，x86 TSF 进程外采集照常；但用户侧的错误框和永久挂起照旧发生，CI 的 x86 红绿也不再有意义。输入法不能把第三方应用的崩溃变成挂起，被取代。
- **维持阻塞，把「目标停在 `NtRaiseHardError`，或 32 位 WerFault 停在 faultrep 插件初始化、事件日志有它的 `0xC0000374`」认定为已知系统缺陷豁免（#10865 方向 2）**：判据比按等待原因猜更准；但要在 CI 里带 cdb 抓栈和事件日志解析，本质仍是对系统缺陷的豁免，用户侧问题也没解决。不采用。
- **在 WOW64 里登记，但给 WerFault 加看门狗或改 WER 标志**：挂起发生在系统 WerFault 进程里，TIP 无法在宿主崩溃后再运行代码去结束它；对照已证明换标志不消除。不可行。
- **用 `GetProcAddress` 取 `IsWow64Process2` 并按 `processMachine` 判定，取不到再退回 `IsWow64Process`**：能顺带知道原生架构，但这里只需要「是不是 WOW64」，`IsWow64Process` 在所有支持的系统和 ARM64 上都给出同样的答案，多一条分支只多一处要测的代码。不采用。
- **在 x86 上保留直接登记的观测用例（非阻塞），用来发现微软何时修好**：每次 PR 多跑几分钟、带一条已知会随机失败的警告，换来的信号很少有人看。要复查时设 `MSIME_TSF_TEST_WOW64=0` 跑默认用例即可。不采用。
- **再加重试或延长期限**：#10865 明确禁止；有实例 150 / 150 停顿，重试也无效。不采用。

## Consequences

- 收益：开启崩溃上报的用户在 32 位应用崩溃时，系统按没有输入法时的方式处理，不再因为 TIP 登记的 helper 弹出错误框或挂起。x86 系统投递恢复为确定的阻塞门禁，`Windows TSF system WER` 作业的红绿重新代表两种架构。
- 代价：x64 和 ARM64 Windows 上的 32 位宿主没有 TSF 进程外崩溃采集，那里的 TIP 崩溃不会进崩溃记录。32 位宿主是少数，采集本来就要用户主动开启；需要 32 位宿主的崩溃证据时，只能靠用户提供的系统 WER 报告。这不是出厂默认值的变化：遥测开关和它的默认值都没变。
- 若微软修好 SysWOW64 WerFault，可以去掉 `wow64_process()` 并恢复 x86 的完整用例；判断依据是在 x86 上设 `MSIME_TSF_TEST_WOW64=0` 跑默认用例和 #10865 的析因对照。
- 历史笔记审计：[TSF WER 采集](2026-11-01-windows-tsf-wer-capture.md)的产品边界多了一类不采集的宿主，部分重叠，互链；[x86 系统投递非阻塞](../../archived/testing/2026-11-05-windows-system-wer-x86-nonblocking.md)被本条完全取代，证据、备选和后果已并入本篇，归档；[停顿重试](../testing/2026-11-02-windows-system-wer-x86-stall-retry.md)的一次重试仍在脚本里、对 x64 和 x86 的两个用例照常生效，部分重叠，互链；[独立 WER 作业](../testing/2026-11-04-windows-wer-independent-ci.md)的作业拆分仍成立，部分重叠，互链；[基线](../testing/2026-11-04-windows-system-wer-baseline.md)的 `baseline` 用例在 x86 上继续阻塞运行，部分重叠，互链。

## Verification

- 本机 x86_64 与 i686 MinGW 对 `TsfCrashCapture.cpp`（生产与测试两种宏）、夹具、生命周期和系统用例做 `-Wall -Wextra -Werror -fsyntax-only`，全部通过。
- 先只推测试、CI 和脚本改动、不带产品判定，PR 上的 `Windows TSF system WER` 作业应在新加的 WOW64 用例失败；再加产品判定，作业应全部通过。两次运行的链接写在 PR 正文里。
- `pnpm run verify-notes` 通过。
