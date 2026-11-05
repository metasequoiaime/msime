# Agent Note: 双拼末音节两键打完后不再按前缀补全

Status: implemented

## Problem

#10842：小鹤双拼打 `guga`（`gu` + `ga`，两个音节都已打完），候选里出现读音是 `gu'gan` 的「骨干」。

双拼词库查询把双拼切分解成全拼段后，走的是全拼那套逐级查询：先精确查 `gu'ga`，查不到再做末段前缀的范围查询 `[gu'ga, gu'ga{)`。真实词库里没有 `gu'ga` 的二字词，范围查询就补出了 `gu'gan`（骨干、骨感）、`gu'gao`、`gu'gai`、`gu'gang`。这一步对全拼是对的，全拼的 `guga` 可能还没打完；双拼每个音节两键就定了，`ga` 就是嘎，`gan` 在小鹤里是 `gj`。各方案的 `gu`、`ga` 都是字面码，所以所有方案都有这个问题。

## Decision

`PinyinDatabase` 的逐级查询多一个 `TailMatch`：`Prefix` 是原来的行为，`Whole` 跳过末段前缀的范围查询，精确、混合简拼、纯简拼三步照旧。`query_segments_keyed_flat` 仍是 `Prefix`，全拼、九键等调用方不变；双拼通过 `query_segments_keyed_flat_with_tail` 传入。

双拼的 `query_rows` 在两个条件同时成立时用 `Whole`：切分里最后一节非空编码打满两键（手动分隔符留下的空节不算），并且解出的末段是整音节。只打了一键的尾巴（`gug` → `gu'g`）照旧前缀补全，`gu + 声母` 仍能带出「骨干」。

判断放在双拼层，而不是在共享查询里看末段长相，是因为单键 `a`、`e`、`o` 解出的段本身就是整音节：只打一键 `a` 时用户可能正要打 `an`、`ang`，两键 `aa` 解出的 `a` 才是打完的。只有双拼层知道这一段是一键还是两键。

范围查询只会补末段：内部单字母的 `%` 是字面字符，范围必然落空，所以跳过它不影响内部有单字母的切分，它们本来就落到混合简拼那一步。

## Alternatives considered

- **在共享查询里按「来源是双拼且所有段都是整音节」直接返回精确结果**：不用改调用方，但单键 `a`、`e`、`o` 尾巴也会被当成打完，`gua` 不再补出 `gu'an`、`gu'ang`。
- **在共享查询里按「来源是双拼且末段是长度至少为 2 的整音节」跳过范围查询**：不用新增参数，单键尾巴也不受影响，但两键零声母码 `aa`、`ee`、`oo` 解出的单字母整音节仍会被补全，同一个问题留了一个口子。
- **改用 `query_exact_segmentations_keyed_flat`**：已有的精确批量查询，但不按词值去重，排序和整句之外的行数与原来的结果不完全一样，也跳过了内部单字母要走的混合简拼。

粤语双拼在 [粤语双拼展开按两键定音节](2026-10-29-cantonese-shuangpin-strict-expansion.md) 里因为同样的原因关掉了末音节前缀补全，这里是普通话双拼的对应修复。末尾单键的补全和整句见 [双拼未完成尾音节保持较长候选优先](2026-10-22-shuangpin-incomplete-tail-candidate-order.md)，本篇不改那条路径。

## Consequences

- 双拼候选只来自按键确定的音节：`guga` 只出「骨嘎」这类逐字组合和单字，`gugj` 才出「骨干」。整词恰好不在词库里时，用户看到的完整音节候选会少一些，这些本来就是读音不对的词。
- 小鹤 `nihaoma` 这类按全拼习惯打的输入，`niha` 不再补成 `ni'hao` 出「你好」。golden 场景 `sp_segment_boundaries` 的期望里那五条候选正是这个缺陷的产物，期望改为无候选；它要守的切分边界不变。
- 共享查询多了一个公开的 `TailMatch` 参数入口，全拼和九键不受影响。
- 双拼的模糊音走 `query_exact_segmentations_keyed_flat`，整句网格对整音节也只做精确查询，二者都不补末段，这次不用改。

## Verification

- `rbuild cargo test -p msime-engine --lib -- complete_last_code_is_not_prefix_expanded whole_tail_does_not_prefix_expand`：2 项通过。把双拼层改回 `Prefix` 后，引擎级用例失败，`guga` 的候选是 `["骨嘎", "骨干", "骨"]`。
- `rbuild env MSIME_EVAL_RESOURCES=<Studio 上的真实词库> cargo test -p msime-engine --lib -- shuangpin::tests`：89 项通过，包括新加的真实词库断言（`guga` 不出「骨干」，`gugj` 前三有「骨干」）。
- `rbuild cargo test -p msime-engine`：全部通过；`rbuild cargo test -p msime-engine-capi --test golden_replay`：通过；`rbuild cargo clippy -p msime-engine --all-targets -- -D warnings`：通过。
- 设了 `MSIME_EVAL_RESOURCES` 跑 golden 时，四个真实词库集合（quanpin-words、sentences 三组）大面积不符（3000 行里 2996 行）。这几组只走全拼，本次改动不碰全拼路径；没有在未改动的 develop 上用同一份词库复跑对照。
- 没有在真机或已安装的输入法里打字验证。
