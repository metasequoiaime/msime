# Agent Note: 双拼末段两键定出的音节不再前缀补全

Status: implemented

## Problem

#10842：小鹤双拼打 `guga`（gu + ga），候选里出现「骨干」。双拼词典查询 `ShuangpinDictionary::query_rows` 把两键码解成全拼切分 `gu'ga` 后，交给与全拼共用的级联 `query_segments_keyed_flat`：精确键 `gu'ga` 查不到时退到前缀区间 `gu'ga%`，于是补出 `gu'gan`（骨干、骨感）、`gu'gao`（孤高）、`gu'gai`（股改）。全拼的末段可能还没打完，前缀补全是对的；双拼的末段两键已经定死了韵母，`ga` 就是 `ga`，`gan` 在小鹤里是 `gj`。

级联判断不了这一点：它只看到解码后的音节，零声母的单键 `a` 和两键 `aa` 都解成 `a`，`gua`（gu + 未打完的 a）和 `guaa` 在级联眼里是同一个 `gu'a`，只有双拼层知道末段按了几个键。

## Decision

双拼层按原始切分的最后一个一键或两键片段判断末段是否已定：两键时调用新的 `PinyinDatabase::query_fixed_tail_segments_keyed_flat`，一键时照旧走 `query_segments_keyed_flat`（`QuerySource::Shuangpin`）。手心 `dk` 补查 `den` 也走前者。

末段已定的级联：

- 先精确查整串键，有结果直接返回，与原级联第一步相同。
- 不做前缀区间查询：它只会补全末段；前面含单键段时区间里有字面 `%`，本来就查不到。
- 只有前面含单键段或 `zh/ch/sh`（手动分隔留下的单键）时才做混合简拼扫描，扫描结果额外要求行键的最后一个音节与末段原样相同，其余段仍按 `matches_mixed_segments` 的双拼规则匹配。
- 不做纯简拼查询：末段是两键解出的 `a/e/o` 时纯简拼会把它当声母补全；多段时混合扫描已经覆盖，单段时只该精确匹配。

末尾单键的补全不变：`gug` 仍补出骨干、孤高，`gua` 仍补出 `gu'ai`；词网格的 `query_lattice_span_with_trailing_initial` 本来就是精确查询，不受影响。模糊音走 `query_exact_segmentations_keyed_flat`，本来就不补全，开着模糊音时 `guga` 同样不出骨干。

## Related decisions

- [粤语双拼展开按两键定音节](2026-10-29-cantonese-shuangpin-strict-expansion.md)：同一原则在粤语双拼上的实现（`complete_trailing` 为假），本篇把它补到普通话双拼。
- [双拼末尾声母参与整句补全](../feature/2026-10-24-shuangpin-trailing-initial-sentence.md)：部分重叠。末尾单键展开仍按那篇执行，本篇只收紧两键末段。
- [双拼未完成尾音节保持较长候选优先](2026-10-22-shuangpin-incomplete-tail-candidate-order.md)：无关，那篇处理单键尾音节的候选顺序。

## Alternatives considered

- **在级联里按「所有段都是完整音节」判断**：改动只在 `query_single_cut_keyed` 一处，不用双拼层传信息。但零声母单键 `a/e/o` 解出来也是完整音节，`gua` 会因此失去 `gu'ai`、`gu'an` 的补全，违背末尾单键按声母补全的约定，所以按键数判断，由双拼层决定。
- **查完再在双拼层过滤末音节**：不碰词典模块，但级联在前缀步取到行就提前返回，过滤后变空时不会再走后面的步骤；末段已定时前缀步本来就不该跑，直接换一条级联更清楚。
- **给 `QuerySource` 加一个变体表示末段已定**：调用点不变，但 `QuerySource` 现在只管单字母段怎么匹配，混进末段语义后 `matches_mixed_segments` 等处都要区分两个双拼变体，所以单独开一个查询入口。

## Consequences

双拼打两键末段只出与输入音节完全一致的词条，`guga` 不再出现骨干，`gugj` 才有骨干。收益是候选与已经敲定的韵母一致；代价是习惯少打末段韵母、靠前缀补全出词的用法不再生效，想要补全需要只打末段的声母键。

golden 合成脚本 `sp_segment_boundaries` 的快照随之改变：小鹤 `nihaoma` 里的 `niha` 是 `ni'ha`，原先补全出的 5 条 `ni'hao` 候选消失，这是本次修复的预期结果，只更新这一份预期。

仍未覆盖的同类情况：前面段是两键解出的零声母 `a/e/o`、后面又有单键段时（如 `aag`），混合简拼扫描仍把那个 `a` 当声母，能匹配 `ai'g…`。需要双拼层逐段传按键数，暂未处理。

## Testing

- `shuangpin::tests::two_key_tail_syllable_is_not_completed`：修复前失败（`guga` 得到 `["骨嘎", "骨干", "鼓高", "骨"]`），覆盖模糊音开关、`gugj`、末尾单键 `gug` 与 `gua` 仍补全。
- `dictionary::pinyin::tests::fixed_tail_cascade_matches_the_tail_syllable_exactly`：全拼 `gu'ga` 仍补全，末段已定时不补全，手动分隔的单键 `sh` 仍按声母展开。
- `shuangpin::tests::real_dictionary_answers_common_readings` 加了 `guga` 与 `gugj` 两项断言，设置 `MSIME_EVAL_RESOURCES` 时在真实词库上运行；未修复时 `guga` 的结果含骨干、骨感、孤高、股改。
