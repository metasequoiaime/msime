# Agent Note: 手机输入页移除外层更多分组

Status: implemented

## Problem

HarmonyOS 手机的共享输入页和 iOS 原生输入页把输入通用设置单独放进末尾「更多」分组。Android 原生 `TypingPage` 的对应页面只有「语言与方案」「中文」「辅助码」「翻译」等直接分组，三端手机的输入设置入口因此不一致，常用设置也需要额外找到一个分组。

## Decision

HarmonyOS 手机不再启用 `InputSharedSettingsSection` 的外层 `foldIntoMore`；iOS 把原「更多」组的行并入「中文」组。Android 已经没有该外层分组，三端手机都直接展示输入设置。HarmonyOS 手机已经在「中文」组提供「中文字符集」，因此通用「输出」组不再重复显示「中文输出」单选；其他输出设置仍保留。方案卡片和候选翻译等组内针对具体低频选项的「更多选项」继续保留，避免把不同类型的设置混成一张长列表。

## Alternatives considered

**只删除「更多」标题而继续把内容藏在折叠里**：视觉上少一个分组，但用户仍要展开同一个「更多选项」，没有解决手机端的额外入口问题。

**删除输入页所有组内「更多选项」**：可以完全平铺页面，但会把资源包、候选翻译第二语言和低频混输开关等不同层级的设置混在主流程里，和其他手机设置页的低频项处理不一致，因此保留具体组内折叠。

## Consequences

鸿蒙和 iOS 手机输入页的通用设置现在可以直接浏览和修改，页面不再出现末尾「更多」分组；Android 原有布局保持不变，桌面布局也不变。重复的「中文输出」控件被隐藏，中文字符集仍由鸿蒙专用控件负责写入同一份偏好。测试同时锁定了共享页分组顺序、iOS 不出现「更多」分组和桌面不出现「更多」分组的现状。

这项决定取代[精简「输入」「快捷键」两页](2026-10-16-macos-input-settings-prune.md)中“鸿蒙手机整页收在一个「更多选项」里”的 HarmonyOS 输入页布局描述，并补上 iOS 原生页的对应布局；该笔记关于桌面输入页分组和其他页面折叠区的约束仍然有效。

## Verification

已运行 `pnpm exec vitest run tests/settings/language-card.test.tsx --reporter=dot --pool=threads --maxWorkers=1 --no-file-parallelism`，12 个用例全部通过；`pnpm --filter @msime/desktop typecheck` 和 `pnpm --filter @msime/harmony typecheck` 通过。HarmonyOS 逻辑测试全部通过；iOS 两个 Swift 文件均通过 `xcrun swiftc -parse`，iOS XCTest 和 Android 原生宿主检查待 CI/目标工具链运行。格式检查、差异检查和笔记校验通过，仓库 quick 门禁因本机磁盘空间不足未启动。
