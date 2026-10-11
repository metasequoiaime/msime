# Agent Note: Windows 原生设置窗口在「帮助与反馈」页绘制帮助

Status: implemented

## Problem

原生设置窗口「帮助与反馈」页的「帮助」一行拉起共享 Tauri 应用的 `settings:help` 页。整项工作的目标是任何设置页都不再拉起共享应用。共享的 `HelpSettingsPage`（`packages/ui/src/settings/help-settings-page.tsx`）在 Windows 上走非 macOS、非移动端的分支：一段平台简介、「快速上手」、「基本功能」三段说明和「更多」分组里的「完整文档」，文案来自 `settings/platform-copy.ts` 的 Windows 分支；系统键盘设置按钮、iOS 的「允许完全访问」和 Android 的「输入权限」都不出现。

## Decision

- `build_feedback_page` 删掉「帮助」行，在反馈和诊断日志之后调用新的 `build_help_section`：分组「帮助」下放平台简介，「快速上手」「基本功能」各是一张只有说明文字的卡片（新的 `add_text_card`，对应 `GroupList` + `SettingsGroupBlock`），「更多」分组里「完整文档」一行用默认浏览器打开 `https://msime.app/docs/`，按钮文字「完整文档（网页）」与共享页相同。
- 文案逐字照搬共享代码；JSX 里换行的「首右 2.0」合成一个空格。
- 每段文字都登记到搜索索引，搜「快速上手」「Win + Space」之类能落到对应卡片。
- 删掉不再使用的 `shell_links::help`，`all` 从 11 项减为 10 项；`help` 路由仍映射到「帮助与反馈」页。README 同步。

## Alternatives considered

- **新增一个独立的「帮助」侧栏页** — 与共享应用的子页结构一致；但侧栏分组和顺序要与 `settingsNavGroups` 一致，那里没有单独的帮助页，加页会让两边不一致，测试里的页数也要改。
- **把 macOS 的问答卡片一并搬来** — 内容更多；但共享页只在 macOS 上显示它们，里面的快捷键（Option+Shift+H）和系统路径是 macOS 的，在 Windows 上是错的。

## Consequences

- 「帮助与反馈」页不再有拉起共享应用的帮助入口；「反馈」一行仍跳共享应用，留给后续步骤。
- 帮助文字写死在 `main.cpp`，`platform-copy.ts` 的 Windows 分支改了要同步改这里。

## Verification

- `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`），没有启动窗口，页面的实际排版未目视检查。
