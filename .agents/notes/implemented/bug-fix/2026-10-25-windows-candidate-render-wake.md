# Agent Note: Windows 渲染屏障等回执时叫醒候选窗

Status: implemented

## Problem

#9519 的现场日志里，所有 `reply-wait-timeout` 都落在空格键（`vk=0x20`）上，耗时 53–61ms，刚好越过当时 50ms 的回包预算；普通字母键的 `edit-session-body` 只有 12–19ms。第一次超时之后，同一个空格的 `deferred-key-queue` 从 688ms 涨到 2125ms，每隔约 500ms 再超时一次，请求号每轮跳 19–38，最后 `deferred-replay-abandoned` 整队作废。

Server 在空格和数字键分派前调用 `SessionController::wait_candidate_render_for_key`，最多等 `candidate_render_wait_max_ms`（30ms），等候选窗画出当前这一页的回执（`CandidateMailbox::wait_rendered`）。回执能不能在 30ms 内到，取决于候选窗什么时候读到新快照，而它完全靠轮询：

- 0.3.0（报告者的版本）由 Server 主循环刷新候选窗，主循环每轮 `MsgWaitForMultipleObjectsEx` 最多睡 50ms；主线程上装着低级键盘钩子，物理按键会顺带叫醒它（推断，未在 Windows 上测量）。
- 当前 develop 改由 [桌面候选路由](../feature/2026-10-24-windows-desktop-candidate-routing.md) 的协调线程和桌面线程刷新，两级各 `WaitForSingleObject`/`MsgWaitForMultipleObjectsEx` 16ms，和物理按键不再有任何关联。

Server 进程没有调用 `timeBeginPeriod`。按 [timeBeginPeriod 说明](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod)，Windows 10 2004 起其他进程提高的计时精度不再作用于本进程，16ms 的等待按默认 15.6ms 精度要 16–31ms，50ms 的要 50–62.5ms。所以按键一旦排在上一键刚发布的新页后面，屏障大概率空等满 30ms，回包时间是 30ms 加上引擎处理，正好落在 53–61ms。

TIP 超时后关管道、重连，并把整段组字前缀摊回队首重放（[分类重置保留 FIFO](2026-10-25-windows-classification-reset-fifo.md) 与 [本地重置诊断](../process/2026-10-24-windows-local-session-reset-diagnostics.md) 记录了这条恢复路径）。重放时字母键接连发布新页，紧跟着的空格又排在一页刚发布、还没被轮询到的候选后面，于是重放必然重新制造同样的等待，超时一轮接一轮，直到用尽 `kMaxDeferredKeyReplayAttempts`。整队作废后 Server 一侧的会话已被重置，候选窗跟着消失，TIP 本地组字只剩原始字母和虚线，这就是「候选框消失只剩虚线」。正常、不积压的打字很少触发，一旦加快节奏、按键在 TIP 队列里积压，就进入这个循环。

[候选回包预算](2026-10-25-windows-candidate-reply-timeout.md) 把预算放宽到 100ms，现场的 53–61ms 不再越线，但屏障仍然在每个快速打出的空格和数字键上让 TIP 线程同步多等最多 30ms，引擎处理只要超过约 70ms，同一个重放循环照样成立。

## Decision

`CandidateMailbox::wait_rendered` 发现回执还没到时，先调用登记的绘制请求，再等回执。请求经 `CandidateMailbox::set_render_request` 登记，`SessionController::set_candidate_render_request` 和 `WindowsServer::set_candidate_render_request` 转发；`server_main` 在构造 `DesktopCandidates` 之后登记 `DesktopCandidates::wake()`，并用声明在它之后的守卫在它析构之前撤掉。

- 绘制请求只发信号：回调必须不阻塞、不回调信箱或控制器。信箱在独立的 `render_request_mutex_` 下调用它，`set_render_request({})` 返回时正在进行的调用已经结束，所以撤掉之后旧回调不会再被调用。
- 已经画到这一页时不请求，屏障直接通过；信箱停止时也不请求。
- `DesktopCandidates::wake()` 置位协调线程的自动复位事件。协调线程从 `WaitForMultipleObjects(stop, wake)` 醒来后立即读一次快照，交给各桌面线程，并置位每个桌面线程自己的唤醒事件；桌面线程的 `MsgWaitForMultipleObjectsEx` 带上这个事件，醒来后刷新并绘制，`WM_PAINT` 末尾照旧发回执。没有被叫醒时两级仍按原来的 16ms 轮询。
- 空格、数字键的按键屏障，以及点选、候选菜单、翻页三条点击路径的屏障都经过 `wait_rendered`，都得到同样的叫醒。

屏障的语义和 30ms 上限不变：仍然是等这一页画出来，或者有界超时后照常分派。变的只是这 30ms 花在真正的绘制上，而不是花在等候选窗下一次轮询上。

## Historical audit

- [候选回包预算](2026-10-25-windows-candidate-reply-timeout.md)：部分重叠，保留 100ms 预算的决定，在其后果里补充屏障占去的份额并互链。
- [分类重置保留 FIFO](2026-10-25-windows-classification-reset-fifo.md) 与 [本地重置诊断](../process/2026-10-24-windows-local-session-reset-diagnostics.md)：部分重叠，它们持有超时之后的恢复与重放路径，本篇不改那条路径，只去掉让重放必然再超时的等待。
- [桌面候选路由](../feature/2026-10-24-windows-desktop-candidate-routing.md)：部分重叠，保留按桌面拆分的线程归属与轮询，补充叫醒事实并互链。
- [跨平台候选绘制边界](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：部分重叠，30ms 上限的事实不变，补充叫醒并互链。
- [游戏候选叠加](../feature/2026-10-08-windows-game-candidate-overlay.md)：相关，它让策略隐藏也发回执、选词键不白等 30ms；本篇沿用这条回执规则，不改动。
- [候选非阻塞读取](2026-10-23-windows-candidate-read-contention.md)：相关，协调线程被叫醒后的读取仍是非阻塞的，争用时保留旧帧的规则不变。

## Alternatives considered

- **删掉按键屏障。** 它对按键的选择结果没有影响（选哪个候选由引擎当前这一页决定，等不等绘制都一样），删掉最省事、能彻底去掉这 30ms。但 #2524 加它时没有写明理由，从代码看是为了让上屏之前这一页至少画出来过；去不去掉这层展示约束属于产品取舍，不在本缺陷的修复范围内。
- **屏障与引擎处理并行。** 把按键先交给输入队列，只把回包和投递压到回执之后，等待时间从「30ms 加引擎」变成两者取大。代价是改变按键与云候选、释义等异步刷新在输入队列上的先后，`SessionPump` 的测试只能在 Windows 原生构建里运行，这里无法验证这层并发顺序，所以不用。
- **候选信箱每次发布都叫醒候选窗。** 能让所有候选刷新都更快，但发布发生在焦点闸门之内，协调线程用非阻塞读取会正好撞上闸门而读空，还得另加重试；屏障发起请求时闸门是空闲的，只在这里叫醒就够解决卡顿。
- **继续放宽 TIP 回包预算。** 只是推迟越线，屏障的空等和重放循环的正反馈都还在，而且会让真正失效的传输更晚被发现。

## Consequences

- 快速连打和重连后的重放中，空格、数字键不再空等候选窗的轮询，回包时间回到绘制加引擎处理；预算余量重新留给引擎本身。
- 每次屏障未满足时多一次 `SetEvent` 和一轮提前的快照读取与绘制，开销与原来每 16ms 一次的轮询同级。
- 候选窗确实画不出来（失败、桌面未路由、快照读取正好争用）时，行为与原来相同：屏障在 30ms 后放行。
- TIP 侧「超时 → 重连 → 重放前缀 → 再超时」的结构没有改，真正超过 100ms 的回包仍会进入有界的重试；本修复去掉的是让重放必然再超时的那一份等待。
- 报告中约两分钟后的 `Server stopping: session controller failed (input queue)` 不在本修复范围内，现有日志不足以判断是哪个输入队列任务失败或队列已满。

## Verification

新增 `tests/ui/candidate_render_request.cpp`（CMake 目标 `windows-candidate-render-request`，可移植 runner 链接 `ChineseTextConversion.cpp` 与共享宿主库运行）：用一个平时按 500ms 轮询、被请求时立即读快照并回执的假候选界面，验证屏障在 30ms 预算内拿到回执且每次未满足的屏障只请求一次、已画过的页不请求、连续九页各自拿到回执、撤掉请求后不再调用旧回调且回到有界超时、停止后不请求。在只登记不调用请求的实现上，该测试 30 次运行全部在第一条断言失败（屏障等满 30ms 未拿到回执）；改为先请求再等待后 30 次全部通过。整套可移植 runner（`python3 scripts/test-windows-native-run.py`，经编译机运行）183 项通过、0 失败。测试数据均为合成值。

`DesktopCandidates` 与 `server_main` 的改动只能由 Windows 构建验证：本机与编译机都没有 MinGW 或 Windows，原生部分只经过代码审读，由 PR 上的 Windows GNU 交叉构建确认编译链接，`windows-desktop-candidates` 原生测试在 CI 上不运行。真实 TIP、Server 与编辑器中的快速连续输入尚未验收。
