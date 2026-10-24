# Agent Note: Windows Shift 组合键同时取消钩子和 TSF 轻点判定

Status: implemented

## Problem

#9445 报告 Windows 0.3.0 在 Chrome、设置页和 Electron 应用中使用 Shift 大写或括号时误切换中英文。线程键盘钩子和 TSF 各自布防单按 Shift；钩子观察到普通组合键时只取消 `_bareShiftArmed`，TSF 的 `_shiftHotkeyArmed` 仍可能保留。生产方法回归确认：普通按键只由钩子观察、未交给 TSF 时，Shift 松开仍被认作切换。此合成序列尚不是报告者实际事件链路的采集证据。

## Decision

钩子看到非 Shift 按下时同时取消 TSF 的 Shift 布防；TSF 看到非 Shift 按下时也取消钩子的 `_bareShiftArmed`，因为应用可能只向其中一路交付普通键或修饰键。TSF 只有在没有其他按键按住、且现有钩子没有取消本轮轻点时才能布防，避免重复通知恢复已经取消的组合键。钩子没有观察到本轮 Shift 按下时不否决 TSF 路径，保留不同入口顺序的兼容性。实体 Shift 尚未松开时不接受 TSF 松开回调，留给真实松开处理。保留已有开关、500 毫秒窗口、线程内钩子和单次切换去重。

检索既有笔记后，[二三候选键](../../implemented/feature/2026-10-09-second-third-candidate-keys.md)仅提到裸 Shift 与选词键冲突，[Caps Lock 切换](../../implemented/feature/2026-10-11-caps-lock-language-switch.md)涉及共用按键入口；二者边界不变，本次单独记录 Shift 两路判定的取消契约。

## Alternatives considered

只在 TSF 的普通按下回调取消，路径最少，也能覆盖完整交付按键的应用；但不能覆盖只由线程钩子观察到组合键的情况，正是回归发现的缺口。

只让钩子切换，能消除双路状态，但现有 TSF 松开路径还承担正常宿主的即时交付和去重；删除它会扩大修改范围，并改变既有回调顺序。

## Verification

`scripts/test-windows-shift-chord.py` 提取并执行生产钩子、布防和松开匹配方法。未修改源码失败于钩子已经观察组合键、TSF 松开仍触发切换；修复后覆盖普通组合键未交给 TSF、重复 Shift 通知、实体仍按住时的松开回调、先按普通键再按 Shift、无钩子回退、钩子尚未观察按下的 TSF 入口、左右 Shift 重叠、单按一次切换、超时和开关关闭；Ctrl 原有切换也通过。

`scripts/test-windows-shift-chord-observation.py` 另行提取同一组生产方法，覆盖普通键、Ctrl/Alt/Win 只交付 TSF 及随后重复 Shift 通知；合入原实现时 33 项中有 9 项失败，补齐反向取消后 33 项全部通过。两套回归均保留。

本机专项、`test-windows-preedit-caret.py`、164 项可移植 Windows 原生测试、`pnpm run verify-notes` 和 `git diff --check` 通过。远端 `scripts/verify-local.sh --quick` 通过，包含最终专项、全部脚本契约、Rust workspace 和共享 Apple 桥接层。远端没有 MinGW 和 Node，Windows 产品编译交给 PR CI，笔记校验在本机补齐。实际 Chrome/Electron 中的连续输入与中英文切换尚未验收。

## Consequences

回归中的按键状态和系统调用由模型提供，不替代真实 Windows 输入法事件采集。沿用当前线程钩子，不新增全局监听或输入日志。
