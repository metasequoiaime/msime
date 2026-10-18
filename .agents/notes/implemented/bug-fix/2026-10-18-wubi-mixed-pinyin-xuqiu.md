# Agent Note: 五笔混拼 `xuqiu` 在四码碰撞后被截断

Status: implemented

## Problem

正式 `dict-v2.0.24` 中，`xuqiu` 是“需求”的全拼，同时 `xuq`、`xuqi` 都能命中五笔候选。类似地，`hanguo` 的 `hang` 也会命中四码。混拼模式在这些边界把五笔表视为已回答，刷新时清除了延长输入状态，因此后续字母被拒绝。

## Decision

四码后的续输判定统一检查原文是否可以拆成完整的首音节和一个由下一字母继续的合法尾音节；同时保留已有延长状态。这样 `xuqiu`、`hanguo` 等输入会继续走全拼回退，同时不改变没有合法拼音切分的纯五笔四码行为。

## Alternatives considered

只在 `xuq` 没有五笔候选时放开第五码，无法覆盖 `xuqi` 这种继续命中五笔表的冲突；直接把所有五笔输入放宽到 32 码又会改变纯五笔输入的四码行为，因此采用基于已有拼音候选的继承判断。

## Consequences

- 新增 `xuqiu_continues_after_a_four_letter_wubi_code` 和 `mixed_wubi_keeps_a_pinyin_prefix_when_the_fourth_code_is_a_complete_syllable` 回归测试，覆盖 `xuqiu`、`hanguo` 两种四码冲突形态。
- 使用锁定的 `dict-v2.0.24` 资源真实运行查询，`xuqiu` 的第五个 `u` 已被接受，首候选为“需求”。
