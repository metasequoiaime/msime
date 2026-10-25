# Agent Note: Windows 单个请求校验失败不再停掉输入队列

Status: implemented

## Problem

#9525 和 #9706 的现象是候选窗消失、组字变成带虚线的原始字母、回车把它们吞掉。源头已由 [回车原文提交观测读取宿主会话](2026-10-24-windows-enter-commit-observation.md) 修掉：TIP 交给 Server 的回车观测读错了来源。但它暴露出的放大链路还在：

1. `ReplyComposer::dispatch` 发现本地回车的观测和 Server 的组字对不上，抛出 `std::invalid_argument`。
2. `SessionPump` 的按键任务不接异常，它冒到 `InputQueue::run`。
3. `InputQueue::run` 把任何任务异常都当成状态已经损坏：`state.shutdown()` 加 `request_stop()`，队列不再接收。
4. `SessionController::run` 看到 `accepting == false`，记 `ControllerFailure::InputQueue`，Server 写下 `Server stopping: session controller failed (input queue)` 后退出，看门狗再把它拉起来。

一个客户端的一次错观测就让所有客户端一起断，日志里也只有「input queue」，看不出是哪一项校验没过。观测仍可能对不上：TIP 的观测超过 127 个 UTF-16 码元时按约定清空（不截断成另一段文字），读宿主快照失败时也是空，两种情况在 #9528 之后照样会触发整条链路。

## Decision

把派发路径上的异常分成两类，分界写进类型：

- **单个请求被拒（`RequestRejected`）**：TIP 交来的数据没通过边界校验，而且在改动 Engine 之前就发现了。`RequestRejected` 派生自 `std::invalid_argument`，带一个固定的校验名（`check()`）和一段只有计数的说明（`detail()`）。现在只有本地回车观测这一项用它：观测缺失记 `observation=missing expected_bytes=N`，不符记 `observation=mismatch observed_bytes=M expected_bytes=N`，都不带文本。
- **其余异常照旧致命**：Engine 调用失败、无法编码的选择结果（这些发生在 Engine 已经推进之后）、待回复闸门或路由不变量被破坏，仍然由 `InputQueue::run` 停掉队列。原来和观测校验写在同一个条件里的「LocalCommit 路由落在不是原文提交的键上」拆出来改抛 `std::logic_error`：`basic_key` 只在回车时选这条路由，落到别的键上说明路由本身错了，属于不变量。

`SessionPump` 的按键任务只接住 `RequestRejected`，接住之后：

- 不发回复帧。本地回车成功时本来就没有回复帧，TIP 不等它，所以拒绝也不会让 TIP 空等。
- 经 `Presentation::rejected` 交出一行诊断：`Request rejected: kind=key client=<id> request=<id> check=<校验名> <计数>; client connection reset`。`server_main` 把它写进 `server.log`（Server 日志开关打开时）。
- 返回新的 `PumpResult::RequestRejected`，照常走循环退出时的清理：撤销焦点、关闭这个客户端的管道、在输入队列上 `disconnected` 丢掉它的会话。输入队列和其他客户端照常运行。

### 为什么断开这一个客户端，而不是保留连接

拒绝的这一刻 Server 会话保持原样（旧的 `ab` 还在组字），而 TIP 那边多半已经从自己的宿主会话把文字写进了文档、结束了组字。两边已经不一致，保留连接意味着下一个键在 Server 上接成 `abc`：候选窗按 `abc` 出候选，之后的选词回复、上屏计数也都落在 TIP 那边不存在的组字上。Server 也不知道 TIP 是否真的写成功了，没法单方面猜一个状态去对齐。

断开后 TIP 走它对任何 Server 侧断开都有的路径（`MarkNamedpipeSessionDirty`、本地会话重置、重连、焦点确认后重放排队的键）：已经上屏的回车不受影响，若组字还在则按本地重置的规则处理，新连接上的 Server 会话从空组字开始，和 TIP 重新对齐。这条路径和 Server 重启时 TIP 走的是同一条，只是不再连带其他客户端，也不用等看门狗。TIP 侧不需要改动。

### 历史检索

- [回车原文提交观测读取宿主会话](2026-10-24-windows-enter-commit-observation.md)：持有观测的来源和校验规则，校验本身不变；它说「Server 的观测校验、提交命令与恢复策略保持原规则」，被拒之后的处理改由本篇负责，两篇互链。
- [候选窗绘制请求唤醒](2026-10-25-windows-candidate-render-wake.md)：记下报告里的 `input queue` 停机不在其范围、日志不足以判断，部分相关，互链。本篇只覆盖被拒的回车这一种来源，队列已满或其他任务失败仍是同一行停机日志。
- [本地会话重置与延迟重放链路加诊断日志](../process/2026-10-24-windows-local-session-reset-diagnostics.md)：断开后 TIP 走的正是那条重置与重放链路，本篇不改它，无需改动。

## Alternatives considered

- **在 `InputQueue::run` 里区分异常类型，让被拒的任务以新状态完成。** 好处是所有提交到队列的任务都自动获得这条规则。不用它：被拒之后还要决定这个客户端怎么办，这个决定属于持有传输和焦点的 `SessionPump`；队列只该知道「任务完成」或「状态已损坏」，让它认识请求语义会把两层职责搅在一起。
- **被拒后保留连接，只回一个拒绝帧或干脆不回。** 改动最小，也最贴近「只拒绝这一个请求」的字面意思。不用它：见上文，Server 留着 TIP 已经结束的组字，后续按键会在错误的状态上继续；TIP 也不读本地回车的回复，拒绝帧没有接收方。
- **被拒时在 Server 上就地取消组字，保留连接。** 能和「TIP 已经上屏」的常见情况对齐。不用它：TIP 自己的上屏也可能失败、组字还在，这时 Server 清空而 TIP 没清，方向反过来又不一致；断开重连让 TIP 自己的状态决定结果，两种情况都对。
- **观测对不上时 Server 照自己的组字提交。** 前一篇已经否掉：失去「TIP 交来的原文属于同一组字」的独立校验。

## Consequences

- 一次错观测不再让 Server 停机重启，其他客户端不受影响；这个客户端短暂断开重连，候选窗随断开收起，重连后恢复。
- Server 日志能直接看出是哪一项校验没过、对应哪个客户端和请求号，以及两边各有多少字节，便于判断是超长、读取失败还是内容不符；日志里没有输入内容。
- 被拒的回车不做共享提交，所以不学习、不计入打字统计；TIP 那边已经上屏的文字也就不计数。
- 新增的拒绝只覆盖本地回车观测。`second_third_candidate` 与候选回车里检查请求号、载荷长度的 `std::invalid_argument` 在 `SessionPump` 之前已经被 `valid_main_frame` 拦下，这条路径到不了，仍按不变量处理；`InputState` 对模式通知、配对标点和字段属性的同类检查也是。以后新增「TIP 数据不对、Engine 还没动」的校验，应当抛 `RequestRejected` 而不是普通异常。
- `stage()` 里的事后校验（Engine 已经推进之后发现观测不符）仍以无法编码的回复结束循环，结果是 `DispatchFailed`，同样只断开这一个客户端，但没有诊断行。

## Verification

新增可移植测试 `tests/input/local_commit_rejection.cpp`（CMake 目标 `windows-local-commit-rejection`，runner 链接 `ReplyComposer.cpp`、`ServerSession.cpp` 等与共享宿主库运行）：五笔和全拼各组字 `ab`，空观测、短观测、同长不同文、完全没有观测四种都抛 `RequestRejected`，校验名和计数正确，诊断文本里没有 `ab`/`ba`，Engine view 与回复合成器均不变；随后同一会话的 `c` 正常组成 `abc`，匹配的观测照常上屏；LocalCommit 路由落在字母键上抛 `std::logic_error`。

新增原生测试 `tests/runtime/session_pump.cpp` 的 `local_commit_rejection_pump_tests`（CMake 目标 `windows-local-commit-rejection-pump`）：经真实 `InputQueue`、`SessionPump` 和 `production_key_handler` 派发 `a`、`b` 和不带观测的回车，断言返回 `RequestRejected`、队列仍在接收且没有失败任务、只断开这一个客户端、恰好一行诊断且不含输入、没有带回车请求号的回复帧；同一客户端重连后 `c` 在新会话里组成 `c` 而不是 `abc`；按键处理器抛普通 `std::invalid_argument` 时队列照旧停止且没有诊断行。

在 develop 的实现上，经编译机运行 `python3 scripts/test-windows-native-run.py` 退出 1，两项测试因 `RequestRejected` 与 `Presentation::rejected` 不存在而编译失败，其余全部通过；修复后同一命令 187 项通过、0 失败，单独编译执行 `local_commit_rejection` 输出通过。`session_pump.cpp` 链接 `FocusedSession`，后者依赖 `windows.h`，可移植 runner 跑不了它；它在 macOS 上以 Clang 和 MinGW GCC（`-Wall -Wextra -Werror`）做过语法检查，链接由 PR 上的 Windows GNU 交叉构建确认，但 CI 不执行原生测试，本机与编译机也没有 Windows 或 Wine，所以这项原生测试和 Server 的日志接线都还没有实际运行过。真实 TIP、Server 与编辑器中的被拒回车与重连尚未验收。
