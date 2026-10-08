# Agent Note: 个人上下文重排复用候选行

Status: implemented

## Problem

个人上下文重排先分配排序索引，再复制整份 `WordItem` 列表；候选行中的拼音、文字和句子词会在每次重排时重新分配。

## Decision

评分阶段把最多 16 行的排序保存为栈上索引，并在会话已有的候选行缓冲中先复制原始行，再按索引原地交换。未重排时继续返回 `None`，学习仍使用重排前的候选顺序，固定首选的保位规则不变。

## Alternatives considered

继续返回深拷贝的 `Vec<WordItem>` 实现简单，但会在每次重排时重复分配行内字符串；只复用外层 `Vec` 仍无法避免这些字符串分配；改变引擎候选的所有权会扩大会话和引擎之间的边界。

## Verification

新增排序索引零分配测试和会话刷新回归测试；旧实现的会话刷新无法保持行内字符串地址，新实现保持地址稳定。完整 `msime-engine` 单元与 golden 测试、Clippy、Rustfmt 和 `git diff --check` 通过。

## Consequences

个人上下文重排不再为排序结果深拷贝候选字符串；混排和固定位置阶段仍按既有规则处理候选。
