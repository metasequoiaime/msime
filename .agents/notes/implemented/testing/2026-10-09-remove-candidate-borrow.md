# Agent Note: 删除候选入口借用候选行

Status: implemented

## Problem

`InputSession::remove_candidate` 在检查候选来源和单字保护条件前无条件克隆完整 `WordItem`。不可编辑的临时英文或在线行会在删除被拒绝前复制词、拼音和其它字段。

## Decision

删除入口直接借用候选行，先完成来源和单字校验，再借用其方案、字典键和词文本调用词库删除接口。刷新候选仍在删除接口返回后执行，原有诊断和保护规则不变。

## Alternatives considered

- **继续克隆完整候选行**：实现简单，但拒绝路径为不会使用的字段分配内存。
- **仅在校验通过后克隆**：能减少拒绝路径分配，却仍复制删除接口可以直接借用的文本。
- **让词库删除接口保存候选行引用**：会扩大存储层的借用边界，并把会话刷新生命周期带入词库操作。

## Verification

新增不可编辑候选的删除回归测试：旧实现分配 2 个缓冲，借用后为 0。190 个会话测试、31 个 golden 测试、Clippy、Rustfmt、差异检查和 `bash scripts/verify-local.sh --quick` 均通过。

## Consequences

拒绝删除操作不再克隆完整候选行，成功操作只读取删除所需字段；候选来源判断、单字保护、词库删除、刷新和错误诊断保持不变。

相关决定：[固定位置入口借用候选行](2026-10-09-fixed-position-candidate-borrow.md)、[固定候选只保留必要的词文本](2026-10-10-pin-candidate-borrow.md)。
