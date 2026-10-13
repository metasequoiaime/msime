# Agent Note: Windows 原生设置窗口去掉「手写设置」跳转行

Status: implemented

## Problem

原生设置窗口的「手写输入」页有两行：「手写输入」打开手写识别板，「手写设置」拉起共享 Tauri 应用的 `settings:handwriting` 页。整项工作的目标是任何设置页都不再拉起共享应用。共享的 `HandwritingSettingsSection`（`packages/ui/src/settings/handwriting-settings-section.tsx`）按平台分支：iOS、Android、HarmonyOS、macOS 各有一组，其余（Windows 落在这里）只有「打开手写识别板」一行和一张写着「水」的静态预览。可调的项（识别等待时间、笔迹颜色、笔迹粗细）只在 HarmonyOS 手机上出现；手写面板的明暗外观 `handwriting_theme` 已在原生「主题」页。

## Decision

- 删掉 `build_hand_page` 里的「手写设置」行，保留打开手写识别板的那一行，并在函数前注明原因。
- 留下的那一行改用共享页的文案：分组「手写识别板」，行「打开手写识别板」，说明「使用鼠标或触控方式手写输入，自动识别候选汉字」，按钮「打开」（`OpenPanelButton` 的默认标签）。
- 删掉不再使用的 `shell_links::handwriting`，`all` 数组从 12 项减为 11 项；`tests/ui/settings_navigation.cpp` 没有引用它，不用改。
- README 里列举「以按钮跳到共享应用」的部分不再提手写的详细设置。

## Alternatives considered

- **在原生页里照搬静态预览** — 与共享页逐项一致；但预览是不可交互的示意图，不是设置，按目标只需要保证每个设置都能在原生窗口里改。
- **把 HarmonyOS 的识别延迟和笔迹设置也做到 Windows** — 多给几个选项；但共享页在 Windows 上隐藏它们，Windows 手写面板也不读这些字段，做了等于造出没有效果的开关。

## Consequences

- [macOS 手写设置入口](../bug-fix/2026-10-13-macos-handwriting-settings-entry.md)让共享 macOS 页也能打开识别复制面板，并按原生会话能力显示输入操作；本篇的 Windows 原生设置入口不变。

- 「手写输入」页不再有拉起共享应用的入口；打开手写识别板仍是面板，照旧由 `MSIME.exe` 提供。
- 托盘或其他启动方用 `handwriting` 路由打开设置时仍落在原生的「手写输入」页（路由别名未变）。

## Verification

- `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`），没有启动窗口。
