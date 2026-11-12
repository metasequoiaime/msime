# Agent Note: HarmonyOS 快捷短语混入普通候选

Status: implemented

## Problem

#10991 明确要求 HarmonyOS 与 Windows 支持快捷短语混入普通候选。共享引擎已经支持 `mixed_input.quick_phrase`，但 HarmonyOS 的宿主能力位和键盘会话请求都缺少 `quick_phrase_candidates`，因此设置开关即使存在也不会改变普通候选。

Windows 的同一范围决策记录在对应的 Windows PR 笔记中，本篇只记录 HarmonyOS 的独立宿主接线；旧的「仅 macOS、Linux」范围见 [快捷短语混输与调频两个共享偏好](../../implemented/feature/2026-10-22-quick-phrase-preferences.md)。

## Decision

`HostCapabilities::quick_phrase_candidates` 现在包含 HarmonyOS。HarmonyOS 键盘在 `KeyboardSession.start` 取得 `PreparedOptions` 后写入 `quick_phrase_candidates = true`，并在 ArkTS 类型中声明这个字段；host-api 继续与 `mixed_input.quick_phrase` 相与。HarmonyOS 设置页读取同一份宿主能力文档，因此共享设置中的「快捷短语混输」控件随能力出现，偏好关闭时仍由 host-api 阻止短语进入普通候选。

## Alternatives considered

- 只在 HarmonyOS 设置页显示开关：键盘会话没有请求能力，开关会成为无效控件。
- 在 ArkTS 侧直接根据偏好筛选候选：候选排序、调频和跨宿主行为已经由共享引擎负责，宿主筛选会破坏统一契约。
- 与 Windows 共用一个宿主改动 PR：违反一个 issue 一个 PR，且 HarmonyOS 需要独立 HAP/设备验收。

## Consequences

HarmonyOS 用户打开「快捷短语混输」后，普通组字会显示编码相同的快捷短语；关闭开关或总的「候选混输」时保持原行为，Shift+K 快捷短语模式不受影响。共享能力测试覆盖 HarmonyOS，host-api 原有偏好门控测试覆盖关闭/开启。当前环境没有 HarmonyOS 签名设备，HAP 构建、安装、真实键盘候选和设置页截图仍需设备 CI 或报告者复验，PR 将保持 Draft。
