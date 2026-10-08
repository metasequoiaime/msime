# Agent Note: 错字跨度键规划复用已有键

Status: implemented

## Problem

错字边规划一次最多保留 96 个跨度键，但每次仍额外创建 `HashSet<String>`，并为哈希集合复制一份键；`PlannedKey` 已经持有同一份键字符串。

## Decision

直接线性扫描已规划的 `PlannedKey` 键，发现重复时跳过；规划数量由 `TYPO_KEY_BUDGET` 固定为 96，因此不会出现无界平方级输入。

## Alternatives considered

为 96 个键建立栈上字符串槽会增加固定栈占用；普通 `Vec<String>` 仍需临时堆分配。复用已有规划行既省哈希桶也省键克隆。

## Verification

新增已有键扫描的零临时分配测试，并保留首次出现语义测试；错字边测试、完整引擎测试、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

键检查最多扫描 96 条已有规划行，复杂度有界；输出键、跨度、惩罚和首次出现顺序保持不变。
