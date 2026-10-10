# Agent Note: emoji 目录多前缀结果延后缓冲

Status: implemented

## Problem

`local::emoji::read` 同时读取 emoji 和颜文字的多个编码前缀时，按 `limit * prefix_count` 预留结果行。所有前缀未命中、行被 `NULL` 字段过滤或结果全部重复时，结果向量为空却仍占用整页容量；这些查询会在每次输入刷新中执行。

## Decision

结果向量从空容器开始。首条有效、去重后的文字准备写入时，再按原 `limit * prefix_count` 一次 `reserve_exact`。排序、跨前缀去重、`sort_order`、limit 截断和公开候选字段保持不变；有效命中继续保留原容量提示。

## Alternatives considered

- 查询前继续预留整页：命中路径少一次分支，但所有未命中多前缀查询都会申请固定容量。
- 每行自然增长：空页成本最低，但密集目录页失去原上限提示并增加扩容次数。
- 先按每个前缀分别收集：需要额外页容器，提升峰值并改变当前跨前缀去重顺序。

## Verification

新增真实 SQLite 目录夹具的空前缀回归；旧实现容量为 10，测试先失败，修复后容量为零。后续提交前运行引擎全量单测、golden、Clippy、Rustfmt、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

空 emoji/kaomoji 多前缀查询不再为结果页申请固定槽位；命中时仍按原提示容量准备存储。容量只描述 Rust 结果向量，不代表 SQLite 内部或系统 RSS，也不直接推导端到端延迟。
