# Agent Note: 混合候选去重状态的栈上容量

Status: implemented

## Problem

`insert_mixed_rows` 的英文、表情和颜文字总量受各自查询上限约束，但去重时仍按完整中文候选列表创建 `HashSet`，随后为三组尾部迭代器创建 `tails`。在 128 个已有候选和 11 个新增候选的合成输入中，合并路径分配 2 次。

## Decision

`insert_mixed_rows`（九宫格的 `insert_expressive_rows` 也走它）用栈上借用表记录位掩码接受的额外候选，检查已有候选时直接扫描，避免按候选列表长度扩容哈希表，去掉 `tails` 容器。借用表的容量 `MIXED_DEDUP_CAPACITY` 是英文 5 行加 emoji、颜文字各 `MIXED_FETCH_LIMIT` 行，最初是 11 项（三组 5、3、3 行），emoji、颜文字改为按关键词紧跟描绘的词之后取回的行变多，现为 101 项，见 [2026-10-08-expressive-rows-follow-their-word.md](../feature/2026-10-08-expressive-rows-follow-their-word.md)。保持各组首次去重的顺序。

## Alternatives considered

继续使用 `HashSet` 只把容量限制为三组上限，仍需把已有候选复制进集合，并保留哈希表分配；直接线性扫描已有候选和最多 11 个栈上额外名称，避免了这次查询的堆状态，代价是固定的小范围比较。

只移除 `tails`，可以少一次分配，但完整候选词集合仍会产生哈希表；只替换哈希表则保留三组迭代器分配。两者都处理才能让受上限约束的合并路径不创建临时堆状态。

## Verification

基线分配预算测试显示 2 次分配；优化后达到最多 1 次分配，现有混合候选顺序、来源、去重和容量测试保持通过。完整引擎测试、Clippy、格式、笔记校验和 quick 门禁覆盖这次切片。

## Consequences

栈上借用表的容量必须与三组生产查询上限一致（`MIXED_DEDUP_CAPACITY` 由 `MIXED_ENGLISH_LIMIT` 和 `MIXED_FETCH_LIMIT` 算出）；每组仍受 64 位掩码上限约束，所以 `MIXED_FETCH_LIMIT` 不能超过 64，超出生产上限的私有调用不属于契约。emoji 有关键词时，定位还要为候选词的有序索引分配一次，分配预算测试按这一次计。
