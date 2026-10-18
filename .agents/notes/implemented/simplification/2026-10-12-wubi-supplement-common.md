# Agent Note: 86/98 五笔补充表共用编码流程

Status: implemented

## Problem

86 和 98 五笔补充表分别复制了单字全码提取、词组编码、语料最大权重、同码候选排序、权重封顶及唯一四码统计。规则修正时两个版本容易漂移。

## Decision

`crates/dict-builder/src/wubi_supplement.rs` 统一这些与版本无关的步骤，词组取码委托给 `crates/engine/src/wubi/code.rs` 的共用规则。86 和 98 模块仍各自筛选候选词、解析来源表、记录来源标记、处理基本区过滤，并保留各自报告和生成格式。旧的 `wubi86_supplement` 辅助函数通过重导出保持 `wubi86_helpcode` 的调用边界。

## Alternatives considered

- 让 98 继续直接依赖 86 模块：会把版本无关规则和 86 的来源常量、报告耦合在一起。
- 合并两个补充表的 `Entry` 和报告：两者来源字段及额外统计不同，统一模型会损失语义并扩大改动。

## Consequences

词组编码和排名规则只有一个实现点；两个版本的来源过滤和输出契约保持不变。两套原有测试继续覆盖权重、编码、去重、不可推导字符和输出合并。

候选来源和长度范围扩展见[五笔多字词覆盖](../feature/2026-10-18-wubi-vocabulary-coverage.md)；此处的共用编码与排名边界继续成立。

构建时重新派生与个人学习共用取码规则的后续决定见[词库持续补全](../../implemented/feature/2026-10-18-dictionary-completion.md)；本篇的来源筛选及权重约束继续适用。
