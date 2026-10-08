# Agent Note: 会话候选刷新复用行缓冲

Status: implemented

## Problem

每次刷新混合候选时，输入会话都把引擎或前缀候选复制到新的 `Vec<WordItem>`；已有的 `mixed_candidates` 和个人重排缓冲被丢弃，行内字符串也随 `Clone` 重新分配。

## Decision

在刷新开始时取出已有候选缓冲，把每个 `WordItem` 的字符串和句子词列表通过 `clone_from` 写回已有行，按长度截断或补齐。个人重排缓冲优先作为下一轮解码缓冲，混合候选的顺序和固定位置处理保持不变。

## Alternatives considered

继续使用 `to_vec` 或 `Vec::clone` 会为每次按键建立新的行和字符串存储；只复用外层 `Vec` 但让派生 `Clone` 重新分配字符串无法消除主要开销；改变引擎候选的所有权会扩大会话和引擎边界。

## Verification

新增候选行缓冲分配回归测试：已有缓冲复制相同大小的合成行从 6 次分配降为 0 次。会话测试、完整引擎测试、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

常规候选刷新复用外层和行内字符串容量，减少按键路径的堆分配；候选内容仍是独立拥有的数据，源列表不会被修改。
