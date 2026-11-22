# Agent Note: 跟踪 macOS issue #9235

Status: rejected — issue #9235 已作为 #9094 的重复关闭，相同根因由 #9186、#9495、#9636 修复，无需单独跟踪。

## Problem

issue #9235 报告 macOS 26 上用 Caps Lock 切换之后一直输出大写字母，期望切换后默认回到小写。维护者在 issue 下说明它与 #9094 是同一个 macOS Caps Lock 实体按键状态问题，以 #9094 为唯一跟踪 issue，并把 #9235 按重复（DUPLICATE）关闭。

#9094 的根因已分三次修复并合并：#9186 改读 HID flags 中独立的实体按住位，区分 Caps Lock 的实体按住与锁定状态；#9495 为不报这个位的外接键盘从 HID 读取实体按住状态；#9636 让 HID 监听只在键盘不报按住位时启动并先取得授权，以 `Fixes #9094` 关闭了 #9094。

## Proposal

原提案为 #9235 单独建一个 Draft PR 作为交付跟踪入口。issue 已作为重复关闭，修复和验收都记在 #9094 及上述 PR 中，本提案不再执行。

## Alternatives considered

- **保留 Draft 跟踪 PR**：理由是让每个历史 issue 都有对应 PR，但重复 issue 的修复证据已在 #9094 链路上，再留一个没有代码的 Draft 只会成为过期提案，因此不采用。
- **直接删除笔记**：理由是改动最少，但会丢掉这条提案为何不做的记录，因此改为归档到 rejected。

## Acceptance criteria

- 笔记位于 `rejected/bug-fix/`，Status 写明 #9235 是 #9094 的重复以及实际修复的 PR。
- 不再为 #9235 保留单独的跟踪 PR。

## Risks

如果报告者在 #9636 合并后的版本上仍能复现，应重新打开 #9094 或另开新 issue，而不是恢复这条提案。
