# Agent Note: x86 系统 WER 专项遇到系统侧停顿时重跑一次

Status: implemented

## Problem

`platforms/windows/tsf/tests/Invoke-SystemWer.ps1` 给每个用例 45 s 等目标进程由系统 WER 结束，超时就让 Native Platform CI 的 Windows TSF system WER 作业失败。2026-11-01 前后的 28 次 CI 里这一步失败 3 次（develop 推送 `41340814014`，PR `41331683904`、`41326203044`），加上更早的 `41327717560`，四次都是 x86，分别卡在 `tip`、`reopened` 和两次 `external`，x64 没有出现过。重跑同一个 job 就通过，失败与改动无关，却挡住了不相干的 PR 合并。

停顿有两种形态，共同点是目标进程没退出、恰好一个线程在等、其余线程全部 `Suspended`、没有 `.werpartial`，正向样本的记录已经完整落盘，负向样本一条记录都没有，helper 没有越权。

- A 形：等待线程停在 `LpcReply` 上等 WerSvc 的回复，系统里找不到为它工作的 `WerFault`。上面四次都是这一形态。
- B 形：等待线程停在 `UserRequest` 上，唯一相关的 worker 是目标自己拉起的 `WerFault.exe`（父进程就是目标），45 s 后仍然活着却不结束目标。首次重跑落地后才出现：PR #10617 的 `41371083017` 两次尝试（第一次 `host`、第二次 `reopened`，记录都已完整落盘）和 PR #10643 的 `41396949560`（负向样本 `withdrawn`）。`withdrawn` 的 helper 在看到未登记后立即返回，所以拖住 `WerFault` 的不是 helper，而是 helper 之后或之外的系统流程。

2026-11-01T23:43Z 首次重跑合入后的 66 个 Windows native settings job 里，A 形 3 次被重跑救回，B 形 2 次直接失败（不含上面第一次尝试），另有 1 次 A 形落在还没合入重跑的分支上，合计约 9%，全部是 x86。[TSF WER 采集](../feature/2026-11-01-windows-tsf-wer-capture.md)此前把这种停顿归因于负向样本请求排队，并让负向样本不再带 `WER_FAULT_REPORTING_FLAG_QUEUE`；上面几次失败都已包含那次修改（`d8d058445c`），`external` 照样停住，正向样本也会停，所以停顿和排队标志无关，是 32 位进程的系统 WER 流程偶发地不把挂起的目标结束。产品侧回调 `OutOfProcessExceptionEventCallback` 只做有界的 `ReadProcessMemory` 和文件写入，不等待，且总把 `*ownership` 置为 `FALSE`。

## Decision

超时后先保存 `timeout.json`：目标线程的状态和等待原因，以及每个相关 `WerFault` / `WerFaultSecure`（最多 16 个）的 id、名字、父进程、线程状态和等待原因（各最多 128 个线程）、是否已退出；不采集内存、环境或命令行。再用 `Test-SystemWerStall` 判断是不是上面两种停顿：目标未退出；线程和 WER worker 两份快照都取到了；所有线程都在等，恰好一个不是 `Suspended`；没有半成品记录；完整记录数不超过本用例允许的上限（正向 1 条，负向 0 条）；并且满足以下其一：

- A 形：没有相关 worker，等待线程的原因是 `LpcReply`。
- B 形：恰好一个相关 worker，名字是 `WerFault.exe`（不是 `WerFaultSecure.exe`），父进程是目标，等待线程的原因是 `LpcReply` 或 `UserRequest`。

只有符合这两种形态的第一次超时会重跑：先按名字复核后结束快照里的 `WerFault`，再结束目标，两者都等到退出，保存目标已读到的输出，在新目录 `<用例>-retry` 里把整个用例重跑一次。重跑仍须由系统在 45 s 内结束目标，并通过原有的全部断言，包括退出码、记录唯一性和内容、负向样本无记录、无半成品。第一次的诊断留在原目录，随 `windows-tsf-system-wer` artifact 上传，日志里有一条 warning 指向它并写明是哪种形态。

其他任何超时（目标已退出却超时、有线程在运行、等待原因不是这三种、多于一个线程在等、A 形等待原因不是 `LpcReply`、worker 是 `WerFaultSecure` 或不止一个或不是目标的子进程、快照失败、负向样本出现记录、正向样本记录多于一条、缺 `expected.json`）和第二次超时都照旧直接失败。等待期限不变，记录落盘也仍不被当成系统退出。

## Alternatives considered

- **把 45 s 期限加长**：最省事，但没有停顿的用例正向约 0.3 s、负向约 30 s（稳定窗口）就结束；A 形里目标被挂起且没有 worker 在处理，B 形里 `WerFault` 也卡了 45 s 没有进展，再等多久都不一定会结束。原脚本也明确不延长期限。不采用。
- **超时后只要记录完整就算通过并自己结束进程**：能盖住正向样本的停顿，但 `external`、`withdrawn` 停顿时本来就不该有记录，这条规则判不了负向样本；而且会把「记录落盘」当成「系统处置完成」，放弃退出码断言。不采用。
- **测试期间在当前用户下关闭 WER 排队或界面再恢复**：可能消除停顿，但要改 WER 设置。原脚本只登记 helper、只启动 WerSvc，有意不改策略；负向样本已经去掉排队标志，照样会停，说明这条路未必有效。不采用。
- **给整个 job 加自动重试**：会把真实的 helper 回归也一起重跑掉，还要重编整个 job。不采用。
- **B 形只结束目标、不管 `WerFault`**：`WerFault` 持有目标句柄，目标被结束后它可能继续跑到重跑期间；先结束它再结束目标，两次尝试互不干扰。

## Consequences

- 连续重试仍超时的后续调查见[不加载产品模块的系统对照](2026-11-04-windows-system-wer-baseline.md)。基线用于取得因果证据，不替代根因修复或放宽这里的失败条件。

- 单个用例偶发停顿不再挡 PR；同一用例连续两次停顿，或出现任何不符合上述形态的超时，仍然失败。代价是：一个只是偶发让目标卡住的真实回归，可能被这一次重跑放过。不过这种回归也得表现成「挂起、单线程等待、记录合规」，B 形还要求卡住的是目标自己拉起的 `WerFault`，才会被重跑；每次重跑都会在日志留 warning，诊断也随 artifact 保留。
- B 形的根因还没定论。`timeout.json` 现在带 `WerFault` 的线程等待原因，下一次 B 形停顿可以据此区分 `WerFault` 在等 WerSvc（WOW64 / 系统侧）还是在等别的；若重跑后同一形态再次停住，先看这份诊断再决定下一步。
- 重跑分支依赖 pwsh 7 的 `Process.Kill()` 在进程已退出时不抛异常：结束 `WerFault` 后目标可能先自行退出。CI 这一步固定用 `shell: pwsh`；Windows PowerShell 5.1 下这个竞态会抛 `MethodInvocationException`。
- 在 win-runner 上用 pwsh 7.5.1 和 Windows PowerShell 5.1 验证过：`41371083017`、`41396949560` 两份 CI 诊断判为 B 形停顿，A 形诊断仍判为停顿；`UserRequest` 但无 worker、`WerFaultSecure`、worker 不是目标的子进程、两个 worker、两个线程在等、有线程在运行、其他等待原因、全部挂起、半成品记录、负向样本有记录、正向样本两条记录、目标已退出、两类快照失败、缺 `expected.json` 都判为不重跑。用改名为 `WerFault.exe` 的子进程验证过 `Save-SystemWerTimeout` 能记下 worker 的线程数组和退出状态，以及重跑分支的结束顺序能让两者都退出。
- 带重跑的完整脚本需要在 PR 的 Windows TSF system WER job 里运行，本机和 win-runner 上都没有跑过真实的系统 WER 投递，也没有复现过真实的 B 形停顿。

2026-11-05 的对照把两种停顿都定位到 Windows 自带的 32 位 `SysWOW64\WerFault.exe` 自身堆损坏或在堆锁上死锁，最小空 helper 同样复现；x86 系统投递先改为非阻塞观测（[x86 系统投递非阻塞](../../archived/testing/2026-11-05-windows-system-wer-x86-nonblocking.md)），随后产品在 WOW64 宿主里不再登记 helper，x86 只跑不登记的 `baseline` 和 `wow64` 并恢复阻塞，见[WOW64 宿主不做进程外采集](../feature/2026-11-06-windows-tsf-wer-skip-wow64.md)。本笔记的重试规则仍在脚本里，对所有用例照常生效。

系统专项与设置作业的运行归属见[独立 WER 作业](2026-11-04-windows-wer-independent-ci.md)。#10844 与 #10755 的新诊断证明同一用例重试后仍可能停顿；这类失败继续保留，独立作业不修改本笔记的重试条件或成功标准。
