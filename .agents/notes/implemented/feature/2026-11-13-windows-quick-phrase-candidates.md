# Agent Note: Windows 快捷短语混入普通候选

Status: implemented

## Problem

共享引擎已经支持把编码相同的快捷短语混进普通候选，偏好也已经落在 `mixed_input.quick_phrase`。当前 Windows 会话没有向 host-api 请求 `quick_phrase_candidates`，Windows 原生设置也没有这项开关，因此 Windows 用户无法使用 #10991 要求的能力。旧决定只允许 macOS、Linux 请求该能力，见 [快捷短语混输与调频两个共享偏好](../../implemented/feature/2026-10-22-quick-phrase-preferences.md)；本 issue 的报告者后来明确要求 Windows 10/11，因此这次只 supersede 其中的 Windows 范围，HarmonyOS 另开独立 issue 分支和验收。

## Decision

Windows 的 `HostCapabilities` 现在声明 `quick_phrase_candidates`，Windows 的 Server 和 TSF 会话选项包装器都写入该能力字段；host-api 继续把它与 `mixed_input.quick_phrase` 相与，所以用户关闭偏好时不会出现快捷短语。Windows 原生设置在「输入 → 候选与联想 → 更多选项」增加「快捷短语混输」开关，读写同一个共享偏好，并在「候选混输」总开关关闭时置灰。共享偏好的默认值和缺字段规则保持不变：本次只开放能力，不翻转已有用户的默认行为。

共享能力测试覆盖 Windows、macOS、Linux 与移动端的能力矩阵；Windows 生命周期回归测试确认两个会话入口传入 `quick_phrase_candidates`，共享 host-api 已有的偏好门控测试继续覆盖开关行为。

快捷短语能力与大写锁定标点分别验证：两项能力的宿主范围已经不同，旧的合取断言改为只验证标点，快捷短语由六平台矩阵完整验证，保留所有未接入宿主的关闭断言。

## Alternatives considered

- 只在 Windows 设置页增加开关：设置值会被宿主忽略，用户会看到一个无效控件，因此不能接受。
- 让 Windows 无条件打开混输：会改变现有用户的候选顺序，也绕过已有的 `mixed_input.quick_phrase` 偏好门控；这次只开放能力，不改默认值。
- 把 Windows 与 HarmonyOS 放进同一个 PR：违反一个 issue 一个 PR，也无法在同一套设备验收条件下完成；HarmonyOS 另开分支处理。

## Consequences

Windows 用户打开「快捷短语混输」后，普通组字会按共享引擎的现有规则显示编码相同的快捷短语；关闭开关或关闭总的「候选混输」时，仍可使用 Shift+K 的快捷短语模式。没有改变 macOS、Linux 或其他平台的默认值。Windows 真机候选窗口验收仍需 CI 或报告者在 Win10/11 上复验；当前环境没有 Windows 设备，也没有启动完整 Windows 构建。
