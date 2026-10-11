# Agent Note: 多读音粗筛借用词典键

Status: implemented

## Problem

`LanguageDictionary::lookup_readings` 的多位置多读音路径用 SQLite `GLOB` 做粗筛后，会先把每行 `key` 转成拥有型 `String`，再调用 `key_matches` 做精确校验。读音长度不一致时粗筛范围会放宽，常见输入会产生被淘汰的行，这些键分配随后全部丢弃。

## Decision

改用 `statement.query` 逐行读取，以 `row.get_ref` 借用 `key`、文字并先完成类型校验；只有 `key_matches` 接受该行时，才把键和文字复制到返回结果。SQL 查询范围、精确匹配、排序、截断与错误传播保持不变。

## Verification

新增合成 SQLite 回归测试，构造一条命中行和两条只通过 `GLOB` 粗筛但会被精确校验淘汰的行。旧实现分配计数为 15，测试先失败；修复后计数降为 11，且完整候选结果保持一致。

## Alternatives considered

- **继续用 `query_map` 返回拥有型行**：会在精确校验前复制粗筛键，无法消除被淘汰行的分配。
- **只借用键而仍返回 `query_map`**：迭代器不能把行内借用值带出闭包生命周期，必须在查询行循环内完成校验和复制。
- **改变公开结果类型以共享键**：会扩大接口和调用方改动范围，且不能消除粗筛行的其他转换成本。

## Consequences

粗筛淘汰行不再分配键或文字字符串；命中行仍返回独立拥有的键和文字，公开类型与排序保持不变。每行仍会完成键、文字和权重的 SQLite 类型校验，因此错误传播顺序保持一致。
