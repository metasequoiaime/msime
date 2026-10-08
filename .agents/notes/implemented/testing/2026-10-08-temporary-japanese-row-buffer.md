# Agent Note: 临时日文候选行缓冲复用

Status: implemented

## Problem

临时日文输入每次编辑都会把引擎候选通过 `WordItem::clone` 复制到会话列表。此前只复用了外层 `Vec`，每一行的拼音、词语和句子词列表仍会重新分配；预编辑字符串也会被临时 `String` 重新构造。

## Decision

复用候选刷新共用的逐字段行复制函数，按行 `clone_from` 并按长度截断或补齐；预编辑直接写回已有的 `String` 缓冲。临时日文候选的内容和回退候选行为保持不变。

## Alternatives considered

继续使用 `clear` 加 `extend(...cloned())` 只能复用外层向量，仍会为每个候选行的字符串重新分配；重新构造整个列表也无法消除这些按键级分配。

## Verification

新增刷新分配回归测试：相同候选再次刷新分配数为零。目标测试、完整引擎测试、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

临时日文每次按键刷新可以复用行内字符串和预编辑容量，避免候选复制路径上的重复堆分配。
