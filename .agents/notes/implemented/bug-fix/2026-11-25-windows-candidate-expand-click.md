# Agent Note: 候选窗点开「∨」不再立刻自己收起

Status: implemented

## Problem

#11767：Windows 上装着 `expand_panel = true` 的皮肤（报告者用的是第三方包「我们聊天绿」，`base = wechat`）时，横排候选行右端画出「∨」，点它没有任何反应——箭头不翻成「∧」，卡片不展开，网格也不出现。

卷轴展开是「以 Server 为准」的：候选窗把这一代的租约交给 Server（`WindowsServer::set_candidate_expansion` → `SessionController::set_candidate_expansion` → `CandidateMailbox::expansion`），之后每一帧读到的快照里，`CandidatePresentation::expansion_open` 由 Server 那一侧的 `expansion_lease_` 现算。候选窗的 `reposition` 每帧按 `candidate_expand_stale` 对照这个字段：Server 没开着就收起。这条对照防的是「候选窗开着、Server 已经关了」的错位。

问题出在点开的那一帧。`toggle_expansion` 先发打开请求，再同步调 `refresh()`，而 `refresh()` 读的是**桌面线程手里缓存的那一帧快照**（`DesktopCandidates::Surface::input`，由协调线程每 16 毫秒读一次 `server.candidate_view()` 后发布）。点开请求发出时，缓存里那一帧是请求之前读的，它的 `expansion_open` 还是 `false`；而 `expansion_lease_` 刚被同步写进 Server，租约又完全一致，于是：

```
candidate_expand_stale(expanded=true, server_open=false, same_lease=true, available=true) == true
```

`reposition` 当场 `collapse_expansion()`，并把 `expansion_(lease, false)` 发回 Server，Server 随即清掉刚记下的展开、`refresh_expansion` 排的任务也因 `expansion_due` 为假而作废。用户看到的就是「点了没反应」，而且每一次点都会这样，与皮肤、候选数量、按键都无关。读屏的「展开候选」走同一个 `toggle_expansion`，同样失效。

## Decision

点开「∨」时记下时刻 `expand_open_requested_at_`，`reposition` 在这一段（`candidate_expand_open_grace_millis = 250`）之内不把 `server_open == false` 当作收起理由，只等 Server 的下一帧快照；`candidate_expand_stale` 增加 `pending_open` 参数来承载这条规则。Server 确认（读到 `expansion_open == true`）或收起（`collapse_expansion`）都清掉这个时刻。

边界保持原样：

- 换了客户端（`same_lease` 为假）、改成竖排、换成不展开的皮肤、释义页或只能用键盘选的列表（`available` 为假）仍然**立刻**收起，不等宽限期。
- 宽限期过后 Server 仍说没开，照旧收起，不会留下一个开着、下面永远空着的「∧」——[卷轴展开](../feature/2026-10-19-windows-candidate-skin-expand-panel.md) 里不想出现的正是这个状态。
- 250 毫秒是给协调线程与桌面线程各一两个 16 毫秒轮询周期留的余量；Server 在点开时就已经同步记下展开，正常情况下下一帧就确认。

## Testing

- `platforms/windows/tests/ui/candidate_expand_panel.cpp` 新增四条 `static_assert`：请求刚发出、快照仍是 `server_open == false` 时 `candidate_expand_stale` 为假（`!candidate_expand_stale(true, false, true, true, true)`）；过了宽限期（`pending_open` 为假）仍为真；换客户端或这一页不能展开时即使请求挂着也立刻为真；宽限期不短于 100 毫秒。整个 `windowscandidate-expand-panel` 用例在本机 `c++ -std=c++17 -Wall -Wextra -Werror` 上编译并运行通过。
- `CandidateWindow.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++20 -fsyntax-only` 检查无错误、无新增警告。
- **未验证**：真实 Windows 上点「∨」的端到端观感（本机没有 Windows 宿主）。报告者装 nightly 后复验；`expand_panel` 的按钮、网格、滚轮观感自 [卷轴展开](../feature/2026-10-19-windows-candidate-skin-expand-panel.md) 起就没有在真机上验收过，这次一并请报告者看。

## Alternatives considered

- **让候选窗每帧直接问 Server 现算 `expansion_open`（新加一个跨线程的只读查询），而不是读缓存快照。** 最强的理由是它精确、不靠时间余量，把「快照过期」这一类问题从根上去掉。不采用：要在 `CandidateMailbox`、`SessionController`、`WindowsServer`、`DesktopCandidates` 各加一层只读接口，并让 `paint` 这条热路径每帧多拿一次 mailbox 锁；这次只需覆盖「自己刚发出的请求」这一处，把规则收在候选窗里更小、更容易在 Win32 之外测。
- **点开后不立刻 `refresh()`，等下一帧带 `expansion_open == true` 的快照再重排。** 最强的理由是它连宽限期都不需要。不采用：桌面线程的循环在 `DispatchMessageW` 之后紧接着就 `candidates.refresh()`，它读到的仍是同一个缓存快照（协调线程还没发布新的一份），照样会当场收起；能不能撑到下一帧取决于两个线程的轮询先后，不稳定。
- **去掉 `!server_open` 这一条，只认 `same_lease` 与 `available`。** 最强的理由是彻底没有这个竞态。不采用：Server 在候选窗关闭时先清掉展开（候选窗晚一步才看到），去掉这一条就会在退出动画期间留着一个「∧」下面空着的卡片，这正是要防的状态。

## Consequences

收益：点「∨」以及读屏的「展开候选」恢复成设计里的行为——箭头立刻翻成「∧」，后续候选到达后卡片长出网格。收起判断对「Server 已经关了」「换了客户端」「这一页不能展开」三种情形仍然是立刻生效的，没有放宽它们。

代价：存在最多 250 毫秒的窗口，候选窗会先把 `server_open == false` 当成「还在等」；若 Server 其实在点开后立刻关了（例如上屏后马上又开新组字），这一帧会晚最多 250 毫秒收起。候选窗自己关闭（`hide_now`）时是本地直接收起的，不经过这条路径，所以不会多留一帧。
