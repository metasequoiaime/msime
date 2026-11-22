# Agent Note: 跟踪平台 issue #8088

Status: rejected — issue #8088 已由 #8188 修复并关闭，无需单独跟踪

## Problem

issue #8088 来自 Linux 用户的需求表单：Windows 版已有智能 ABC 双拼，希望 Linux 版的双拼方案也增加智能 ABC。#8188（`Fixes #8088`）在 Engine 中加入智能 ABC 内置键位表，让 Linux、Windows 和 macOS 都能选择、保存和展示该方案，Linux 端可在 IBus 菜单和 Fcitx5 轮换菜单中直接选择；该 PR 于 2026-10-16 合并，issue 随之以 completed 关闭。实现决定见 [桌面端智能 ABC 双拼](../../implemented/feature/2026-10-16-smart-abc-shuangpin.md)。

## Proposal

为 #8088 建立一个独立的 Draft PR 作为交付跟踪入口，补齐复现、责任路径、代码回归和 Linux 平台验收后再关闭跟踪。

## Alternatives considered

- **直接复制相邻平台修复**：最强理由是可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：最强理由是无需创建分支，但不能形成可审查的代码交付边界，因此不采用。

## Acceptance criteria

- 跟踪 PR 关联 #8088，并记录该 issue 的平台、复现和验证边界。
- Linux 双拼方案中可以选择智能 ABC，并有对应回归和平台验收。

## Risks

跟踪 PR 本身不含实现，issue 解决后若不及时收尾，会留下一篇与事实不符的过时提案。
