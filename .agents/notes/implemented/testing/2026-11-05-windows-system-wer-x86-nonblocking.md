# Agent Note: x86 系统 WER 投递改为非阻塞观测，只有 x64 系统投递阻塞

Status: implemented

## Problem

`Windows TSF system WER` 作业里 x86 的真实系统投递反复超时（#10865），[一次受限重试](2026-11-02-windows-system-wer-x86-stall-retry.md)、[拆成独立作业](2026-11-04-windows-wer-independent-ci.md)和[不加载产品模块的基线](2026-11-04-windows-system-wer-baseline.md)都没有消除它，PR #10710、#10844、#10755 等因此变红，x64 从未停过。

2026-11-05 的析因对照（运行 41940490887、41945191887、41950498292，每台 runner 每种模式 150 次，停顿时用 cdb 非侵入抓全部线程栈并导出 Application 事件日志）把直接原因定位到 Windows 自带的 32 位 `C:\Windows\SysWOW64\WerFault.exe`：

- 只有「WER 真正加载了允许列表里的 helper」才停。`baseline` 0 / 900、只加载产品模块和采集线程但不登记 0 / 450、登记但不在允许列表 0 / 450；只登记真实 helper 230 / 900、CI 的 `external` 37 / 450；三个导出只写 `*ownership = FALSE`、没有 CRT 和 DllMain 的最小 helper 23 / 600，所以与 helper 代码无关。
- 换 WER 标志（不设、只设 `NO_UI`、只设 `NOHEAP`）和换系统（Server 2025 26100.33674、Server 2022 20348.5761）都不消除；停顿率按 runner 实例差异很大，有的实例 0 / 600，有一台 150 / 150，所以现有一次重试救不回来。
- A 形（`LpcReply`、无 worker）：目标主线程停在 `KERNELBASE!UnhandledExceptionFilter -> ntdll!NtRaiseHardError(0xD0000144)`，即 WER 失败后的经典错误框，非交互会话没人应答；同时 Application 日志记录 `SysWOW64\WerFault.exe` 在 `ntdll.dll` 以 `0xc0000374`（`STATUS_HEAP_CORRUPTION`）崩溃。
- B 形（`UserRequest`、目标拉起的 WerFault 不退出）：WerFault 主线程停在 `faultrep!CCrashReport::GenerateCrashReport -> WerPluginsInitialize -> CIptPlugin::Initialize -> LoadLibraryExW -> LdrpDrainWorkQueue`，加载器工作线程停在 `RtlAllocateHeap -> RtlEnterCriticalSection`，等一把没人释放的进程堆锁；整条栈没有产品模块。WerSvc 宿主在两种形态下都空闲。

产品代码修不了系统 WerFault 的堆损坏；按 #10865 的约定也不能再加重试、延长期限或跳过用例。继续让 x86 系统投递阻塞，等于让一个与本次改动无关、按 runner 实例随机触发的系统缺陷决定 PR 是否变红。

## Decision

houko 于 2026-11-05 在 #10865 上选定方向 1：x86 系统投递保留为非阻塞观测，只有 x64 系统投递阻塞。

`ci-platforms.yml` 的 `Windows TSF system WER` 作业拆成四步，作业名不变：

- `Build and test TSF WER callback and lifecycle (x64 and x86)`：两种架构的配置、构建、callback 和 lifecycle CTest，任何一项失败都令作业失败。
- `Run TSF system WER delivery (x64)`：原样调用 `Invoke-SystemWer.ps1`，失败令作业失败。
- `Run TSF system WER delivery (x86, non-blocking)`：原样调用同一脚本，`continue-on-error: true`。脚本本身不变：超时、稳定窗口、唯一一次受限重试、退出码、记录唯一性与内容、负向样本无记录、无半成品等断言都照旧执行，只是失败不让作业变红。
- `Report non-blocking x86 system WER failure`：仅在上一步失败时运行，输出一条 `::warning` 注解并写入作业摘要，列出留有 `timeout.json` 的用例目录；没有超时目录时明确写「不是超时」，提示那类失败不属于这个已知缺陷。

`windows-tsf-system-wer` artifact 照旧在 `always()` 下上传两种架构的 `system-wer/` 和 `wer-tests.xml`。x64 系统投递失败时 x86 系统投递不再运行，与拆分前循环先跑 x64 的行为一致。这个作业从来不在 develop 规则集的必需检查里（必需检查是 macOS、iOS、quality 和 Android host），所以不涉及必需检查改名；它在 PR 页上的红绿现在只反映 x64 系统投递和两种架构的构建与 CTest。

## 开放事项

- 产品影响尚未决定：helper 登记在每个加载 x86 TIP 的 32 位第三方进程里，这些进程崩溃时会按同样的概率改走有缺陷的 32 位 WerFault，坏的时候用户看到经典错误框（A 形），或崩溃的应用一直挂起不退出（B 形，WerFault 死锁且没有超时）。若代价不可接受，可以在 WOW64 进程里不登记 helper，代价是失去 x86 TSF 的进程外采集。本笔记不改产品代码，#10865 因此保持开放。
- 若微软修复了 SysWOW64 WerFault，或产品决定在 WOW64 进程里不登记 helper，应重新评估是否恢复 x86 系统投递的阻塞。

## Alternatives considered

- **维持阻塞，但把「目标停在 `NtRaiseHardError`，或 32 位 WerFault 停在 faultrep 插件初始化、事件日志有它的 `0xC0000374`」认定为已知系统缺陷豁免（#10865 方向 2）**：判据比现在按等待原因猜更准，非超时的 x86 回归仍会阻塞；但要在 CI 里携带 cdb 抓栈和事件日志解析，本质仍是对系统缺陷的豁免，判据一旦漏判就继续误报。未选。
- **x86 系统投递整个删掉，只留 callback / lifecycle CTest**：最干净；但会失去唯一经过真实 WOW64 WER 路径的观测，而这条路径正是开放的产品影响所在。未选。
- **再加重试或延长期限**：#10865 明确禁止；对照里有实例 150 / 150 停顿，重试也无效。不采用。
- **把作业改成 job 级 `continue-on-error`**：job 级设置下检查列表里该作业仍显示失败，且 x64 也会一起变成非阻塞。不采用。

## Consequences

- 收益：与改动无关的 x86 系统缺陷不再让 PR 变红；x64 系统投递和两种架构的 CTest 照旧阻塞；x86 每次仍完整跑一遍并留下诊断，摘要和注解让失败可见。
- 代价：x86 系统投递里的真实产品回归（包括非超时的断言失败）也只会以 warning 出现，不再自动挡住合并；看到摘要里写「不是超时」时必须当作真实失败去查。x86 真实 WER 投递在托管 runner 上不再有确定性的门禁。
- 历史笔记审计：[停顿重试](2026-11-02-windows-system-wer-x86-stall-retry.md)的重试规则仍在脚本里，部分重叠，互链；[独立 WER 作业](2026-11-04-windows-wer-independent-ci.md)的作业拆分仍成立，其中「任意一组失败都会显示」对 x86 系统投递改为以 warning 显示，部分重叠，互链；[基线](2026-11-04-windows-system-wer-baseline.md)提供了本次对照的出发点，部分重叠，互链；[TSF WER 采集](../feature/2026-11-01-windows-tsf-wer-capture.md)的产品边界不变，无关。

## Verification

- 本机 `actionlint .github/workflows/ci-platforms.yml` 通过。
- 报告步骤的 PowerShell 用 pwsh 在本机构造的假输出目录上运行：有 `timeout.json` 时列出 `reopened, reopened-retry`，没有输出目录时写「不是超时」，两次都退出 0。
- 真实 Windows 行为以本 PR 的 `Windows TSF system WER` 作业为准。
