# Agent Note: 拼音打完整时，首选不再让给更长的补全词

Status: implemented

## Problem

#7345：全拼输入 `dacuozi`，首选是「打错字了」；输入 `doumeiren`，首选是「都没人管理」。按空格上屏会多出用户没打的字。

根因在 `QuanpinDictionary::query_series`（`crates/engine/src/quanpin/dictionary.rs`）。对覆盖全部音节的那一组，它先取同长度查询的行，再追加 `longer_phrase_candidates`（最多多 3 个音节的补全词），然后整组按 `weight` 排序。补全词的语料权重只要比整词高，就排到首位：锁定词库里 `da'cuo'zi` 的「打错字」是 565，`da'cuo'zi'le` 的「打错字了」是 1010；`dou'mei'ren` 的「都没人」只有 3，「都没人买」129、「都没人管」100、「都没人管理」100 全在它前面。

整句块也救不回首位。`whole_sentence_insert_position`（`crates/engine/src/lattice/merge.rs`）只数开头那几行 `canonical_pinyin` 等于输入的词库行，补全词的 `canonical_pinyin` 是更长的读音，于是插入位置是 0；而和整词同字的整句又因为整词已在列表里被当成重复丢掉。

`merge.rs` 文件头和测试 `sentence_rows_follow_the_exact_hits_and_precede_longer_phrases`（quanpin.md §11.1）写的顺序本来就是「整词、整句块、补全词」。原有测试只用整词远重于补全词的夹具（苹果 1143881 对 苹果电脑 21495，你好 10000 对 你好吗 900），反过来的情况从来没测过。这个排序从 e2945caa9c 引擎重写起就存在。

## Decision

全长组里有补全词时，首位只留给读音和输入完全一致的同长度行里最重的那一行，补全词只参与第二位起的排序：

1. 同长度查询的行先按 `weight` 稳定排序。排第一的那行 `canonical_pinyin` 等于输入的切分时，把它固定在首位。
2. 其余同长度行和补全词合在一起，再按 `weight` 稳定排序。同权重时同长度行在前，和原先整组稳定排序的结果一致。
3. 同长度查询一行都没有，或排第一的那行读音和输入不一致时，不固定首位，同长度行和补全词照旧整组按权重排，整句块由 `whole_sentence_insert_position` 插到首位。

补全词仍然排在冷僻同音词前面（苹果电脑 在 平锅 前面），这是它们并入全长组的原因，不变。补全词的 `pinyin` 仍是键入的切分、`canonical_pinyin` 仍是更长的读音，选中后只消耗已键入的音节，这部分也不变。

首位要求 `canonical_pinyin` 和输入逐字相等，判据与 `whole_sentence_insert_position` 认定整词的条件一致。同长度查询有精确键时只返回精确键的行，没有时才退到前缀区间（`build_key_like_pattern` 的末音节带 `%`），所以只看排第一的那行就够了。末音节同时是更长音节前缀时（`jian` 和 `jiang`），没有 `zhi'liang'jian` 这一行的输入会带回「质量奖」这类前缀行。它的字数和音节数虽然相同，读音却不是用户打的，#7345 保护首位的理由（首选必须是用户打出来的词）对它不成立，固定它反而会压过更重的补全词「质量检验」。这类前缀行和补全词一起按权重排，和修复前一样。

双拼走同一个 `query_series`，一起修好。改动只在引擎，没有新的偏好项，各平台宿主不受影响。

## Alternatives considered

- **加一个开关，关掉补全词。** 报告人提的就是这个。开关能让不想要补全词的人彻底不见它们。不选它：问题出在首位，不在补全词本身；排序修好后首选已经是用户打出的词，补全词排在后面不会被空格误选。加开关要新增偏好键，还要在六个平台的设置界面各接一遍。以后真有人不想在列表里看到补全词，再单独考虑。
- **去掉补全词。** 实现最简单。但同长度表里常常只剩冷僻同音词，`ping'guo` 的第二位会退回「平锅」这种几乎没人要的词，这正是当初并入补全词的原因。
- **所有同长度行都排在全部补全词前面。** 也能保证首位，规则更好讲。但「评过」「平果」「平锅」会压过「苹果电脑」「苹果公司」，第二位重新落回冷僻同音词，等于把并入补全词的理由撤回了。只保护首位就够解决多上屏的问题。
- **按比例保护：权重不低于首个整词某个比例的同长度行都排在补全词前面。** 比只保护首位覆盖面更大。但比例没有依据可定，词频跨度从个位数到百万，任何阈值都会在某些输入上出现说不清的翻转；首位以外的位置用户要看过才会选，误上屏的风险只在首位。

## Consequences

- **收益**：`dacuozi` 首选「打错字」，`doumeiren` 首选「都没人」，补全词排在整句块之后。用锁定词库和产品重排器跑 `convert_eval` 的词级集（`quanpin-words-v1.tsv --limit 3000`，2791 条），top-1 从 0.756 升到 0.781，MRR 从 0.829 升到 0.847，top-5 从 0.941 升到 0.942。不检查读音的初版逐条比对是 77 条名次变好、1 条变差；加上读音条件后，排第一的同长度行读音不一致的输入恢复修复前的整组按权重排序，其余输入和初版相同，那 1 条变差的用例回到原位。
- **代价**：补全词的语料权重再高，也只能排第二位起，整词很冷而补全词很常用时（「都没人」3 对「都没人买」129），用户要往后看一位。最初的实现不检查读音，把前缀行「质量奖」也固定在首位，`zhiliangjian`（金样「质量检验」）因此从第 5 位退到第 6 位；加上读音一致的条件后它回到第 5 位，「质量奖」排在补全词之后。
- **基线**：`resources/eval/baseline-words.json` 按这次结果更新。整句集 `sentences`、`harvested`、`neutral` 在改动前的 develop 上就已经和基线不一致，这次改动只让 `harvested` 的 top-1 从 0.321 变成 0.324，没有顺手更新它们；九键和 14 键四个集改动前后都和基线一致。
- 和 [模糊音下覆盖整个输入的词条排在替补整句之前](2026-10-08-fuzzy-whole-input-before-stand-in-sentence.md) 是同一个方向：整句是没有整词时的补位，补全词同样不该压在用户打出的整词前面。
- [拼音打完整时，补全词让位于一页精确同音词](2026-10-13-longer-phrases-yield-to-a-full-page.md) 接续这一决定：精确同音词够填一页时补全词整组不再并入，不足一页时最多并入 3 条；本篇的首位规则不变。

## Verification

- `crates/engine/src/quanpin/dictionary/tests.rs` 新增五个测试，全部用合成夹具：`heavier_longer_phrase_does_not_take_the_first_seat_from_the_exact_reading`（打错字 565 对 打错字了 1010，且补全词仍在冷僻同音词「大错字」之前）；`light_exact_reading_still_leads_heavier_continuations`（都没人 3 对 100 到 129 的补全词）；`sentence_rows_follow_a_light_exact_hit_and_precede_heavier_longer_phrases`（开整句候补时整句块夹在整词和补全词之间）；`sentence_leads_longer_phrases_when_no_exact_reading_exists`（没有整词时整句在补全词之前）；`prefix_only_row_does_not_pin_the_first_seat_over_a_heavier_continuation`（只有前缀行 质量奖 10 时，更重的补全词 质量检验 500 排在它前面）。前三个在修复前失败，第四个修复前后都通过，钉住没有整词时的既有行为；第五个在不检查读音的初版上失败。原有的 `longer_phrases_join_the_full_key_group_by_weight` 和 `sentence_rows_follow_the_exact_hits_and_precede_longer_phrases` 不改动照样通过。
- 用锁定词库和产品重排器经完整 runtime 打 `dacuozi`、`doumeiren`、`pingguo`：修复前前两个的首选分别是「打错字了」「都没人管理」，修复后是「打错字」「都没人」；`pingguo` 前后都是「苹果」，第三、四位仍是「苹果电脑」「苹果公司」；`zhiliangjian` 的「质量检验」在第 5 位，「质量奖」在第 8 位。
