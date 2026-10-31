# Agent Note: 移动端全拼 26 键默认保留 Shift

Status: implemented

## Problem

Android、iOS 与 HarmonyOS 的触屏全拼 26 键原来在组字时把左下角的 Shift 替换成「分词」，因此默认看不到 Shift。三端的标准 26 键第二行已经各自保留了 A 与 L 两侧的半键缩进；这次需要恢复 Shift 的默认可达性，同时保留依赖分词键的用户路径。

历史行为和实现边界见[全拼 26 键用分词键替代触屏 Shift](../../implemented/feature/2026-10-15-quanpin-touch-separator.md)与[触屏全拼 26 键组字时保留 Shift 的共享偏好](../../implemented/feature/2026-10-26-touch-quanpin-shift-preference.md)。本笔记 supersede 后一篇笔记中的“默认关”决定。

## Decision

把共享偏好 `touch_quanpin_shift` 的默认值翻为 `true`。缺字段、旧文档或没有 App Group 镜像时，Android、iOS 与 HarmonyOS 都按开启处理；只有用户明确保存 `false` 时，组字中的左下角才显示「分词」并发送 `'`。九键、14 键、18 键、双拼和其他输入模式的分词或 Shift 语义不变。

Rust `Preferences` 使用 `enabled_by_default` 并在默认开启时省略序列化字段；明确的 `false` 仍会被保存和同步。Android 设置页、共享 React 设置页、iOS 原生设置页与 HarmonyOS 会话读取逻辑统一展示和解释这一默认值。A/L 第二行半键缩进沿用现有三端布局实现，本次不另改几何比例。

## 历史笔记审计

- `2026-10-15-quanpin-touch-separator.md` 仍准确记录分词替换的动作、统计和边界；默认值部分被本决定覆盖。
- `2026-10-26-touch-quanpin-shift-preference.md` 仍准确记录共享开关和跨端接线；其中“默认关、缺字段按关”被本决定改为“默认开、只有明确 false 才关”。
- 现有 A/L 缩进来自 Android `ImeLetterRows`、iOS `KeyboardViewController` / `KeyboardFormFactor` 和 HarmonyOS `KeyboardView`，没有发现需要修复的缺失实现。

## Alternatives considered

- 继续默认显示「分词」：保留了旧用户的按键手感，但新安装用户仍无法直接找到 Shift，和三端标准 26 键的可达性要求不符。
- 删除分词入口或再增加一个底行按键：会破坏明确关闭开关的用户，也会改变既有底行宽度；保留显式 `false` 的兼容路径更小。
- 只改一个平台：会让同一账号在 Android、iOS 与 HarmonyOS 上出现不同的全拼 26 键行为；共享偏好和三端缺字段回退一起调整。

## Consequences

新安装和缺字段的移动端全拼 26 键在组字时直接显示 Shift，可继续使用臂辅助码；需要断开音节的用户关闭「组字时保留 Shift」即可恢复「分词」。默认字段被省略时由各端回退为开启，因此老文档不会因为缺键意外回到分词；显式保存的旧 `false` 仍尊重用户选择。macOS 继续只使用物理键盘，不读取该触屏行为。

## Verification

- 代码契约：Rust 默认值与缺字段/显式 `false` 反序列化测试；Android 设备分词验收 fixture 显式关闭开关；iOS 分词 XCTest 显式关闭开关；HarmonyOS 纯策略测试同时覆盖开关两种分支。
- 静态检查：`git diff --check`、`pnpm run verify-notes`、Android host 检查和 HarmonyOS 纯逻辑套件在提交前运行。
- 未在本次改动中声称 Android、iOS 或 HarmonyOS 真机验收；真实设备/模拟器覆盖仍需 CI 或平台设备工作流完成。
