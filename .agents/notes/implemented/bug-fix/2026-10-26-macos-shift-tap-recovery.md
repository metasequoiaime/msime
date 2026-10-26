# Agent Note: macOS 单按 Shift 在丢失松键后恢复

Status: implemented

## Problem

macOS 输入法的单按 Shift 检测会在 Shift 组合输入时取消当前轻点。若浏览器或其他文本编辑器丢失随后那次 Shift `flagsChanged` 松开事件，观察器仍把上一轮的 Shift 修饰状态留在 `modifiers_` 中。下一次单按 Shift 到来时，观察器把它当成上一轮的延续，不再布防，因此连续短按都无法切换中英文；Shift+空格仍可能工作，掩盖了问题。

## Decision

`MSIMEModifierTap` 在新的 Shift 或 Control 按下事件到来时，如果当前轻点没有布防、上一份修饰状态仍是同一修饰键、两侧修饰键实际没有同时按住、也没有其他物理按键按住，就把这次按下视为新一轮轻点并重新记录时间。两侧 Shift 同时按住时不重新布防，保留原有组合键取消规则；修饰键和普通按键的物理状态仍通过现有 `KeyHeldProbe` 查询，不读取输入文本。

## Alternatives considered

- 只在下一个 `flagsChanged` 事件中清理旧状态：丢失的正是这类事件，无法恢复。
- 每次看到同修饰键按下都重新布防：两侧 Shift 同时按住时会把被普通字符打断的组合误认成轻点，因此增加另一侧修饰键和其他按键的物理状态边界。
- 在输入控制器中针对浏览器或提交文字后强制 `resetModifierTap`：依赖应用名称和焦点时序，无法覆盖其他编辑器，也可能截断仍在进行的真实组合键。

## Consequences

丢失 Shift 松开通知后，下一次独立单按可以恢复中英文切换；仍在按住另一侧 Shift 或其他键时不会提前恢复，原有 Shift 组合和两侧 Shift 行为保持不变。`ShortcutTest` 增加了“Shift 组合后丢失松开通知，再次单按”的回归，真实 macOS `shortcut` CTest 通过。真实浏览器编辑器和报告者环境尚未验收。

与[Shift 组合键后的空格规则](2026-10-23-macos-shift-space-after-chord.md)互补：那篇记录同一轮 Shift 被字符打断后的空格路由，本篇处理跨轮次的修饰状态恢复。
