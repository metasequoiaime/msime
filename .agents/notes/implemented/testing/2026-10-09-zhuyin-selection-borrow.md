# Agent Note: 注音候选选择借用行字段

Status: implemented

## Problem

注音候选列表选择一行时，`select` 先克隆完整 `ListCandidate`，随后关闭列表并清空原行。`text` 和 `key` 的两次字符串复制在选中结果即将转移到 pin 的路径上没有用途。

## Decision

`select` 先检查下标，再用 `swap_remove` 取走选中的 `ListCandidate`，把其中的 `text` 和 `key` 所有权直接移入 `Span`。列表会在同一调用中关闭并清空，未选行的顺序不会对外可见；无效下标仍保持列表和状态不变。

## Alternatives considered

- **继续克隆候选行**：改动最小，但每次选择都为两个字符串分配并复制内容。
- **只借用字符串再复制到 pin**：仍无法跨过列表清空的所有权边界，不能消除分配。
- **保留候选并延后清理**：需要改变列表生命周期，且会留下无用行存储，超出本切片范围。

## Verification

新增成功选择的指针转移回归，确认 pin 直接接收候选行的两个字符串；新增越界选择回归，确认列表仍保持打开且内容不变。注音 scheme 37 项测试通过，随后运行引擎全量测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁。

## Consequences

注音候选选择不再复制 `text` 和 `key`，并保持提交、重转换和越界行为不变。`swap_remove` 会改变列表内部顺序，但列表立即清空，因此调用者看不到该顺序变化。
