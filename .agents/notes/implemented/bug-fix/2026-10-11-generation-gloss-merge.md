# Agent Note: 同代次候选释义合并与空结果边界

Status: implemented

## Problem

在线候选更新已经保留前后同文词条的释义，但下一轮离线或部分在线结果经 `apply_translations` 整体替换后仍会丢失未回答词条的释义。Fcitx5 的任务种子只保护一个宿主，其他宿主仍会闪烁。空数组同时被宿主用于关闭翻译或切换目标语言时主动清理，不能一律当成无更新。

## Decision

共享运行时按当前候选代次合并非空结果，新结果覆盖同词旧释义，旧代次照旧拒绝；只保留当前候选范围内的释义，避免多次部分回包积累请求外条目。空数组仍主动清空，Android/iOS 宿主的异步空结果跳过应用，保留原有失效、渲染和后续在线调度；macOS 宿主恢复上游行为，不在本 PR 范围内。命令翻译的独立分支不变。

同代次回包只有在当前候选的释义新增或变更时才推进 `candidates_epoch`；重复同值与候选范围外的回包不改变视图，也不应让生命周期核心误报 `candidates_shown`。空数组仅在原有释义非空时推进计数器。

Fcitx5 移除当前页任务种子；翻译任务只合并本轮离线与在线结果。在线候选刷新前后的候选集合分别用 `HashSet<&str>` 判断，引用不跨越刷新。

合入 `23ba3a6d7` 后，AI 空回答把占位行从 `…` 改为 `—` 也走同一交集保留刷新；它不改变输入，不能顺便清掉本地候选释义。仍推进代次并拒绝旧结果，随后真实输入变化仍清理。

## Alternatives considered

- 继续用 Fcitx5 的当前页种子，改动局限于一个宿主；但同根因在 IBus、Windows 和移动宿主仍存在，逐宿主复制展示状态会继续漂移。
- 新增显式清理 ABI，让空结果统一是无更新，所有调用方更容易表达意图；但要扩展各平台桥接层，现有空数组清理调用也需迁移。本次保留接口，用结果边界区分主动清理与异步无答案。
- 所有回包直接扩展释义表，最少代码且空回包自然无更新；但会破坏现有主动清理语义，并让多次回包积累候选之外的条目。

## Verification

同代次部分回包与请求外词条的两条 Rust 回归先失败，修复后 Runtime 177 项、Host API 428 项通过。Fcitx5 原生云/AI 与 `ibus-empty-translation` 均先在异步空结果清掉译文的断言失败，修复后通过；主动关闭翻译仍能清理。IBus 完整冒烟在先行语音用例失败，翻译回归单独登记，未掩盖无关失败。

后续回归覆盖同值回包与候选范围外回包的 `candidates_epoch` 不变：断言先在重复同值处失败，修复后通过。生命周期核心以该计数器决定是否产生 `candidates_shown`，故此处在运行时阻止虚假的视图事件。

macOS 宿主连同其卡片用例整体恢复上游，本 PR 不再改动、也不声称验证 macOS；Linux 无 Apple 宿主与 Android JVM 工具链，两条源码接线契约验证 Android 与 iOS 的空结果守卫、离线记账和服务调度顺序，不替代原生运行验收。Windows、HarmonyOS 已有空结果守卫，不改其路径。

源码接线检查沿用仓库已有 `scripts/test-*.py` 契约入口，只钉住 Android 与 iOS 的空结果守卫、异步记账/调度及显式清理路径，不是替代平台测试的策略。优先用原生测试，但缺相应宿主时不能执行；不为了这次修复搭建新跨平台模拟层。合入上游后，AI 空座位保留释义回归先失败，复用交集刷新后通过，输入改变仍清理、旧代次仍拒绝。

## Consequences

共享合并覆盖所有宿主，现有主动清理无需迁移 ABI。代价是本 PR 的宿主必须在异步空结果处跳过应用，仍保留渲染和后续在线调度；macOS 宿主恢复上游后，其异步空结果边界本 PR 不保证修复，也未经 CI 验证。源码接线契约依赖现有控制流形状，重构时需与原生测试同步，Linux 与共享测试不等于其他平台真机验收。

## Related notes

[macOS 空译文视图同步补丁撤销](../../rejected/bug-fix/2026-10-11-macos-empty-gloss-view-sync.md) 与[按显示代次记账的 macOS 提案](../../rejected/bug-fix/2026-10-11-macos-empty-gloss-display-generation.md) 记录两轮 macOS 补丁均被否决；用户最终完全退出 macOS 宿主改动并恢复上游，不改变本篇共享合并和空数组契约。

[在线候选释义保留](../../implemented/bug-fix/2026-10-11-online-candidate-gloss-retention.md) 部分重叠：候选交集与代次规则保留，其 Fcitx5 种子方案由本决定取代。[重复在线查询](../../implemented/bug-fix/2026-10-11-fcitx-online-refresh-translation-flicker.md) 决定请求分发，本篇只决定释义应用。检索命中的[Windows 释义列与账号释义](../feature/2026-10-10-windows-gloss-columns-and-account-glosses.md)、[macOS 多语言分隔](2026-10-08-macos-gloss-line-separator.md) 处理宿主传输与上屏，不改变其格式和排序规则。

[释义偏好变化撤销旧代次回包](2026-10-13-host-api-translation-preference-generation.md) 规定配置变化时清空旧释义并推进代次；本篇的同代次合并只用于配置未变化的回包。
