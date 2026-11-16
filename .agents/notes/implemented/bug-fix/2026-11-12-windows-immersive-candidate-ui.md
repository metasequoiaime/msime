# Agent Note: Windows 沉浸式宿主候选窗协商

Status: implemented

## Problem

Windows 11 26H2 的开始菜单可以通过水杉组字，但候选框不显示。#10827 已修复沉浸式进程加载 TSF 宿主库的路径问题，报告者随后确认开始菜单仍留下候选呈现缺口；#11263 单独跟踪这一层。

TSF 的 UI-less 约定允许宿主在 `ITfUIElementMgr::BeginUIElement` 中返回 `pbShow=TRUE`，把候选窗交还给输入法。现有实现却对所有 `TF_TMF_UIELEMENTENABLEDONLY` 激活无条件把 `_isShowMode` 改成 `FALSE`，并在 `ActivateEx` 里提前把 UI-less 状态锁存给 Server。沉浸式宿主因此无法使用这条协商路径，即使它没有自己的候选绘制实现。

## Decision

输入法从 `ITfThreadMgrEx::GetActiveFlags` 读取 `TF_TMF_IMMERSIVEMODE`，而不是把它误当成 `ActivateEx` 的输入标志。沉浸式 UI-less 激活先不向 Server 锁存宿主接管状态，等 `BeginUIElement` 的 `pbShow` 协商：

- `pbShow=TRUE` 时由水杉候选窗显示，Server 继续收到普通候选呈现包；
- `pbShow=FALSE` 时保留 UI-less 候选更新，交给宿主的 `ITfUIElementSink`；
- 非沉浸式 UI-less 宿主仍按原有游戏路径锁存并禁止输入法自有候选 HWND；强制叠加策略优先级不变。

协商规则集中在 `Global/CandidateUiModePolicy.h`，纯策略测试与既有候选叠加策略测试共用一个行为测试入口。这样不把开始菜单名称硬编码进 TSF，也不改变其他 UI-less 宿主的默认行为。

## Alternatives considered

- **所有 UI-less 宿主都尊重 `pbShow=TRUE`**：最接近 TSF 文档，但会重新放开未知游戏的输入法窗口；既有[游戏候选窗策略](../feature/2026-10-08-windows-game-candidate-overlay.md)已经明确把非沉浸式 UI-less 作为宿主接管路径，因此本次只收窄到运行时确认的沉浸式激活。
- **按 `StartMenuExperienceHost.exe` 或 `TextInputHost.exe` 特判**：能针对报告场景，但 TSF 代码实际可能运行在共享输入宿主中，进程名不能可靠表示当前焦点控件；也会把 Windows 版本差异固化成进程名单。
- **继续只补诊断日志**：能等待报告者提供 `pbShow` 和激活标志，但无法修复已确认的协商逻辑缺口；本次先按 TSF 合同实现，真实 Windows 11 26H2 验收仍作为 PR 的必要门槛。

## Consequences

- 沉浸式搜索/开始菜单若允许输入法自绘候选，候选窗可以恢复显示；若宿主明确要求自绘，仍不会出现第二个候选窗。
- Server 不再在激活早期错误锁存沉浸式 UI-less；`pbShow=FALSE` 的后续包仍带 UI-less 标志，保持宿主绘制路径。
- 非沉浸式 UI-less 游戏及现有强制叠加规则不变。
- 当前 macOS 开发机无法运行 Windows TSF 和 Windows 11 26H2 开始菜单；需要 Windows CI 编译、真实开始菜单候选显示/选词和游戏 UI-less 回归后才能把 PR 转为 ready。
