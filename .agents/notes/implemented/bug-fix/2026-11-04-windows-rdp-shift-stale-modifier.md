# Agent Note: Windows 远程桌面切回后的单按 Shift

Status: implemented

## Problem

Windows 全屏远程桌面把本地按下、远端释放的 `Ctrl` 或 `Win` 键截走后，TSF 侧的 `GetAsyncKeyState` 仍可能显示这些修饰键按下。单按 Shift 的布防逻辑把这个残留状态当成真实组合键，因此切回本机后连续单按 Shift 都不能切换中英文。#10741 的诊断记录同时出现了 `Ctrl`/`Win` 的按下记录和后续的 `bare-shift-keyup`，说明 Shift 已到达 TIP，问题在组合键判定的状态来源。

## Decision

TSF 在键盘焦点离开时记录物理修饰键位作为「失焦前残留」掩码。重新获得焦点后，残留位在本地再次观察到对应修饰键按下之前不参与单按判定；新的本地按键会清掉自己的残留位。普通键和真实仍按住的修饰键继续按原规则取消单按，左右 Shift 共享既有判定。纯策略函数 `modifier_hotkey_allows_bare` 记录这一层规则，TSF 只负责读取 Win32 状态和维护焦点生命周期。

## Alternatives considered

- **只在切回焦点时清空全部 `GetAsyncKeyState` 判断**：实现简单，但会把用户实际仍按住的 Ctrl、Alt 或 Win 当成释放，可能把组合键误判成单按，因此保留按键级残留掩码。
- **收到任意普通键后重置全部修饰键状态**：能绕过 RDP 残留，但会让真实组合键在普通键后错误触发单按切换；因此只在本地再次看到对应修饰键时清除对应位。

## Consequences

新增 Windows TSF 的小型策略回归，覆盖 RDP 残留位允许新的单按 Shift、实际重新按下 Ctrl 重新参与组合键，以及普通单按保持原行为。该测试验证状态规则，不声称替代 Windows 11 全屏 RDP 真机验收；PR 验证段仍需报告者或 Windows 真机复验。
