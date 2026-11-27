# Agent Note: 跟踪平台 issue #10134

Status: rejected — issue #10134 已由 #10153 修复并关闭，无需单独跟踪

## Problem

issue #10134（Linux，0.14.0）报告点「检查词库更新」后下载一直停在 0%，直连、VPN、TUN 模式都一样。原因在共享 Rust 层 `crates/client-providers/src/dictionary_updates.rs`：连接没有数据时要约 30 分钟才换下一个下载源，不能断点续传，不使用设置里填的下载镜像，进度也到不了总量。#10153 已合并（2026-10-29），停滞一分钟内换源，支持 HTTP Range 续传和用户镜像，进度能走到 100%，下载途中关掉自动更新即停止；issue 随之以 completed 关闭，维护者在 issue 下说明了未覆盖的部分（下载源在用户网络里本身的速度，可用镜像或 #9529 的手动下载清单）。决策记录见 [词库更新下载停滞](../../implemented/bug-fix/2026-10-29-dictionary-update-stalled-download.md)。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#10134`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
