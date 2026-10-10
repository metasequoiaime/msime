# Agent Note: 笔画拥有型查询延后页缓冲

Status: implemented

## Problem

`StrokeScheme::candidates` 在读取 SQLite 结果前固定为精确查询预留 200 条、为补全查询预留 100 条中间行。输入没有命中时，这些页缓冲仍会申请；通配查询也会为精确和补全两组空结果付出同样的容量成本。会话的 `candidates_into` 路径已经复用字典的首行延后预留逻辑，但拥有型入口没有复用这一边界。

## Decision

拥有型查询直接从 `StrokeQueryBuffer::default` 开始。`lookup_into`、`lookup_completions_into` 和 `lookup_pattern_into` 在首条有效行读取后仍按原上限一次预留，因此密集命中继续保留原容量提示，空页不创建固定页缓冲。候选内容、去重、排序、键和值字段以及公开返回类型保持不变。

## Alternatives considered

- 查询前继续按 200、100 预留：命中路径少一次延后判断，但所有未命中笔画都会申请固定页存储。
- 按每一行自然增长：空页可以不申请，但密集命中会失去字典查询已有的上限提示并增加扩容次数。
- 修改复用型 `candidates_into`：该路径已经由语言词典的延后预留覆盖，扩大范围会增加会话缓冲风险而不减少拥有型入口的空页申请。

## Verification

新增预热后的精确空查询和通配空查询分配回归；旧实现分别为 5、8 次分配，修复后为 3、6 次，去除了两组固定页缓冲。笔画 scheme 测试 11 项通过；提交前继续运行引擎全量单测、golden、Clippy、Rustfmt、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

没有候选的拥有型笔画查询不再为 200 条精确槽位和 100 条补全槽位申请存储；有效首行仍按原查询上限准备容量。分配计数只反映 Rust 容器和查询路径，不等于 SQLite 内部或系统 RSS，也不直接推导端到端输入延迟。
