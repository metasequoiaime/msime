# Agent Note: 跟踪平台 issue #9190

Status: rejected — issue #9190 已由 #9496 实现并关闭，无需单独跟踪

## Problem

issue `#9190`（iOS）请求增加类似讯飞的 14 键双拼键盘布局：用户在 iOS 0.52.0 上用 26 键打双拼容易按错，九宫格也不方便。该需求已由 PR `#9496`「feat(ios): 支持双拼十四键布局」实现，PR 正文写明 `Fixes #9190`，于 2026-10-24 合入后 issue 随之以 completed 关闭。实现记录见 [iOS 十四键双拼](../../implemented/feature/2026-10-24-ios-fourteen-key-shuangpin.md)：原生设置可选择 14 键双拼，默认保持 26 键，支持小鹤、自然码、微软和首道方案。本笔记原本为该 issue 另建一个 Draft 跟踪 PR，是在未核对 `#9496` 的情况下生成的。

## Proposal

为 `#9190` 建立独立 Draft PR 作为交付跟踪入口，再补复现、代码和平台验收。不采用：需求已由 `#9496` 完整交付，额外的跟踪 PR 没有可交付内容。

## Alternatives considered

- **保留 Draft 跟踪 PR 等待后续验收**：最强理由是 `#9496` 正文写明迁移后尚未完成系统编辑器重验、也尚未覆盖实体设备，但这类补充验收应针对具体缺陷另开 issue，不需要一个空的跟踪 PR。
- **直接关闭跟踪 PR 不留记录**：最强理由是最省事，但会丢失这次判断的依据，因此改为记录成 rejected。

## Acceptance criteria

- 本笔记位于 `rejected/`，Status 写明 `#9190` 已由 `#9496` 实现并关闭。
- 不再为 `#9190` 保留待实现的 proposed 笔记。

## Risks

无代码改动。若 14 键双拼后续出现问题，应按新 issue 单独处理，不复用本笔记。
