# Agent Note: 九键候选排序表延迟预留

Status: implemented

## Problem

`NineKeySession::refresh` 每次都为候选去重排序使用的 `leading` 建立 `CANDIDATE_LIMIT` 个槽位的 `HashMap`。输入没有可接受的词典行、所有行被筛选掉或查询结果为空时，这张表没有元素，却仍承担固定的哈希桶存储。

## Decision

刷新从空的 `HashMap` 开始；`push_ranked` 在首次保留候选词前按 `CANDIDATE_LIMIT` 调用 `reserve`，后续继续复用原有按词保留最佳 `RankKey` 的逻辑。候选查询顺序、同词去重、排序键、筛选、截断和 `leading` 超过上限时的自然增长保持不变。

## Alternatives considered

- **继续在刷新开始时预留 128 个槽位**：逻辑最直接，也能让密集列表少一次扩容，但空结果的每次刷新都支付固定哈希表存储。
- **完全取消 `leading`，排序后统一去重**：可以省掉这张表，但会让大量重复词条先进入候选向量，扩大候选行的复制和排序成本，也会改变当前跳过已被更优行支配的路径。
- **只按当前候选数逐步预留**：空结果同样可以不分配，但密集刷新会退回几何增长，失去原有的容量提示；首次候选到达时一次按上限预留能保留这条热路径的预算。

## Verification

新增 `push_ranked_defers_leading_capacity_until_a_candidate_exists`，先确认空表容量为零，再确认首条候选进入后容量至少为 `CANDIDATE_LIMIT`；旧实现先行运行时因容量为零而失败。九键测试继续覆盖空查询、筛选、排序、去重和刷新复用路径。

## Consequences

没有候选的九键刷新不再建立 `leading` 的固定哈希桶；有候选时仍一次取得原容量提示，避免密集结果逐项扩容。哈希表容量属于内部实现细节，候选顺序、内容和上限不变。若候选上限或 `push_ranked` 的所有权契约改变，需要重新评估首次预留位置。
