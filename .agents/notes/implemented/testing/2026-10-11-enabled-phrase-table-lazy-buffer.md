# Agent Note: 短语表空启用列表延后缓冲

Status: implemented

## Problem

`enabled_phrases` 按 `MAX_ENABLED_PHRASES` 创建输出向量。启用列表为空、插件缺失或全部插件载入失败时，结果为空但仍保留未使用的固定容量。

## Decision

输出向量从空容器开始，首条有效短语加入前再按原上限执行 `reserve_exact`。有效短语表继续一次预留 `MAX_ENABLED_PHRASES`，启用顺序、行上限和加载失败语义不变。

## Verification

空启用列表的容量断言先在基线因固定预留而失败，修复后通过；相关插件测试、client-core 全量测试、Clippy、Rustfmt、差异检查和 quick 门禁覆盖提交。

## Alternatives considered

- 继续按 `MAX_ENABLED_PHRASES` 预留：实现简单，但空结果会保留未使用容量。
- 让向量逐行自然增长：空结果成本最低，但有效短语表会增加扩容次数。

## Consequences

空短语表不再分配行缓冲；首条有效短语出现时仍一次预留原容量，合并结果和限制保持不变。
