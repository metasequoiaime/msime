# Agent Note: 跟踪平台 issue #10597

Status: rejected — issue #10597 已由 #10710 修复并以 completed 关闭，无需另开跟踪 PR

## Problem

平台 issue `#10597`（windows、macos、linux）的标题是「[桌面] 语音标点：共享设置页与 Windows、macOS、Linux 接上 voice_input.punctuation_mode」，报告三个桌面宿主不读取共享偏好 `voice_input.punctuation_mode`。

#10710（「feat(desktop): 接入语音标点偏好」，正文写 `Fixes #10597`）在三端最终识别文本（含润色）上屏前按该偏好处理，并在 Windows 原生设置页与 Win7 设置页加了选项；它于 2026-11-06 合并，issue 随之以 completed 关闭。#10710 正文里记着的 macOS、Windows、Linux 真机复验缺口属于该 PR 自己的验收记录。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#10597`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
