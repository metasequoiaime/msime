# Agent Note: 跟踪平台 issue #10723

Status: rejected — issue #10723 已由 #10788 修复并关闭，无需单独跟踪

## Problem

issue #10723 报告 Linux 的 `ibus-engine-smoke` 与 `fcitx5-native-test` 不带参数的默认分支（语音按住、取消、组合键等用例所在处）在 CI 里从不运行，手动运行时又会在语音段之前被与语音无关的断言挡住，导致宿主级语音用例改坏也不会变红。

#10724 先补了 Linux 资源集成 job，但资源宿主门禁只跑了两个 `--voice-hotkey` 分支，没有满足 issue 的期望，issue 因此重新打开。#10788（2026-11-04 合并，正文 `Fixes #10723`）让资源宿主门禁真正运行 `ibus-native-context` 和 `fcitx5-native-context`，修正了 Linux 候选拼音行光标缺陷、`typing_statistics::batch_tests` 的测试隔离问题以及多条过时或写错的用例，issue 随之以 completed 关闭。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#10723`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
