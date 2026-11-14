# Agent Note: 鸿蒙双拼触屏分词键

Status: implemented

## Problem

#11097 反馈鸿蒙触屏双拼的 26 键字母页没有可输入的分词符。`KeyboardView` 已经把左下角动作接到 ASCII 撇号，但 `KeyboardScheme.usesQuanpinSeparator` 只允许 `preferenceId === "quanpin"`，小鹤、自然码、微软和首道双拼因此始终走大小写 Shift 路径。

## Decision

鸿蒙触屏中文双拼在 26 键字母页也显示并发送分词符。判断仍要求触屏、中文字母层和非本地模式；双拼不受 `touch_quanpin_shift` 影响，因为该偏好只描述全拼用户是否保留 Shift 作为辅助码入口。全拼继续按该偏好在分词和 Shift 之间切换，九键全拼、五笔、英文、符号层和其他本地输入保持原行为。

这次沿用既有的 `tapLetterShift`、`KeyIdPolicy.character("'")` 和引擎撇号入口，不在键盘视图另造双拼编码或编辑器状态。行为边界与[中文全拼触屏分词键](../feature/2026-10-15-quanpin-touch-separator.md)一致，但修正其“其他方案保留大小写”的鸿蒙特例；Android 和 iOS 的既有行为不在本 issue 范围内。

## Alternatives considered

- **只修改双拼键帽，不修改动作**：改动表面最小，但点击仍会切换大小写，无法输入 issue 所需的分词符，因此不能采用。
- **把双拼分词符绑定到 `touch_quanpin_shift`**：可以复用一个布尔值，但会让名称为全拼设计的偏好意外关闭双拼必需的组字能力，也与硬件键盘已经允许双拼撇号的边界不一致。
- **在双拼专用视图中单独实现分词动作**：会复制现有键位统计、读屏和引擎派发逻辑，增加触屏形态漂移；继续复用 `usesQuanpinSeparator` 和 `tapLetterShift` 更稳定。

## Consequences

双拼用户可以在鸿蒙 26 键触屏键盘上直接分隔音节，撇号仍按 `Quote` 统计并交给现有引擎组字。全拼保留 Shift 的辅助码用法不变，其他方案不会因为本次修复改变键面。代价是鸿蒙双拼触屏左下角不再提供单击大小写切换；英文层仍使用独立的 Shift 语义，中英文切换入口保持原样。

## Verification

- 新增纯策略覆盖：四种双拼方案显示分词且忽略 `touch_quanpin_shift`，全拼开关边界、英文、符号、本地模式、九键全拼和五笔边界保持原断言。
- 待执行：`pnpm run verify-notes`、`git diff --check`、`bash platforms/harmony/check-host.sh`、`bash platforms/harmony/tests/run.sh`；若本机工具链和磁盘允许，再执行 HAP 构建和设备验收。
- 当前尚未在真实鸿蒙编辑器中完成触屏双拼分词验收，PR 验证段会明确列出该缺口。
