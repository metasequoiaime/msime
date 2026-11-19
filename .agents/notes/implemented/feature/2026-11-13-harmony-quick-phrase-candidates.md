# Agent Note: HarmonyOS 快捷短语混入普通候选

Status: implemented

## Problem

#10991 要求快捷短语混入普通候选，本篇处理 HarmonyOS 的宿主接线。共享引擎已经支持 `mixed_input.quick_phrase`，但 HarmonyOS 的宿主能力位和键盘会话请求都缺少 `quick_phrase_candidates`，因此设置开关即使存在也不会改变普通候选。

Windows 的同一范围决策记录在对应的 Windows PR 笔记中，本篇只记录 HarmonyOS 的独立宿主接线；旧的「仅 macOS、Linux」范围见 [快捷短语混输与调频两个共享偏好](../../implemented/feature/2026-10-22-quick-phrase-preferences.md)。

## Decision

`HostCapabilities::quick_phrase_candidates` 现在包含 HarmonyOS。HarmonyOS 键盘在 `KeyboardSession.start` 取得 `PreparedOptions` 后写入 `quick_phrase_candidates = true`，并在 ArkTS 类型中声明这个字段；host-api 继续与 `mixed_input.quick_phrase` 相与。HarmonyOS 设置页读取同一份宿主能力文档，因此共享设置中的「快捷短语混输」控件随能力出现，偏好关闭时仍由 host-api 阻止短语进入普通候选。

快捷短语能力与大写锁定标点分别验证：两项能力的宿主范围已经不同，旧的合取断言改为只验证标点，快捷短语由六平台矩阵完整验证，保留所有未接入宿主的关闭断言。Windows 能力及原生设置已由 #11364 合入 develop；整合时保留 Windows 的开启值，并把已有六平台矩阵的 HarmonyOS 期望改为开启。其他平台不变，尤其不将 HarmonyOS 归入桌面宿主，避免连带启用实体键盘专属能力。

## Alternatives considered

- 只在 HarmonyOS 设置页显示开关：键盘会话没有请求能力，开关会成为无效控件。
- 在 ArkTS 侧直接根据偏好筛选候选：候选排序、调频和跨宿主行为已经由共享引擎负责，宿主筛选会破坏统一契约。
- 直接采用任意一边的能力矩阵：冲突最少，但会丢掉 Windows 或 HarmonyOS 的已实现能力，因此合并两边的平台集合。

## Consequences

HarmonyOS 用户打开「快捷短语混输」后，普通组字会显示编码相同的快捷短语；关闭开关或总的「候选混输」时保持原行为，Shift+K 快捷短语模式不受影响。共享能力测试覆盖 HarmonyOS，host-api 原有偏好门控测试覆盖关闭/开启。此前同源签名 HAP 已在鸿蒙 USB 真机保留数据升级，并使用内置公开短语对混输开关和系统候选完成验收，改后截图已保留。刷新 develop 后仍须通过同源 HAP、quick 与 PR 检查；改前图缺口如实记录，不将构建结果当作新的设备验收。维护者已授权审查和合并符合条件的 PR，不再保留历史 Draft 限制。
