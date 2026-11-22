# Agent Note: 跟踪平台 issue #8710

Status: rejected — issue #8710 已由 #8733 和 #8731 修复并关闭，无需单独跟踪

## Problem

issue `#8710`（macOS）报告：用户已在系统设置里移除水杉输入源，在设置应用里点击卸载仍会反复拉起系统设置，无法完成卸载。该 issue 已于 2026-10-18 以 completed 关闭：#8733 让卸载检查优先读取当前系统的第三方输入源列表，不再让旧 `com.apple.HIToolbox` 列表的残留误报 `input_source_listed`，决定见 [macOS 卸载优先读取当前第三方输入源列表](../../implemented/bug-fix/2026-10-19-macos-uninstall-stale-input-list.md)；#8731 让卸载前通过已安装输入法的只读 TIS 探针读取输入源实时状态，避免 `defaults export` 缓存造成同样的误报，决定见 [macOS 卸载使用实时输入源状态](../../implemented/bug-fix/2026-10-19-macos-uninstall-input-source-state.md)。两个 PR 都以 `Fixes #8710` 合并进 `develop`。

## Proposal

为该 issue 建立独立 Draft PR 作为交付跟踪入口，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和 macOS 平台验收。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#8710`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在同一 issue 的实现切片中加入代码、回归和平台验收。

## Risks

跟踪 PR 本身不包含修复；在 #8733 和 #8731 已经交付的情况下继续保留，会让已解决的问题看起来仍未处理。
