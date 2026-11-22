# Agent Note: 跟踪平台 issue #7695

Status: rejected — issue #7695 已由 #7735 修复并关闭，无需单独跟踪

## Problem

issue `#7695` 报告 iOS 键盘扩展 0.51.5 出现大量 RUNNINGBOARD `0xdead10cc` 终止崩溃（signal 9），即扩展挂起时仍持有 App Group 文件锁。PR `#7735`（2026-10-13 合并，正文 `Fixes #7695`）让 `DictionarySnapshotWorker.stop()` 在取消任务后立即释放 `worker.lock` 租约，issue 随之关闭。决定记录见 [词库快照租约释放](../../implemented/bug-fix/2026-10-14-ios-snapshot-worker-lock-release.md)。

## Proposal

为 `#7695` 另开一个独立 Draft PR 作为交付跟踪入口，补齐复现、责任路径、回归测试和 iOS 平台验收。

## Alternatives considered

- **只依赖 issue 关联的修复 PR**：最强理由是无需另开分支，但当时还不确定修复 PR 是否覆盖全部终止路径，因此当时未采用。

## Acceptance criteria

- 跟踪 PR 关联 `#7695`，记录平台、复现和验证边界。
- 责任路径确认后加入代码、回归和平台验收。

## Risks

跟踪 PR 与已合并的修复 PR 重复，会让已关闭的 issue 看起来仍未完成。
