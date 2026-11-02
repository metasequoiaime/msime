# Agent Note: x86 系统 WER 专项遇到系统侧停顿时重跑一次

Status: implemented

## Problem

`platforms/windows/tsf/tests/Invoke-SystemWer.ps1` 给每个用例 45 s 等目标进程由系统 WER 结束，超时就让 Native Platform CI 的 Windows native settings 失败。2026-11-01 前后的 28 次 CI 里这一步失败 3 次（develop 推送 `41340814014`，PR `41331683904`、`41326203044`），加上更早的 `41327717560`，四次都是 x86，分别卡在 `tip`、`reopened` 和两次 `external`，x64 没有出现过。重跑同一个 job 就通过，失败与改动无关，却挡住了不相干的 PR 合并。

四份 `timeout.json` 形态相同：目标进程没退出，恰好一个线程在 `LpcReply` 上等 WerSvc 的回复，其余线程全部 `Suspended`，系统里找不到为它工作的 `WerFault`，也没有 `.werpartial`。正向样本的记录已经完整落盘，负向样本 `external` 一条记录都没有，helper 没有越权。[TSF WER 采集](../feature/2026-11-01-windows-tsf-wer-capture.md)此前把这种停顿归因于负向样本请求排队，并让负向样本不再带 `WER_FAULT_REPORTING_FLAG_QUEUE`；上面几次失败都已包含那次修改（`d8d058445c`），`external` 照样停住，正向样本也会停，所以停顿和排队标志无关，是 32 位进程的系统 WER 流程偶发地不把挂起的目标结束。

## Decision

超时后先按原样保存 `timeout.json`，再用 `Test-SystemWerStall` 判断是不是上面那种系统侧停顿：目标未退出；线程和 WER worker 两份快照都取到了；恰好一个线程等 `LpcReply`，其余线程都是 `Suspended`；没有相关的 `WerFault` / `WerFaultSecure`；没有半成品记录；完整记录数不超过本用例允许的上限（正向 1 条，负向 0 条）。

只有符合这个形态的第一次超时会重跑：结束卡住的目标进程，保存它已读到的输出，在新目录 `<用例>-retry` 里把整个用例重跑一次。重跑仍须由系统在 45 s 内结束目标，并通过原有的全部断言，包括退出码、记录唯一性和内容、负向样本无记录、无半成品。第一次的诊断留在原目录，随 `windows-tsf-system-wer` artifact 上传，日志里有一条 warning 指向它。

其他任何超时（目标已退出却超时、有线程在运行、WER worker 还在、快照失败、负向样本出现记录、正向样本记录多于一条、缺 `expected.json`）和第二次超时都照旧直接失败。等待期限不变，记录落盘也仍不被当成系统退出。

## Alternatives considered

- **把 45 s 期限加长**：最省事，但目标是被挂起、没有 worker 在处理它，再等多久也不会被结束；原脚本也明确不延长期限。不采用。
- **超时后只要记录完整就算通过并自己结束进程**：能盖住正向样本的停顿，但两次 `external` 停顿时本来就不该有记录，这条规则判不了负向样本；而且会把「记录落盘」当成「系统处置完成」，放弃退出码断言。不采用。
- **测试期间在当前用户下关闭 WER 排队或界面再恢复**：可能消除停顿，但要改 WER 设置。原脚本只登记 helper、只启动 WerSvc，有意不改策略；负向样本已经去掉排队标志，照样会停，说明这条路未必有效。不采用。
- **给整个 job 加自动重试**：会把真实的 helper 回归也一起重跑掉，还要重编整个 job。不采用。

## Consequences

- 单个用例偶发停顿不再挡 PR；同一用例连续两次停顿，或出现任何不符合上述形态的超时，仍然失败。代价是：一个只是偶发让目标卡住的真实回归，可能被这一次重跑放过。不过这种回归也得表现成「挂起、无 worker、记录合规」才会被重跑；每次重跑都会在日志留 warning，诊断也随 artifact 保留。
- `Test-SystemWerStall` 在 win-runner 上用 Windows PowerShell 5.1 和 pwsh 7.5 验证过：上面三份 CI 诊断都判为停顿；多出记录、WerFault 存活、半成品记录、有线程在运行、没有 LPC 等待、目标已退出、worker 快照失败、缺 `expected.json` 这些变体都判为不重跑；`Save-SystemWerTimeout` 只返回一个有序字典并照常写 `timeout.json`。
- 带重跑的完整脚本只能在 PR 的 Windows native settings job 里跑通，本机和 win-runner 上都没有跑过真实的系统 WER 投递。
