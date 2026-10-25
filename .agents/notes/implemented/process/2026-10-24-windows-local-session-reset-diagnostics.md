# Agent Note: 本地会话重置与延迟重放链路加诊断日志

Status: implemented

## Problem

Windows 桌面端快速连续输入时，输入法会卡住数秒，候选框消失，选词之后整段拼音被重来一遍，最后上屏常常出错（#9518、#9519）。日志里能看到的只有结果：`reply-wait-timeout` 带着 `0x800705B4`、`deferred-key-queue` 从 688ms 涨到 6094ms、`dropped` 从 42 涨到 111、最后 `Server stopping: session controller failed (input queue)`。

看不到的是过程。读了代码之后找到一条能解释全部现象的闭环，但没有一条日志能证实或者推翻它：

1. `MarkNamedpipeSessionDirty` 挂起 `_localSessionResetPending`（`IPC/Ipc.cpp`）
2. `_RequestLocalSessionReset` 发现还有组字，排一次取消组字的编辑会话（`IME/MetasequoiaIME.cpp`）
3. 那个编辑会话要走 TSF 的编辑通道，而通道正跟着传输在重连
4. `_CompleteLocalSessionReset` 只在「没有组字、候选窗为空」时才清标志，否则重排重连定时器再来一次
5. 组字要结束得靠按键被处理，而 `_ResolveDeferredKeyClassification` 一看到重置标志就把整个延迟队列作废

第 4 步和第 5 步互相等：不清标志→队列作废→组字结束不了→标志清不掉。

`_RefreshDeferredRecoveryPrefix` 是现象的直接来源。它把当前整串拼音摊回按键放进 `_deferredAppliedPrefix`，重连之后连同失败的那个键一起重放，用户看到的就是「整段输入被重来一次」。这个设计有它的道理（选词后剩余组字需要上下文），所以不能简单删掉。

## Decision

2026-10-25 复核：上文的第 4、5 步自锁仍是假设。取消组字专用编辑会话绕过普通 worker Ready 检查，并主动完成本地重置，不能据静态路径认定必须靠普通按键结束组字。已单独确认并修复的是分类期间同焦点重置清掉尚未发送尾键，见[分类重置保留 FIFO](../bug-fix/2026-10-25-windows-classification-reset-fifo.md)。本篇保留诊断决定，不把新增日志或局部修复当作整项卡顿已解决。

加五处诊断日志，不改任何行为，全部走 `QueueTsfDiagnosticLog` 并且只在 `TsfDiagnosticLogEnabled` 打开时输出。

- `_RequestLocalSessionReset`：入口和「编辑会话已排过」两条，记 token、是否有组字、是否有候选窗、是否需要取消
- `_CompleteLocalSessionReset`：四个出口各一条，分别是 `enter`、`superseded`、`cleared`、`blocked`、`drained`。`blocked` 反复出现而 `cleared` 一直不出现，就是闭环坐实
- `_RefreshDeferredRecoveryPrefix`：`raw_len`、`caret`、`move_left`，直接量化「整段重来」的长度
- `_ClearDeferredKeyDowns`：补一条完整状态，含 `reset_pending`、`applied_prefix`、`generation`
- `_ResolveDeferredKeyClassification` 的作废分支：把四个条件分别记成 `owner_current`、`enabled`、`reset_pending`、`foreground_changed`，分清是焦点真的变了还是传输在重连

两个文件各加一个匿名命名空间的 `BoolFlag`。直接写 `L"x=" + (cond ? L"1" : L"0")` 两边都是 `const wchar_t*`，是非法指针相加，编译不过；`BoolFlag` 返回 `std::wstring` 把整条拼接链落在 `std::wstring` 上。

日志不记录任何输入内容，只记长度、布尔量、token 和代次。

直接抽取生产方法编译的回归夹具也声明诊断开关、空日志函数、`BoolFlag` 和默认关闭的 `_localSessionResetPending`。`application-key-replay` 的 `g_connected` 只定义一次；`deferred-recovery` 与 `wubi-commit-lifecycle` 保留对真实恢复、清队列及五笔收尾方法的执行，不删除生产方法中的诊断分支来绕过编译。

## Alternatives considered

- **直接按闭环假设改代码。** 试过一次：把 `_localSessionResetPending` 从 `_ResolveDeferredKeyClassification` 的作废条件里挪到重试分支。编译前就发现方向错了：`_RetryDeferredKeyReplay` 无条件调 `MarkNamedpipeSessionDirtyForOwner`，每次重试都会再触发一次重连，把重连风暴放大。没有日志确认闭环之前，任何改法都是猜。
- **只加日志不改任何东西，等维护者。** 和当前决定差不多，区别在于这五处是照着闭环的每个环节铺的，不是泛泛地加 trace，能直接读出卡在哪一步。
- **在 `_ClearDeferredKeyDowns` 加参数记录调用点。** 更直接。放弃是因为有八个调用点，而 `tests/input/smart_punctuation_focus_wiring.cpp:57` 依赖 `_ClearDeferredKeyDowns();` 这个字面形式做源码断言，改签名会连带改测试断言，收益不抵成本。

## Consequences

- 诊断日志开关打开时，日志量增加。按一次重置可能产生数条 `[local-reset]`，快速输入时 `[deferred-recovery]` 每个恢复前缀一条。开关默认关闭，只在排查时打开，可以接受。
- `BoolFlag` 在两个翻译单元里各有一份，是匿名命名空间里的 `inline`，不违反 ODR。
- 这次没有修任何缺陷。缺陷本身留给下一次改动，判断依据由这批日志提供。

## Verification

- 最初只做了静态检查：确认没有残留 `L"..." + (cond ? L"1" : L"0")` 形式的指针相加，确认每处拼接链的起始表达式落在 `std::wstring` 上，确认 `<string>` 与 `Ipc.h` 两个文件都已经包含。
- 2026-10-24 在 macOS 用 Clang 执行三个真实方法回归：`python3 scripts/test-windows-application-key-replay.py`（191 项）、`python3 scripts/test-windows-deferred-recovery.py`（0 失败）、`python3 scripts/test-windows-wubi-commit-lifecycle.py`（32 项）均通过。补齐夹具前分别因 `g_connected` 重复定义、重置状态缺失及诊断声明缺失编译失败。
- 确认 `smart_punctuation_focus_wiring.cpp` 依赖的 `ThreadMgrEventSink.cpp` 原文未被改动。
- 编译由 CI 的 Windows 作业验证。
- 真机复验（需要装开发版）：打开诊断日志开关，连打 `szyd` 后快速按空格上屏，日志里应当出现 `[local-reset] complete stage=blocked`，且同一 token 的 `stage=cleared` 缺席；同时出现 `[deferred-recovery] stage=prefix raw_len=` 且值等于当时已输入的拼音长度。
