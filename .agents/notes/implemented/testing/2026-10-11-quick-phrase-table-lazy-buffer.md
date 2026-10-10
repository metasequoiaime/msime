# Agent Note: 宿主快速短语表延后缓冲

Status: implemented

## Problem

`crates/engine/src/local/quick_phrase.rs` 的 `usable_quick_phrase_table` 在过滤宿主表前按输入长度预留容量。表中行全部无效时，结果为空却仍会申请最多 `TABLE_LIMIT` 行的存储。

## Decision

过滤容器从空 `Vec` 开始，首条有效行加入前再按原来的 `table.len().min(TABLE_LIMIT)` 执行 `reserve_exact`。有效表继续保留原容量提示、稳定排序和上限行为，全无效表保持零容量。

## Alternatives considered

- 保留过滤前预留：命中路径少一次首次插入判断，但无效表会为不会返回的行分配存储。
- 按每条有效行自然增长：可降低稀疏表的容量，但会引入多次扩容并改变已有命中结果的容量契约。

## Verification

新增全无效宿主表的零容量回归；旧实现容量为 3，修复后为 0。随后运行快速短语模块、引擎全量、Golden、Clippy、格式、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

全无效的宿主快速短语表不再申请结果缓冲；首次有效行仍按输入上限一次预留，因此有效结果的排序、截断和容量行为保持不变。
