# Agent Note: 跟踪平台 issue #9407

Status: rejected — issue #9407 已由 #9442 修复并关闭，无需单独跟踪。

## Problem

issue `#9407` 报告 macOS 输入法 0.54.0 打开「候选显示音标」后，全拼输入 `pingguo` 时候选只显示「苹果 apple」，不显示音标，尽管 App 包内带有 `pronunciations/en-phonetic.db`。该 issue 已于 2026-10-24 由 PR `#9442`（`fix(macos): 回退读取随包音标资源`，正文 `Fixes #9407`）合并关闭：活动资源旁没有音标表时，候选音标查询回退读取输入法 bundle 内的 `pronunciations`，独立 InputMethodKit bundle 的打包流程同时补入该目录并在最终包检查中校验数据库与 NOTICE。本笔记原本建议为它另开 Draft PR 跟踪，但创建时 issue 已关闭且已有修复 PR，因此该跟踪提议不再成立。

## Proposal

原提议是为 `#9407` 建立独立 Draft PR 作为交付跟踪入口，补齐复现、责任路径、回归和平台验收。该提议不执行，修复与验证记录以 `#9442` 为准。

## Alternatives considered

- **保留 Draft 跟踪 PR**：最强理由是可以集中补做 `#9442` 中尚未完成的已安装输入法实机验收，但 issue 已由合并的修复关闭，长期挂着的 Draft 只会成为过时提议，因此不采用；若实机仍复现，应重新打开 issue 或新开 issue。

## Acceptance criteria

- 本笔记以 rejected 状态记录 `#9407` 已由 `#9442` 修复关闭，不再作为待办提议出现。

## Risks

`#9442` 合并时注明尚未在已安装的 macOS 输入法中验收实际候选音标；如果用户反馈仍不显示音标，需要基于新报告重新跟踪，而不是恢复本提议。
