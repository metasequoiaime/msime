# Agent Note: #11914 macOS Wine 游戏快捷键调查

Status: proposed

## Problem

#11914 报告在 macOS 上玩红色警戒（共和国之辉）时，水杉英文模式下游戏快捷键失效，只能切换到系统 ABC 输入法。issue 已补充为 macOS 平台，但尚未提供 Wine、CrossOver 或其他封装的版本、失效键组合、窗口模式和同一窗口切换 ABC 的对照结果。

代码核查显示，macOS 英文模式的字母、Shift+字母以及 Command、Control、Option 组合在 `platforms/macos/src/input/InputController.mm` 的 `handleEvent:client:` 中交回应用，现有 `ShortcutTest.mm` 已覆盖这些透传路径。输入法侧没有把这些事件转换成文本或吞掉组合键。

## Proposal

把 #11914 作为宿主透传与 Wine 输入接收之间的调查记录合入仓库。修复前先收集同一环境下的运行方式和版本、具体失效按键、窗口化或独占全屏状态、英文模式进入方式、系统 ABC 对照，以及开启诊断日志后的 `[key-latency] stage=handle ... handled=0` 记录。`handled=0` 只能证明输入法交回事件，不能单独证明 Wine 已正确接收。

只有在复现证据显示输入法没有交回应交回的事件时，才修改 `InputController` 并增加回归测试。若普通文本应用和 ABC 对照正常，而 Wine 仍丢失按键，则保留现有透传实现，在 issue 中记录问题属于 Wine 或封装层的接收路径。

## Alternatives considered

- 为红色警戒或所有 Wine 应用新增应用规则：当前没有应用标识、封装方式和失效键证据，过早加入规则会影响其他 Wine 应用的快捷键。
- 改写英文模式的组合键处理：现有回归已证明输入法会把组合键交回应用，缺少改写依据。
- 直接关闭或修复 issue：报告仍缺少可复现的封装版本和键组合，无法区分宿主责任与 Wine 接收问题。

## Acceptance criteria

- issue 补充运行封装及版本、失效按键、窗口模式和 ABC 对照。
- 在同一构建上记录输入法是否以 `handled=0` 交回对应事件。
- 若确认宿主责任，另开代码 PR 使用 `Fixes #11914`，包含回归测试和 macOS 原生构建验证。
- 若确认 Wine 或封装层责任，在 issue 中发布对照结论，不修改输入法透传逻辑。

## Risks

在缺少复现的情况下为单个游戏增加规则，可能吞掉本应交给其他应用的快捷键。改变英文模式的透传语义会影响所有 macOS 用户的组合键行为。
