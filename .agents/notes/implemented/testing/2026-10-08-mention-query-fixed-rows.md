# Agent Note: @ 查询使用固定行缓冲

Status: implemented

## Problem

`query_mentions` 的 exact 和 prefix 结果最多各受 18 行上限约束，但每次查询仍为两组借用行创建临时 `Vec`，启用地点补全时还会复制一份已有名称切片。

## Decision

用两个固定 18 槽的栈上引用数组收集 exact/prefix 行，再构造最终匹配；地点补全直接从固定名称数组读取已有名称。保持 exact 优先、prefix 截断、地点补全顺序和最终权重。

## Alternatives considered

继续使用两组动态 `Vec` 会为受上限约束的查询保留临时堆状态；把所有结果直接写入最终行结构会让地点补全前的借用和字段组装更复杂。固定引用数组只承载借用行，最终输出仍按原接口创建。

## Verification

新增查询行收集零临时分配测试；完整本地 mention 测试、完整引擎测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖实现。

## Consequences

每次 `@` 查询不再为 exact/prefix 行或已有地点名称创建临时堆容器；结果内容、顺序和 18 行上限保持不变。
