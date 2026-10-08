# Agent Note: 词网格终点路径的小列表去重

Status: implemented

## Problem

词网格终点默认只保留 `beam = 32` 个假设，神经重排也只请求有限的 n-best 路径，但终点句子去重仍为每次解码建立 `HashSet` 和重复索引数组。

## Decision

终点路径不超过 64 条时，直接扫描已保留句子并原地压缩，再应用 `take` 上限；更大的自定义 `beam` 继续使用哈希表回退。去重抽成 helper，保留原有路径顺序和重复句子的首条路径。

## Alternatives considered

所有终点路径都改用线性扫描会让调用方传入大 `beam` 时退化为平方复杂度；只限制集合容量仍会保留临时堆状态。

## Verification

新增 32 条终点路径的零分配测试，确认去重后保留 12 条路径；既有词网格排序、n-best 上限和重复句子测试继续覆盖行为。

## Consequences

64 条以内的去重时间复杂度为平方级，但默认 beam 有明确小上限；更大的自定义 beam 继续走哈希表路径。
