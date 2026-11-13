# Agent Note: macOS Caps Lock 撤回后的陈旧大写字母

Status: implemented

## Problem

#11304 报告 macOS 开启「单按 Caps Lock 切换中英文」后，轻点切到英文虽然成功，但随后输入的字母全部变成大写。现有撤回逻辑已经能处理仍带 `CapsLock` 修饰位的陈旧键事件，却在先到达关闭状态 `flagsChanged` 事件上清掉了临时标记；窗口服务器随后仍可能把同一次锁定状态翻译成不带 `CapsLock` 修饰位的大写字母。

本问题是[单按 Caps Lock 切换中英文](../feature/2026-10-11-caps-lock-language-switch.md)中「撤回锁定后的陈旧修饰位」规则的遗漏，也发生在[Caps Lock 轻点等待实体状态](2026-11-07-macos-caps-lock-tap-state-latency.md)新增的松键事件链之后。

## Decision

撤回锁定后保留 `_capsLockTapReleased`，不因单独的关闭状态 `flagsChanged` 事件立即清除。英文路径为每个事件先保存该状态：当首个无 Shift 的 ASCII 字母到达时，即使事件已经没有 `CapsLock` 修饰位，也把大写字符转换为小写，并在该字母被处理后消费一次性标记。带 Shift 的显式大写输入不消费也不转换，标记会留给下一个无 Shift 字母；非字母事件同样不消费，避免标点或编辑键遮蔽后续字母的陈旧状态。若先收到一次迟到的 Caps Lock 开启状态，再收到下一次开启事件，则把后者视为新的按键序列，避免旧标记阻塞后续轻点判定。

## Alternatives considered

- **继续在关闭状态事件上清除标记**：改动最小，但正是报告者时序下的根因；陈旧大写字母可能在该事件之后到达，无法再被识别。
- **永久保留标记直到下一次 Caps Lock 按下**：可以覆盖所有迟到事件，但会让后续普通英文大写输入长期依赖这个历史状态，边界不清晰。采用首个无 Shift ASCII 字母消费的一次性标记。
- **只根据事件的 `modifierFlags` 判断大小写**：不能覆盖窗口服务器已经将锁定状态从修饰位中移除、但 `characters` 仍为大写的事件，无法修复 #11304。

## Consequences

英文模式在撤回锁定后可以跨过先到的关闭状态事件，将陈旧的大写 ASCII 字母稳定还原为小写；标记只影响一个实际处理的无 Shift ASCII 字母，不改变显式 Shift 大写、标点、编辑键或其他输入模式。迟到的开启状态被识别后，下一次开启事件可以立即开始新的轻点判定。回归测试覆盖关闭状态事件先到、随后字母不带 `CapsLock` 修饰位，以及迟到状态后再次轻点的顺序。审查补充独立控制器的双场景断言，同时保留字母仍带 `CapsLock` 修饰位的旧回归，不用新时序替换原保护。

## Verification

- `git diff --check` 通过。
- 已更新 macOS 合成行为测试，保留仍带 `CapsLock` 修饰位的旧回归。远端构建 `shortcut-test` 成功，CTest `shortcut` 和 `caps-lock` 2/2 通过；quick `20261113-130155` 退出码 0。实体键盘与报告者环境仍待合并后复验。
