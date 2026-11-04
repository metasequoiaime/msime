# Agent Note: 系统 WER 投递增加不加载产品模块的对照

Status: implemented

## Problem

#10865 中两次独立 Windows WER 作业在 x86 的 `external`、`withdrawn` 连续超时。目标只有一个线程等待，其余线程挂起；相关 `WerFault` 可能仍在，也可能已经消失。等待原因不能说明 worker 究竟卡在哪里，已有 callback / lifecycle 测试通过也不足以证明产品没有影响。[停顿重试笔记](2026-11-02-windows-system-wer-x86-stall-retry.md) 已明确根因未定，拆成独立作业与现有一次重试都没有修复它。

## Decision

系统专项先运行 `baseline`：沿用同一合成可执行文件、环境、错误模式、负向样本的 WER 标志和故障函数，但在加载 Host API、TIP、helper 和建立采集线程之前触发故障。夹具还断言三份采集模块没有加载，写入有界 `expected.json` 后关闭文件，再触发访问异常。

基线仍须由系统结束目标，退出码必须是访问异常，不得有采集记录或半成品。沿用原有超时、稳定窗口和最多一次受限重试，不放宽产品用例，也不把记录落盘当作系统退出。基线失败保留同样的线程快照并直接令作业失败。

这是一项定位手段：基线也停顿时，可以证明没有产品模块的系统故障路径同样停顿；基线通过不能排除产品造成的间歇故障，更不能宣布 #10865 已修复。

## Alternatives considered

- **继续增加重试或等待期限**：可以暂时减少红灯，但已有连续两次超时，不会补上系统与产品的因果对照，因此不采用。
- **只读取等待原因就归为系统环境**：不增加测试耗时，但这些原因不含等待对象和符号栈，不能排除产品或夹具，因此不采用。

## Consequences

- 每个架构增加一个负向样本，正常情况下增加约 30 秒稳定窗口；不更改产品逻辑或系统通用策略。
- 当前基线只覆盖无排队的负向标志，不能推论带排队标志的正向样本。
- 历史笔记审计：与停顿重试笔记部分重叠并互链；[TSF WER 采集](../feature/2026-11-01-windows-tsf-wer-capture.md) 的产品边界不变，其余决定不受影响。

## Verification

- 本机完整 `bash scripts/verify-local.sh --quick` 退出 0，包含 Windows 原生行为门禁、仓库契约、笔记、Rust workspace、Apple 桥接及 x64/x86 管道交叉构建；完整 Windows 宿主因依赖树未配置而跳过，wasm 按改动范围跳过。
- PowerShell Parser 校验通过；MinGW x64/x86 按 full 版宏定义执行 C++17 `-Wall -Wextra -Werror -fsyntax-only` 均通过。
- 尚未在真实 Windows 上运行新增基线。PR 的 `Windows TSF system WER` 作业会调用同一脚本；必须读取该作业的实际结果后再判断系统故障路径。
