# Agent Note: 跟踪平台 issue #6305

Status: rejected — issue #6305 已由 #6330 修复并关闭，无需单独跟踪。

## Problem

平台 issue #6305（linux）的标题是「[Bug]: ubuntu 22.04 install error」，提出本提案时它已关闭。提案依据的判断是仓库 PR 索引中没有独立的 `Fixes` 或 `Refs` PR 关联它，这个判断不成立。

issue 报告 Ubuntu 22.04 装不上 `.deb`：包的 Depends 要求 `fcitx5 (>= 5.0.20)`，而 22.04 的 apt 只有 5.0.14。#6330 把 `fcitx5` 从 Depends 移到 Recommends，并在打包时去掉 `dpkg-shlibdeps` 推出的 `libfcitx5*` 依赖，让 22.04 可以安装并走 IBus 宿主；该 PR 以 `Fixes #6305` 合并，issue 于 2026-10-09 关闭，决定记录在 [linux-deb-fcitx5-recommends](../../implemented/bug-fix/2026-10-09-linux-deb-fcitx5-recommends.md)。之后 #6457 把依赖改成 `ibus (>= 1.5.20) | fcitx5 (>= 5.0.20)` 二选一，22.04 仍由 IBus 满足这一项，见 [linux-input-framework-either-dependency](../../implemented/bug-fix/2026-10-09-linux-input-framework-either-dependency.md)。

## Proposal

建立独立 Draft PR 作为该 issue 的交付跟踪入口。先保留报告原始范围和平台边界，补齐最小复现、版本与环境证据，再按确认的责任路径加入代码、行为回归和目标平台验收。历史 issue 也保留一条可审查的 PR 记录，避免用相邻修复推断它已经完成。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 `#6305`，并记录该 issue 的平台、复现和验证边界。
- 责任路径确认后，在本 PR 或后续同一 issue 的实现切片中加入代码、回归和平台验收。
- 历史已关闭 issue 也能追溯到对应 PR，不把相邻 PR 当作本 issue 的完成证据。

## Risks

当前 PR 只建立交付跟踪，不宣称已经修复该 issue。未经平台复现的改动可能影响其他宿主或用户设置。
