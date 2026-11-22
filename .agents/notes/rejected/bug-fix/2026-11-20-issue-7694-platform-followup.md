# Agent Note: 跟踪平台 issue #7694

Status: rejected — issue #7694 已由 #6686、#7849、#7934 处理并于 2026-10-15 关闭，无需单独跟踪

## Problem

issue #7694 报告 Linux 0.11.0 的 IBus 宿主崩溃：`std::terminate: nlohmann::json_abi_v3_11_3::detail::type_error 306`，即对 null 视图调用 `value()` 导致 abort。输入模式提示回调的这条路径已由 #6686 修复；#7849 补了真实 GLib 主循环下的原生生命周期回归测试，#7934 在 `process_key` 的快捷键读取处加了视图防护，两者都以 `Refs #7694` 合并（线上样本没有可匹配的 Build ID，栈未能符号化到具体回调），issue 随后于 2026-10-15 以 completed 关闭。#9590 的遥测分析确认这类崩溃在 `linux-v0.12.0` 中已修复。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#7694`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
