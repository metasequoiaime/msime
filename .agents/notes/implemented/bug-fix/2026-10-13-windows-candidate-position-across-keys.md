# Agent Note: Windows 按键投递沿用同一租约里报过的候选窗位置

Status: implemented

## Problem

TIP 只往游戏会话的按键包里写坐标（`Ipc.cpp` 在协商到 GameHost 能力后补上 `Global::Point`）；普通宿主的按键包用写入标志 `0b000111`，`point` 是线上结构的默认值 (100,100)。`candidate_presentation` 把包里的坐标抄进 `x/y`，`CandidateMailbox::delivered` 原样换上，于是每个按键之后、TIP 随后的 Show/Move 到达之前，展示的位置都是 (100,100)。候选窗的主循环至少每 50 毫秒 `refresh()` 一次，在这段间隙里刷新就把卡片摆到屏幕左上角，等 Show/Move 到了再跳回去。

宿主报的是 INVALID_Y（还没有文字范围、`TS_E_NOLAYOUT` 之外的 `GetTextExt` 失败，TIP 的本意是把卡片藏起来）时更糟：下一个按键的 (100,100) 把这个哨兵顶掉，卡片在左上角露出来，直到下一个 Show/Move 再把它藏回去。

[输入模式提示只认 Show/Move 报的锚点](2026-10-11-windows-digit-commit-and-hud-anchor.md) 当时为提示另记了 `caret_anchor_*`，并且有意不动 `x/y`，理由是候选窗在真机上的位置是对的。那次验收看的是停稳之后的位置，这段一帧到几十毫秒的间隙看不出来；`candidate_mailbox` 的用例当时还把按键之后 `x == 100 && y == 100` 钉成了期望。

## Decision

`CandidateMailbox::delivered` 在同一租约里、包上没有 GameHost 位时，沿用上一份展示的 `x/y`，不用包里的默认值。`x/y` 因此始终是这个租约里最近一次 Show/Move（或带坐标的游戏按键包）报的位置，包括 INVALID_Y。带 GameHost 位的按键包照用包里的坐标；新租约的第一份展示没有可沿用的，仍是包里的值，随后的 Show/Move 照常改写。点选之后的投递（`request_selection` 用画出来的 `x/y` 拼包）在普通宿主上同样沿用最近一次报的位置。

`caret_anchor_*` 保留：新租约里还没有 Show/Move 时 `x/y` 仍是 (100,100)，宿主还没有文字范围时又是 INVALID_Y，输入模式提示的兜底仍只认 Show/Move 写的那一份。

## Alternatives considered

- **维持现状，只靠另记的 `caret_anchor_*`**（10-11 那篇笔记的决定） — 不动候选窗的定位时序，游戏兜底定位和「锚点无效时等下一个 Move」都不受影响。现在不再成立：那两条判断恰恰被 (100,100) 带偏——游戏会话里漏了 GameHost 位的按键包带着默认值，落在游戏客户区里时被当成可信锚点，INVALID_Y 被它覆盖后不再等 Move；沿用之后它们拿到的是宿主真正报过的值。游戏会话带坐标的按键包不受影响。
- **新租约的第一个按键也不用默认值，改成 INVALID_Y 等第一个 Show** — 连第一次组字的那一帧左上角也一起消掉。没有采用：`server_smoke` 这类只发按键、不发 Show/Move 的路径靠这个默认值显示卡片；没有 Show 跟上的宿主（如果有）会从「位置不对」变成「完全看不到候选」，这需要真机确认，这次不改。

## Consequences

- **收益**：组字过程中候选窗不再在按键之后闪到屏幕左上角；宿主报了 INVALID_Y 的那段时间卡片保持隐藏，不会被下一个按键顶出来。
- **代价与已知上限**：一个租约里第二次组字的第一个按键，卡片先出现在上一次组字最后报的位置，Show/Move 到达后移到新位置（以前是先到左上角）。新租约的第一个按键仍可能闪一下 (100,100)。没有在真机上用高速截屏确认闪烁消失。

基岩版首键与组字后的 extent 持续互相矛盾，不属于短暂默认位置问题；按进程固定物理锚点的补充见[游戏 UI-less 候选同步与 IMM 状态](2026-10-16-windows-game-uiless-and-imm.md)。

## Verification

- `platforms/windows/tests/ui/candidate_mailbox.cpp`（随 `windows-session-smoke` 运行）：Move 之后的按键投递沿用 Move 的位置；Move 报 INVALID_Y 之后的按键投递仍是 INVALID_Y；带 GameHost 位的按键包照用包里的坐标。改动前第一条断言失败（`x` 是 100）。
