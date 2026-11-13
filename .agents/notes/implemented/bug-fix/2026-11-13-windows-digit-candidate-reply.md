# Agent Note: Windows 候选不足时消费数字选词回执

Status: implemented

## Problem

Server 按自己的候选页处理数字键。候选页只有八行、而宿主候选页仍持有真实的一页时，按数字 9 会让 Server 返回无动作的选词回执。TSF 的数字键路径在本地槽位不存在时直接返回，未读取这份回执；下一次按键可能读到旧回执，导致宿主会话和 Server 会话错位。

## Decision

`_HandleCandidateSelectByNumber` 在真实宿主候选页没有对应槽位时，继续调用 `_HandleCandidateWorker`，并标记 `localSlotSelected=false`，让共享的 Server 回执路径消费 `NavigationIgnored` 或 `OutofRange`。只有没有有效请求或没有真实宿主页时才保持原来的 `S_FALSE` 返回。

把“本地槽位缺失但需要跟随 Server 回执”的判定放进 `common/SecondThirdCandidatePolicy.h`，数字键和二三候选键共用同一个纯规则；测试覆盖真实宿主页、已选槽位、缺少请求三种边界。

## Alternatives considered

直接让候选页显示一个不可选的第九行会改变候选窗口和编号语义，也无法处理 Server 已翻页而宿主仍停在另一页的情况，因此不采用。把 Server 的回执改成不发送会使 Server 已经处理的按键无法与宿主会话同步，也不采用。

## Consequences

候选不足时数字键仍由输入法消费，但不会把旧回执留在管道中，也不会从宿主自己的错误槽位选词。真实 Windows 宿主行为需要在 Windows 原生测试或设备环境中继续验收；可移植测试验证了槽位判定规则。
