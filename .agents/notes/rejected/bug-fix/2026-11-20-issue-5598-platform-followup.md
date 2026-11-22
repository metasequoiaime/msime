# Agent Note: 跟踪平台 issue #5598

Status: rejected — issue #5598 已由 #5754 修复并关闭，无需单独跟踪

## Problem

issue #5598 报告 macOS 0.52.0 设置第二释义语言（日文）后，候选的英文和日文释义都不显示，关闭在线服务只用随包词库时也一样，而随包词库里确有「国家」等词的两种译文。

该问题已由 #5754 修复，issue 于 2026-10-08 随 #5754 合并关闭。根因是 `InputController.mm` 用 `"\n"` 把同一候选的多语言释义连成多行，共享会话的 `msime_client_apply_translations` 拒收含控制字符的释义，导致整页释义被丢弃；#5754 在交给会话前把行分隔换成 U+2028，读回时再换回 `"\n"`。决策见 [macOS 释义行分隔](../../implemented/bug-fix/2026-10-08-macos-gloss-line-separator.md)。

## Proposal

为 #5598 建立独立的 Draft 跟踪 PR，保留报告的平台边界，补齐最小复现、责任路径、代码回归和目标平台验收后再宣称完成。

## Alternatives considered

- **直接复制相邻平台修复**：可能快速覆盖相似症状，但没有该 issue 的独立复现和验证，容易把无关改动带入目标平台，因此不采用。
- **只在 issue 下追加调查评论**：无需创建分支，但不能形成可审查的交付边界，因此不采用。

## Acceptance criteria

- PR 正文关联 #5598，并记录平台、复现和验证边界。
- 责任路径确认后加入代码、回归和平台验收。

## Risks

跟踪 PR 本身不修复问题；若 issue 已由其他 PR 解决，跟踪 PR 会成为过期提案。
