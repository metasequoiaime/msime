# Agent Note: 日文罗马字转换借用小写输入

Status: implemented

## Problem

日文罗马字转换的输入来自 `JapaneseRomajiScheme::build_request_into`，其中 `raw_input` 已经是 ASCII 小写。`convert_romaji` 仍无条件调用 `to_ascii_lowercase()`，为每次逐键刷新复制一份完全相同的输入。分配计数显示 `convert_romaji("nihongo")` 基线为 4 次分配，其中结果构造之外多出 1 次小写副本。

## Decision

`convert_romaji` 先检查输入是否含 ASCII 大写；全小写输入以 `Cow::Borrowed` 继续扫描，只有含大写时才创建规范化的拥有字符串。罗马字匹配、待处理尾部和大小写输入行为保持不变。全小写回归测试钉住分配预算从 4 次降到 3 次。

## Alternatives considered

- **始终调用 `to_ascii_lowercase`**：实现最短，但每次日文按键都会为已规范化的小写输入重复分配。
- **让调用方保证小写并删除转换层规范化**：可省掉检查，但 `convert_romaji` 是公开转换函数，其他调用方仍可能传入大写；会把行为契约转移给调用方。
- **复制到复用缓冲**：能减少后续容量增长，但需要跨调用保存额外状态；单次借用扫描已经足以消除无意义副本。

## Consequences

常见小写日文输入少一次堆分配；带大写的输入仍按原路径规范化，分配和结果保持兼容。函数多做一次 ASCII 字节扫描，扫描成本与输入长度线性且远小于原先分配与复制成本。结果字符串自身的增长分配仍存在，由现有转换逻辑负责。

请求构造直接写入已有 reading，完整性检查只扫描不收集结果，见[罗马字流式扫描](2026-10-09-japanese-reading-stream.md)。本篇的规范化与公开拥有型结果构造契约继续有效。
