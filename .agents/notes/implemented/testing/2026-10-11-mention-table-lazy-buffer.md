# Agent Note: 宿主提及表延后缓冲

Status: implemented

## Problem

`crates/engine/src/local/mention.rs` 的 `usable_mentions` 在验证宿主提及表之前，按输入长度预留结果 `Vec`，长表还会预留去重 `HashSet`。输入全部无效时，两个过滤容器都不会使用却仍会分配。

## Decision

短表和长表的结果容器都从空 `Vec` 开始，首条有效行出现时再按原输入上限预留。长表的文本去重集合也延后到首条有效行时预留；无效表不建立任何堆状态。有效行的校验、去重、启用顺序、上限和容量提示保持不变。

## Alternatives considered

- 保留过滤前的两个预留：命中路径少一次判断，但无效宿主输入会承担不会使用的结果和去重存储。
- 只延后结果 `Vec`：全无效长表仍会为 `HashSet` 分配，无法消除空过滤请求的固定成本。
- 取消长表的容量提示：可以降低稀疏表容量，但会引入多次扩容并改变现有有效结果容量契约。

## Verification

新增长表全无效输入的零分配回归；旧实现产生 2 次分配，修复后为 0。随后运行提及模块、引擎全量、Golden、Clippy、格式、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

全无效宿主提及表不再申请结果或去重缓冲；首次有效行仍按原上限准备容量，因此有效结果的顺序、去重和截断行为保持不变。
