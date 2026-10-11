# Agent Note: 空剪贴板历史加载延后缓冲

Status: implemented

## Problem

剪贴板历史加载把结果向量固定预留为 `MAX_ENTRIES`。持久化文件为空数组时没有任何条目，但仍保留未使用的容量。

## Decision

结果向量从空容器开始，首条去重后的有效历史条目加入前再按原上限执行 `reserve_exact`。空历史保持零容量，非空历史仍一次预留 `MAX_ENTRIES`；去重、排序、截断和文件错误语义不变。

## Verification

空历史容量断言先在基线因固定预留而失败，修复后通过；相关剪贴板测试、client-core 全量测试、Clippy、Rustfmt、差异检查和 quick 门禁覆盖提交。

## Alternatives considered

- 继续按 `MAX_ENTRIES` 预留：实现简单，但空历史会保留未使用容量。
- 让向量逐条自然增长：空历史成本最低，但非空历史会增加扩容次数。

## Consequences

空历史加载不再申请条目缓冲；首条有效条目出现时仍一次预留原容量，历史内容和容量上限保持不变。
