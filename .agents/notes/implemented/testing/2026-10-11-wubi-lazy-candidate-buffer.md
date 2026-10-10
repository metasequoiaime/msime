# Agent Note: 五笔查询候选缓冲延后预留

Status: implemented

## Problem

五笔 provider 的每次前缀查询最多返回 50 条候选，因此在执行 SQLite 行扫描前按固定上限创建候选向量。没有匹配、表缺失、查询失败或所有行因 `NULL` 键值被跳过时，空结果仍申请 50 个 `WordItem` 槽位。五笔输入会在前缀未命中时频繁经过这条路径。

## Decision

候选向量从空容器开始。首条行成功读取并完成键、值和权重转换后，再按原 `QUERY_LIMIT` 一次预留；无效行不会触发预留，已有候选的字段、顺序、排序、SQL 上限和 `SchemeType::Wubi` 标记保持不变。

## Alternatives considered

- 查询前继续预留 50 槽：命中路径少一个分支，但所有空页都支付固定候选存储。
- 按有效行逐步增长：空页可以零容量，但密集命中会失去原有上限提示并增加扩容次数。
- 按 SQLite 实际行数预留：需要额外计数或改变扫描顺序，不能保留当前一次读取和 `NULL` 跳过语义。

## Verification

新增只含 `NULL` 值行的空前缀回归测试；旧实现实际返回容量 50，测试先失败，修复后容量为零。五笔 provider 15 项测试通过，命中和 50 行截断测试继续覆盖候选内容、顺序与上限。提交前运行引擎全量单测、golden、Clippy、Rustfmt、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick`。

## Consequences

空五笔查询省去固定 50 槽位的结果存储；首条有效候选仍按旧上限一次准备容量。Rust 容器容量不等于 SQLite 或系统 RSS，也不能仅凭该变化推断端到端输入延迟下降。
