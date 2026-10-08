# Agent Note: 粤拼候选行缓冲复用

Status: implemented

## Problem

粤拼候选刷新会重新构造 `WordItem` 向量，旧候选行和其中的字符串容量随向量释放；重复刷新会在候选行没有变化时产生不必要的堆分配。

## Decision

粤拼刷新取出会话已有候选向量，逐字段写回现有行并按结果长度截断或补齐，保留候选行和字符串容量。粤拼词典是只读的，`Requery` 直接保留当前快照；其他刷新仍查询词典并复用已有行。

## Alternatives considered

继续让 registry 返回新的 `Vec<WordItem>` 会同时丢失外层向量和行内字符串存储；只复用向量容量仍会为每个候选字符串重新分配。把词典结果借用到会话状态会跨越 provider 和会话的所有权边界。

## Verification

新增字段级行复用零分配测试和粤拼重查询指针复用测试；完整引擎测试、Clippy、笔记检查和 `verify-local.sh --quick` 均通过。

## Consequences

粤拼候选刷新可以保留已有候选行的存储，重复重查询不再重建请求或读取只读词典，同时保持候选内容、顺序和元数据不变。
