# Agent Note: 九键原样上屏移动数字缓冲

Status: implemented

## Problem

九键 `CommitRaw` 在数字可原样上屏时先克隆 `self.digits`，随后通过取消命令清空组字。提交结果本身需要拥有这段数字，克隆后原缓冲不会再被使用。

## Decision

用 `mem::take` 移出数字缓冲，再直接调用内部 `clear_composition` 清除锁定、切分、首字母和撤销状态，最后把移动出的字符串交给 `KeyResult`。14 键有候选时仍走 `finish(0)`；没有候选时的原样提交、空闲拒绝和公开命令语义保持不变。

## Historical audit

已检索 `CommitRaw`、九键原样上屏、数字缓冲和取消路径笔记。[14 键原样上屏规则](../../implemented/feature/2026-10-10-fourteen-key.md)持有九键与 14 键的行为差异，本片只优化九键仍可原样提交的所有权路径；[九键选择借用候选行](2026-10-09-nine-key-selection-borrow.md)和[提交文本移动到结果](2026-10-10-commit-text-move.md)处理候选选择，不覆盖原始数字提交。

## Alternatives considered

- **继续克隆后调用 `Command::Cancel`**：行为最直观，但提交热路径保留一次必然无用的数字字符串分配。
- **给 `KeyResult` 增加借用字段**：会改变跨引擎调用边界的拥有型结果契约；移动已有字符串无需改变公开接口。
- **先清空会话再从快照复制数字**：仍需复制输入内容，并可能丢失原样提交应返回的字节；`mem::take` 同时保留文本和清理状态。

## Verification

新增九键原样提交分配回归：旧实现固定输入 `64426` 时为 1 次分配，在 0 次上限下失败；移动数字缓冲后为 0 次并通过。随后运行九键专项、完整引擎、golden、input-runtime、Clippy、格式、笔记校验和 quick 门禁。

## Consequences

九键无候选或显式原样上屏少一次数字字符串复制；取消后锁定、切分、首字母、撤销记录和种子状态仍按原规则清理。14 键结束组字和所有非九键方案不受影响，局部收益不能外推为整体输入延迟目标完成。
