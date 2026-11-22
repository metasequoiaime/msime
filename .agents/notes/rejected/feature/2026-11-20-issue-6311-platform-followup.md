# Agent Note: 跟踪平台 issue #6311

Status: rejected — issue #6311 已由 #6639 的 legacy .deb 解决并关闭，无需单独跟踪

## Problem

issue `#6311`（linux）报告发布页的 arm64 `.deb` 依赖 glibc ≥ 2.35、libstdc++ 11、WebKitGTK 4.1、Fcitx5 等，在 UOS 20 / Debian 10（glibc 2.28、fcitx 4）上无法安装。#6639（`Refs #6311`）已合入：Release Linux 会同时发布在 Debian 10 容器里构建、只含 IBus 宿主的 `msime-linux-legacy_<版本>_<架构>.deb`（amd64 与 arm64），面向 glibc 2.28 到 2.34 的 Debian 系系统；它接替了未合入就关闭的 #6455。#6756 更正了 `platforms/linux/README.md` 里 legacy 包的说明。issue 于 2026-10-11 以 completed 关闭，fcitx 4 前端明确不在近期计划内。

## Proposal

为该 issue 建立独立的跟踪 PR，补齐最小复现、责任路径、代码回归和目标平台验收后再判断是否完成。

## Alternatives considered

- **直接复用相邻平台修复**：可能更快覆盖相似症状，但缺少该 issue 自己的复现和验证，因此当时不采用。
- **只在 issue 下追加调查评论**：无需分支，但形不成可审查的交付边界，因此当时不采用。

## Acceptance criteria

- PR 关联 `#6311`，并记录平台、复现和验证边界。
- 责任路径确认后加入代码、回归和平台验收。

## Risks

跟踪 PR 本身不交付修复；在问题已由 #6639 解决后继续保留，会让已关闭的 issue 看起来仍在进行中。
