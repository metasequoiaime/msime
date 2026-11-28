# Agent Note: macOS 按住修饰键点鼠标不算轻点

Status: implemented

## Problem

#11914：英文模式下，在 Wine 里跑的红色警戒游戏快捷键时灵时不灵，切到系统 ABC 就正常。宿主在英文模式下对不带修饰的字母和 Cmd / Ctrl / Option 组合都 `return NO`，键本身没有被吞；真正会改掉英文状态的是单按 Shift（出厂开着，`keybindings.switch_language_shift`）和单按 Control（`switch_language_ctrl`）的轻点检测。

`platforms/macos/src/core/ModifierTap.h` 判断轻点只看键盘：修饰键按下到松开之间没有别的键、两侧修饰键状态一致、不超过半秒。鼠标完全不在判断里，只有打开了「仅在可编辑焦点切换」（`switch_language_shift_requires_editable`，默认关）时 `EditableShiftGate` 才会因为鼠标取消这一轮。于是默认配置下 Shift+点击（RTS 加选单位、编辑器扩展选区）、Shift+滚轮（横向滚动）、打开单按 Control 时的 Ctrl+点击，松开修饰键时都会被当作轻点：切到中文，并且这次松开的 flagsChanged 由输入法 `return YES` 拿走。之后游戏的字母快捷键全部进入拼音组字，玩家看到的就是「快捷键失效」。报告者没有回复复现细节，这是从代码里能确定的、在默认设置下就会发生的那条路径。

## Decision

`MSIMEModifierTap` 在布防（修饰键按下）时记下窗口服务器的鼠标计数和按键状态，松开时再读一次：期间有过鼠标按下、松开、拖动或滚轮事件，或者两端任何一端有鼠标键按着，这一轮就不是轻点。计数沿用 `EditableShiftFocus.h` 里已有的 `EditableShiftMouseEvents()` / `EditableShiftMouseButtonHeld()`（`kCGEventSourceStateCombinedSessionState` 上的 `CGEventSourceCounterForEventType` 与 `CGEventSourceButtonState`），不需要辅助功能权限，也看得到落在别的进程里的点击。只在布防和候选松开这两个边沿各读一次，其余按键不碰。

测试通过 `setMouseProbe` 注入替身；`reset()` 和已有的 `setKeyHeldProbe` 一样保留注入的替身。

## Existing notes audit

[可编辑焦点限制](../feature/2026-10-24-macos-editable-shift.md) 部分重叠：它的门禁仍然只在打开开关时做焦点确认和临时鼠标监视；本项把「鼠标操作取消本轮」挪进不带限制的默认路径，开关打开时两边都会取消，结论一致。[单按 Shift 进入大写英文](../feature/2026-11-26-macos-uppercase-english-shortcut.md) 无冲突：它只在轻点成立之后生效。[Shift+Space 打断与漏释放](2026-10-23-macos-shift-space-after-chord.md) 部分重叠：键盘侧的打断规则不变。没有完全吸收或过时的提案。

## Alternatives considered

- **按 Wine / CrossOver 的 bundle id 整组透传。** 最强理由是只影响游戏、不改其他应用的行为。没有采用：Wine 起的 Windows 程序是不带 bundle 的可执行文件，IMK 客户端报给输入法的标识无法在本仓库里确认，凭猜测写死名单不可靠；而且同一个问题在原生应用里也存在（Shift+点击扩展选区会切中英文），按应用打补丁只盖住一处。需要整组透传的用户已经可以用「应用输入模式」把该应用固定为英文，再关掉单按 Shift。
- **用 `NSEvent` 全局鼠标监视器，像 `EditableShiftGate` 那样。** 监视器回调可能晚于 IMK 的松键通知，`EditableShiftGate` 本身也靠计数器补这个时序；默认路径每次按 Shift 都装卸监视器代价也更大。计数器单独就够。

## Consequences

所有应用里 Shift / Control 加鼠标的组合不再切换中英文；不碰鼠标的单按行为不变，鼠标移动（不按键）不计入。`shortcut-test` 的 `TestModifierTapIgnoresModifierClicks` 用合成鼠标覆盖：按住期间点击、松开时鼠标键仍按着、先按鼠标再按修饰键、之后恢复正常单按、双侧 Shift 之间的点击。真实 Wine 游戏里的验收没有做，报告者环境未复现。
