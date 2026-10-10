# Agent Note: 混排读法结果缓冲延后预留

Status: implemented

## Problem

`read_readings` 在开始查询每组 emoji 或颜文字读法前，固定为 `MIXED_FETCH_PER_READING` 条结果申请容量。无命中、所有行被 `accept` 拒绝或数据库只有不匹配编码时，混排结果为空，却仍保留这块缓冲。

## Decision

混排结果从空 `Vec` 开始。首条通过编码核对、`accept` 和跨读法去重的行加入前，再按原来的 `MIXED_FETCH_PER_READING` 执行 `reserve_exact`；后续读法继续复用这块容量，达到总量上限和每种读法上限的行为保持不变。

## Alternatives considered

- 查询前继续预留整页：命中路径少一次判断，但每次空混排查询都承担固定容量成本。
- 按每行自然增长：空结果成本最低，但多读法命中时会增加扩容次数并失去原容量提示。
- 先收集再按结果数分配：需要额外的中间存储，且不能避免空查询的结果容器分配。

## Verification

新增空读法结果容量回归；旧实现测试先失败并显示容量为 16，修复后容量为 0。相关 emoji 模块测试 14 项通过；提交前还会运行引擎库、golden、Clippy、格式、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

没有可用混排行时不再申请结果页存储；首条有效行仍按原上限准备容量，混排排序、去重、关键词和截断语义不变。容量检查只反映 Rust 结果向量，不代表 SQLite 内部或端到端内存占用。
