# Agent Note: Windows 的 Shift 组合键观察跨 TSF 与线程钩子同步

Status: implemented

## Problem

#9445 报告 Windows 0.3.0 在网页宿主中输入 Shift 字母或标点时误切中英文。当前 TIP 的 TSF 回调和 `WH_KEYBOARD` 线程钩子分别维护 `_shiftHotkeyArmed` 与 `_bareShiftArmed`，任一条路径观察到组合键，只取消自己的状态。如果宿主让字母只经过其中一条路径，另一条路径仍可能把 Shift 松开当成单按。合成模型编译执行实际 `_TrackModifierHotkeyArming` 和 `_BareShiftKeyboardHookProc`：Shift 两路可见、字母只给 TSF、松开只给线程钩子，原实现错误投递一次切换。这个模型证明状态遗漏，不证明报告者 Chrome 的真实事件序列。

## Decision

两条观察路径看到非 Shift 键按下，都取消另一条路径的 Shift 单按状态。保留原有超时、左右键、焦点代次、去重与回放边界；不新增钩子或改变按键归属。回归编译执行实际三种方法，覆盖只给 TSF、只给钩子、双路观察及纯 Shift。

## Historical audit

[二三候选键](../../implemented/feature/2026-10-09-second-third-candidate-keys.md)部分重叠：保留其 Windows 单按 Shift 与真实设备验证边界，不引入左右 Shift 选词。[Caps Lock 切换](../../implemented/feature/2026-10-11-caps-lock-language-switch.md)部分重叠：沿用 TSF 中其它键取消轻点的原则，不修改 Caps Lock。[桌面生命周期](../../implemented/architecture/2026-10-11-desktop-lifecycle-core.md)只涉及切换后的反馈，无需修改。其余检索命中的 Android 大写恢复、macOS 菜单状态及 Linux 模式提示与本状态契约无关。

## References

- [Microsoft KeyboardProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/keyboardproc)：线程钩子观察消息队列事件。
- [Microsoft TestKeyDown](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeystrokemgr-testkeydown) 与 [TestKeyUp](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfkeystrokemgr-testkeyup)：TSF 探测和处理由宿主派发；不能假定所有宿主的两条观察路径完全相同。

## Alternatives considered

- 改成只保留 TSF 状态：只有一个判定来源，状态容易一致；已有 Word、mintty 不提供 TSF 松开回调的兼容路径会丢失，保留线程钩子。
- 改用全局低级键盘钩子：能观察更完整的物理按键；扩大进程与性能边界，也无法替代 TSF 自身的焦点归属，采用现有两路状态的最小同步。

## Verification

原实现上，编译执行实际方法的回归 33 项检查有 15 项失败（`issue-9445-shift-regression-red.log`），失败均为跨观察路径组合键仍能切换及组合键后重复 Shift 的回退。修复后 33 项检查全部通过（`issue-9445-shift-regression-green.log`），覆盖 TSF 单路、钩子单路、双路、字母、数字、标点、Ctrl/Alt/Win、左右 Shift、重复探测。独立复审未发现生产修改阻塞，并在内存用原始源码再次确认 33/15 失败。复审指出最初右 Shift 用例仍用左扫描码后，夹具已按左右键分别提供 `0x2a`/`0x36`；修正后的用例重新通过。完整 `bash scripts/verify-local.sh --quick` 已通过（`issue-9445-quick-final.log`），本机 Windows 可移植套件 164 项通过；2 项按宿主差异排除、76 项需要 Windows 构建。本机未安装 MinGW，因此 Windows 交叉构建未运行；真实 Chrome/Electron 验收尚未完成。

## Consequences

复用现有观察链路，组合键只需被一方看见即可取消误切；无需新增全局钩子或把平台规则放进引擎。

合成模型替代 Win32 状态与消息边界，不能证明 Chrome 的实际回调顺序，也不能证明 #9445 的唯一原因。跨编译不等于系统输入法验收。
