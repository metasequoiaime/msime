# Agent Note: Windows 选词回包在 Server 慢时继续等，不当成断线或空回复

Status: implemented

## Problem

#10826：Windows 10、六代 CPU，用 WinRAR 性能测试或复制文件把 CPU 占满后，五笔输入 `shwy` 再按数字选第 3 个候选，记事本里看到按键被重复几次却不上屏，或者最后上屏了别的字（报告里是「睢」）。报告人说资源紧张时每次都出现，旧版本也有。

选词和提交的回包都只等 [第一级预算](2026-10-25-windows-candidate-reply-timeout.md) `kServerReplyWaitTimeoutMs`（100ms）。CPU 被占满时 Server 只是慢，回包晚于 100ms 就会走进下面两条路径之一，取决于这一键落在哪个读回包的地方：

- **必须确认的回包**（`abortTransportOnTimeout=true`：`_HandleCandidateFinalize` 里的空格选词和非宿主会话的数字选词、带候选的标点提交、`WM_AsyncServerCandidateKey` 的逗号句号翻页键、联想选择）。超时后 `TryReadDataFromServerPipeWithTimeout` 关掉回包管道、请求重连、返回 `TransportUnavailable`；选词键随后走 `_RetryDeferredKeyReplay`，把当前组字按 rawInput 摊成按键排在选词键前面，标脏会话后整串重放，最多 8 次，从第 3 次起退避并发 `WM_IpcReconnect`。重放的每一轮又要过同一个 100ms，于是一直超时、一直重放。
- **宿主会话里的数字选词**（`_HandleCandidateSelectByNumber` → `_HandleCandidateConvert` → `_HandleCandidateWorker`，TIP 自己的宿主会话拥有组字时，这是数字选词的常规路径）。这里读的是软回包（`abortTransportOnTimeout=false`），同样只等 100ms。等不到时 `softMiss` 返回的是 `Normal` 加空文字，`ServerReplyDecidesSelection(Normal)` 把它当成 Server 的决定，`_SelectHighlightedHostCandidate` 拿空文字找不到候选，返回 `S_FALSE`，什么也不上屏，按键却已经吃掉。Server 那边这一键已经选过了，晚到的回包只会进 `pendingReplies`，两边的组字从这里分开。

`ReadOverlappedWithTimeout` 区分得出两种情况：管道断了（Server 退出、句柄失效）时读请求立刻以失败完成，返回 `Failed`；返回 `TimedOut` 说明这次读是我们自己取消的，管道还连着，Server 没来得及回。Server 对选词键一定会回一帧（`ReplyComposer::stage` 的 `ReplyPath::Selection` 不是编出回复就是 `InvalidFields`，后者会断开连接，读会以失败返回），所以等得更久不会在正常情况下白等。

## Decision

回包等待分两级，都写在 `IPC/ReplyWaitPolicy.h`：

- 第一级 `kServerReplyWaitTimeoutMs` 仍是 100ms。预编辑、候选窗刷新、UILess 组字这些可以缺席的软回包只等这一级，等不到照旧回落本地状态，不拆管道。
- 第二级 `kServerReplyStallBudgetMs` 是 1000ms。必须确认的回包由 `ServerReplyWaitBudgetMs(abortTransportOnTimeout)` 取到这一级；`TryReadDataFromServerPipeWithTimeout` 新增显式预算的三参数版本，两参数版本按这个函数取预算后转给它。宿主会话里的数字选词仍是软回包、仍不拆管道，但用三参数版本等到第二级。管道报错、包长不对、帧不合法仍然立刻按断线处理；第二级上限到了也仍然走原来的路径（必须确认的回包关管道、重连、重放，宿主会话的数字选词返回软回包）。

超过第一级才到的回包记成 `reply-wait-slow`（`IsSlowServerReply`），不再和普通的 `reply-wait` 混在一起，现场日志能直接看出「Server 慢但没断」。

焦点激活等回执（`EnsureNamedpipeFocusSessionActivated` 里的轮询）仍然只用第一级，见 [焦点回执循环](2026-11-02-windows-tsf-focus-echo-loop.md)；它只在激活或重连之后才走到，选词回包不再误判之后很少触发。

## 没有确认的部分

「上屏错字」那一半没有确认根因。「睢」的五笔码是 `hwyg`，落在 `hwy` 前缀的候选里，像是某次重放后 Server 的组字丢了首码 `s`，或者两边的组字在一次读不到回复之后已经分开，下一次选词时 Server 那一页是另一份候选。两份附件 `server.log` 只有 Explorer.EXE 的记录，没有记事本进程的 `reply-wait-timeout` 或 `deferred-replay` 记录，也就分不出报告人当时走的是上面哪一条路径。这次只去掉有代码证据的触发条件：晚于 100ms 的回包被当成断线或空回复。

两处已知但没改的行为：选词键在真正超时（超过 1000ms）后仍会原样重放；宿主会话里的软回包超时仍会被当成 Server 的「Normal 空回复」而不是退回本地高亮项（`ServerReplyDecidesSelection` 的注释说的是后者）。改这两处要动恢复队列语义或给软回包换一个新的哨兵类型，等拿到带 `reply-wait-slow` / `deferred-replay-retry` 的现场日志再定。

## Alternatives considered

- **把第一级直接提到 1000ms，所有回包一起等**：实现最少，但预编辑和 UILess 组字的回包本来允许缺席，几乎每个字母键都会读，Server 慢时每个键都多卡将近一秒；所以只给选词和提交这类决定上屏内容的回包加第二级。
- **把宿主会话的数字选词改成必须确认的回包**：等得一样久，但超时后会拆管道、整串重放，正是要避开的那条路径；这里原本特意不拆管道（注释写着「和 `_HandleConversionKey` 一样」），所以只延长预算，不改超时后的处理。
- **选词类键超时后不重放，改成标脏会话、按 rawInput 恢复组字让用户重选**：直接切断「重复几次」的表现，但它改的是恢复队列的语义，牵涉 `_RetryDeferredKeyReplay`、`_CompleteDeferredKeyReplay` 和多个异步候选消息的交接；而错字那一半的根因还没定，现在改恢复语义是在猜。先去掉触发条件。
- **超时后再查 Server 进程是否还活着再决定**：TSF 侧没有现成的 Server 进程句柄，`TimedOut` 本身已经说明管道还连着，Server 退出时读会以 `Failed` 返回，不需要额外探测。
- **无限等待直到回包**：Server 卡死时会让编辑器 UI 线程永久挂住；[上一篇](2026-10-25-windows-candidate-reply-timeout.md) 已经否掉，这里仍保持有限上限。

## Consequences

CPU 被占满、Server 回包在 100ms 到 1000ms 之间时，选词和提交等它回来再上屏，不再拆管道、不再整串重放，宿主会话里的数字选词也不再吞键。这一键期间编辑器的 UI 线程最多多阻塞约 900ms。Server 卡死但管道没断时，选词键要等满 1000ms 才进入原有路径，比原来多等 900ms。超过 1000ms 的回包、真正的断线、读失败都和原来一样。

`msime-tsf-reply-wait-policy` 断言软回包仍是第一级、必须确认的回包用第二级、第二级至少是第一级的 5 倍且不超过 1000ms，并核对 `Ipc.cpp` 按 `ServerReplyWaitBudgetMs(abortTransportOnTimeout)` 和调用方给的预算算截止时间、用 `IsSlowServerReply` 记慢回包，以及 `_HandleCandidateWorker` 的宿主会话读回包用 `kServerReplyStallBudgetMs`。改动前这个测试编译不过（没有 `ServerReplyWaitBudgetMs`），新测试对旧源码的接线检查也失败。`msime-tsf-gloss-column-wiring` 跟着核对新的读回包调用。TIP 本身只能在 Windows 上构建，压 CPU 后在真实编辑器里选词的验收还没做。
