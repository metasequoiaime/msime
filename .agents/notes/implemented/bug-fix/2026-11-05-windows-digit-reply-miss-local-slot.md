# Agent Note: Windows 数字选词的回复没等到时按本地这一页的位置选，不再整段重放

Status: implemented

## Problem

#10826：Windows 五笔在 CPU 吃紧时（后台复制文件、WinRAR 压力测试）用数字键选后面的候选，候选窗里的输入「重来」好几遍，最后不上屏或上屏错字。

宿主会话拥有组字时，数字键走 `_HandleCandidateWorker`（`tsf/UI/CandidateListUIPresenter.cpp`）：先用软等待（`abortTransportOnTimeout=false`，100ms）读这个键的回复，按 [翻页后照 Server 的回复选](2026-10-27-windows-paged-selection-follows-server.md) 的规则，`ServerReplyDecidesSelection` 为真时照回复选，读不到回复时才按本地这一页上按键位置的那一项选。但软等待超时的结果帧（`Ipc.cpp` 里的 `softMiss`）是 `Normal` 加空文字，与「Server 整段上屏了空文字」无法区分：

1. `ServerReplyDecidesSelection(Normal)` 为真，`ServerSelectedPiece` 取出空文字，`_SelectHighlightedHostCandidate` 返回 `S_FALSE`；本地这一页的退路永远走不到。
2. 编辑会话返回 `S_FALSE`，`CKeyHandlerEditSession` 判为没应用，`_RetryDeferredKeyReplay` 把已应用的整段读音（恢复检查点）连同数字键放回延迟队列，标脏传输、重连，Server 的会话随新代次清空后再重放一遍。这就是用户看到的「重复键入的动作」（与 #9518 的现象相同）。
3. CPU 一直吃紧时，重放出来的数字键同样等不到回复，再重来一遍；到 `kMaxDeferredKeyReplayAttempts`（8 次）后丢掉整批，于是「不上屏」。没有延迟回放令牌的数字键则直接被丢掉：Server 已经选中上屏并清空了组字，TIP 的宿主会话还停在原处，两边从此分开。

按键在超时前已经写进 Main 管道（`KeyEventSendResult::Sent`），Server 已经或稍后一定会执行这次选词；超时只说明 TIP 这一次没读到结论。

## Decision

- `shared/contracts/windows_ipc.h` 新增只在 TIP 本地使用的 `FanyImeReplyType::ReplyMissed`（`0xFFFFFFFE`），与 `TransportUnavailable` 一样从不上管道；`IsValidServerReply` 与光标扩展解码都拒绝它（高位置位、保留位非零）。
- `tsf/IPC/ReplyWaitPolicy.h` 的 `SoftReplyMiss(requestId)` 生成软等待超时的结果帧，类型是 `ReplyMissed`、不带文字；`TryReadDataFromServerPipeWithTimeout` 的两处软超时都用它。
- 其余软等待调用方（预编辑、UILess 组字、整句改字的空格）只认 `Preedit`、`UiLessComposition` 或直接丢弃结果，行为不变。数字键因 `ServerReplyDecidesSelection(ReplyMissed)` 为假，落到既有的本地退路：选本地这一页上按键位置的那一项，编辑会话返回 `S_OK`，不再触发重放。
- Server 端不需要改动：软超时帧不经过管道，协议格式和包长不变。迟到的真实回复照旧按 request id 缓存在 `pendingReplies`，到上限后被挤掉，不会被当成别的键的回复。

## Alternatives considered

- **Server 按 request id 去重，重放时沿用原 id 只回缓存的回包** — 最强的理由是它让「重放」本身幂等，硬超时、写失败等所有重放路径一并受益。否决：现在的重放之前会换激活令牌、清空 Server 会话的代次，重放的是一整段重建的读音（每个键都是新 request id），与原请求不是同一个；要去重就得让重建也沿用旧 id 并跨代次保留回包，改动两端协议，而本问题的根因是软超时被误判为 Server 的决定，不是重放本身。
- **软超时后不选词，改成重连后同步 Server 的状态** — 理论上能拿到 Server 选中的真实文字。否决：TIP 没有从 Server 拉回「刚才上屏了什么」的通道，需要新增消息和对应的 owner 校验；而 2026-10-27 的笔记已经定下「读不到回复时退回本地高亮项」，这里只是让实现符合它。
- **把数字键的软等待改成硬等待（超时即断线）** — 与空格一致。否决：硬超时同样返回失败、同样走整段重放，CPU 吃紧时问题不变，还多一次断线重连。
- **加长等待预算** — 能减少超时次数。否决：现场是持续的 CPU 压力，回包时间没有上界；按键路径在 UI 线程上同步等待，预算不能无限拉长（见 [候选回包等待预算](2026-10-25-windows-candidate-reply-timeout.md)）。

## Consequences

- **收益**：数字键的回复没在软等待里到时，宿主会话按本地这一页的位置选中并上屏一次，Server 也独立执行了同一个键；没有整段重放，也不会在八次重放之后丢键。
- **代价与已知上限**：退路选的是本地这一页上那个位置的词。两边的页不同时（滚轮只翻了 Server 那一页、两会话各自学习后顺序分开），文档里上屏的可能与 Server 选的不是同一个词，这是 2026-10-27 笔记里写明的旧退路。按键位置超出宿主会话这一页（`localSlotSelected=false`）时仍返回 `S_FALSE` 并重放。空格、标点等走硬等待的路径超时后仍断线重连并重放，未改。
- **重访信号**：现场仍报告慢机上选词后整段重来、或软超时退路选错词时，需要的是 Server 把这次选中的文字经 worker 管道补发给 TIP（或协议级的请求去重），而不是继续调大等待预算。

## Verification

- `platforms/windows/tests/input/shuangpin_paged_selection.cpp` 新增 `reply_missed_selects_local_slot`：两份真实会话加合成词库，Server 执行数字键 3 并上屏「泥好」，TIP 拿到 `SoftReplyMiss` 后按本地第 3 项选中、上屏同一个词一次，两边组字都清空。修复前（`SoftReplyMiss` 仍是 `Normal`）该场景在「a reply that never arrived is not a Server decision」处失败，改为 `ReplyMissed` 后通过。
- `msime-tsf-reply-wait-policy` 断言软超时帧不是任何 Server 回复类型、不是 `TransportUnavailable`、不能解码成光标提交；`msime-tsf-space-host-selection-wiring` 核对 `Ipc.cpp` 的软超时用 `SoftReplyMiss`。
- `python3 scripts/test-windows-native-run.py` 在 macOS 上整套通过；`Ipc.cpp` 只做了 MinGW `-fsyntax-only` 检查。没有在真实 Windows、CPU 压力下验收。
