# Agent Note: 跟踪平台 issue #10595

Status: rejected — issue #10595 已由 #10708 实现并以 completed 关闭，无需另开跟踪 PR

## Problem

平台 issue `#10595`（ios）的标题是「[iOS] 语音标点：接上 voice_input.punctuation_mode」：Android 已在 #10599（对应 issue #10558）加入共享偏好 `voice_input.punctuation_mode`，iOS 只有豆包专用的「自动添加标点」，本地模型、系统识别和润色结果里的标点无法按偏好处理。

提议时写的「仓库 PR 索引中没有找到 `Fixes` 或 `Refs` PR 关联它」不成立：PR #10708（feat(ios): 接入语音标点偏好）正文写明 `Fixes #10595`，2026-11-06 合并进 develop，issue 随之以 completed 关闭。#10708 合并时，正文里需要真实语音来源的上屏检查（K1 到 K8、A1 到 A5）仍未勾选。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#10595`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。若之后发现 iOS 语音标点在真实识别下与 Android 不一致，应新开 issue，而不是复用这条跟踪记录。
