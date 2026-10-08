# Agent Note: 非游戏会话的独占抑制与全屏兜底

Status: proposed

## Problem

[Windows 游戏里由水杉画候选窗](../../implemented/feature/2026-10-08-windows-game-candidate-overlay.md) 只对「游戏会话」（包上带 `PipeMetadata::GameHost`，也就是命中强制叠加策略的进程）做独占全屏抑制和兜底定位。其余宿主仍按普通窗口处理，全屏游戏里留下两个缺口：

- 不是 UILess、由水杉自己画候选的游戏（推断包括 UE4/5、Unity 引擎的多数游戏，未实测）开了 D3D 独占全屏时，候选窗照样弹出：外部窗口显示不出来，弹出来还可能把游戏挤出全屏。
- 这类游戏在几何全屏下 `GetTextExt` 失败时，TIP 发来 `INVALID_Y`，Server 一直隐藏候选窗；锚点是垃圾值时候选窗跑到左上角。用户只有把游戏加进「总是显示」列表、让它变成游戏会话，才能得到兜底。

对游戏会话直接隐藏没有风险，因为那类宿主本来就没人画候选。对非游戏会话却不一样：它们现在能显示候选，判断一旦误报就是回归。

## Proposal

两项改动都只在 `platforms/windows/src/candidate/CandidateWindow.cpp` 和 `GameCandidateAnchor.h` 里，并且都要求前台窗口属于这个客户端进程（`client_pid` 等于前台 pid）：

- **非游戏会话的独占抑制**：前台呈现方式为 `ForegroundPresentation::ExclusiveFullscreen` 时，非游戏会话也走 `policy_hide`（隐藏并发一次渲染回执），抑制的进入和解除经 `take_suppression_changes()` 写进 Server 诊断日志，和游戏会话的抑制分开标记。
- **几何全屏下的兜底**：`GameAnchorInput` 加回 `fullscreen` 和 `invalid_timed_out` 两个字段，兜底条件扩成 `game_host && (invalid || garbage)`，或者 `fullscreen && (garbage || (invalid && invalid_timed_out))`。`invalid_timed_out` 表示这个快照的 `INVALID_Y` 已经持续 150ms，期间没有有效的 Move：`CandidateWindow` 记下首次看到 `INVALID_Y` 的时间，主循环每轮重试。等 150ms 是为了避开 `reposition()` 里那段注释担心的情况：浏览器这类宿主的首个 Show 先于文本 extent 到达，立刻兜底会在兜底点闪一下再跳走。
- 非全屏、非游戏会话遇到 `INVALID_Y` 仍然隐藏。
- 测试：`tests/ui/game_candidate_anchor.cpp` 补「全屏加 `invalid_timed_out` 兜底」「全屏但未超时不兜底」「非全屏非游戏会话不兜底」；`tests/runtime/server_smoke.cpp` 补非游戏会话在独占全屏下隐藏并发回执。

合入前提是全屏优化（FSO）实测：选一款 UE5 游戏，分别在无边框、全屏（FSO 开）、全屏（FSO 关）三种模式下，记录 `SHQueryUserNotificationState` 的返回值和候选窗表现。只有确认 FSO 开时**不**返回 `QUNS_RUNNING_D3D_FULL_SCREEN`，才合入独占抑制这一半；兜底这一半不依赖 QUNS，可以先合。

## Alternatives considered

- **和游戏会话一起立刻抑制** — 规则对所有宿主一致，任何独占全屏的游戏都不会被候选窗挤出全屏，也省掉一轮实测。但 FSO 下全屏游戏实际走的是合成路径，外部窗口能叠在上面；如果 QUNS 在 FSO 下也报 D3D 独占，这条规则会把现在能显示候选的游戏变成不显示，属于回归。游戏会话没有这个风险，所以先只对它们生效。
- **全屏下 `INVALID_Y` 立刻兜底，不等 150ms** — 候选出现得更早，实现也更简单，不用记时间、不用主循环重试。但浏览器这类宿主的首个 Show 常常先于文本 extent 到达，立刻兜底会先在客户区左下部闪一下，再跳到真实位置，非游戏宿主上这种跳动每次组字都会发生。
- **维持现状，只靠「总是显示」列表** — 不改任何非游戏会话的行为，零回归风险；用户把游戏加进列表后，它就是游戏会话，兜底和抑制都有了。但用户得先知道有这个列表、知道该加哪个程序名；几何全屏下 `INVALID_Y` 的游戏在用户找到设置之前一直没有候选。

## Acceptance criteria

- FSO 实测记录写进本笔记（或转 implemented 时写进 `## Verification`）：三种模式下 QUNS 的返回值和候选窗表现。FSO 开时返回 `QUNS_RUNNING_D3D_FULL_SCREEN` 的话，独占抑制这一半转为 rejected 或改用别的判据，不合入。
- 非游戏会话在 D3D 独占全屏、前台属于客户端进程时不弹出候选窗，选词键不等渲染回执超时，Server 诊断日志有抑制的进入和解除。
- 几何全屏的非游戏会话 `INVALID_Y` 持续 150ms 后，候选窗出现在游戏客户区左下部；150ms 内到达有效 Move 时直接出现在真实位置，不经过兜底点。
- 浏览器和普通编辑器（非全屏）的行为不变。
- 上面列出的自动化用例在 Wine 和原生 Windows 上通过。

## Risks

- QUNS 是系统全局状态，不针对某个窗口；只认前台 pid 能挡住「另一个进程在独占」的情况，挡不住同一进程的误报。
- Vulkan 独占和 OpenGL 改分辨率全屏可能根本不让 QUNS 报 D3D 独占（未验证）。游戏会话有反应式锁存兜底，这份提案不给非游戏会话加锁存：锁存的代价是第一次弹出已经造成一次破坏，对现在能用的宿主不划算。要加的话另写提案。
- 150ms 是估计值，没有测过各类宿主的 extent 到达延迟。太短会在浏览器类宿主上闪，太长会让游戏里第一次弹出明显滞后。
- 回滚只需去掉两处条件，不涉及协议和偏好格式。
