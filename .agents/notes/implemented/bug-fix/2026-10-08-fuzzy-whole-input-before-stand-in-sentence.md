# Agent Note: 全拼模糊音下，覆盖整个输入的模糊词条排在替补整句之前

Status: implemented

## Problem

开启模糊音 z=zh 后全拼输入 `zongguo`，首选是按原拼音 zong'guo 解出的整句「总国」（`Generated`），词库里的「中国」（`Database`，模糊命中，权重 505879）排第二。用户开模糊音就是分不清 z/zh，这一下等于模糊音白开。同类的还有 `cuanqi`（开 c=ch）首选「窜起」而不是「传奇」，`sangban`（开 s=sh）首选「桑半」而不是「上班」，`zengzai` 首选「增在」而不是「正在」。

根因在 `QuanpinDictionary::query`（`crates/engine/src/quanpin/dictionary.rs`）的合并顺序：

1. `query_exact` 只按原拼音查词、解整句。`merge_sentences` 用 `whole_sentence_insert_position` 把整句块放在「原拼音覆盖整个输入的词库行」之后；zong'guo 没有这样的词，整句于是排在第 0 位，它只是没有整词时的替补。
2. 模糊行在 `query_exact` 之后才由 `fuzzy_candidates` 追加，然后整表按 `matched_letters` 稳定排序。整句行和覆盖整个输入的模糊行字母数相同，稳定排序保留原先后，模糊行永远落在整句块之后。

参考实现（QD:1711-1735）就是这个顺序，录下的金样 `fuzzy_session_quanpin`、`fuzzy_learning_canonical_key` 也把「宗国」整句排在「中国」之前。双拼的金样 `fuzzy_session_shuangpin` 反倒是「中国」在前，那是因为双拼按 `pinyin.len()` 排序，模糊行的 `pinyin` 是带分隔符的 `zs'go`（5 字节），整句是 `zsgo`（4 字节），分隔符让模糊行排到了前面，不是有意的规则。

## Decision

模糊行合并、按字母数稳定排序之后，`yield_stand_in_sentence_to_fuzzy_rows` 在覆盖整个输入的那一档（与首行字母数相同的前缀段）里做一次让位：

- **只在整句是替补时让位。** 首行必须是整句行（`sentence_association`）。首行是词库行时什么都不动。
- **精确先于模糊。** 这一档里只要有原拼音覆盖整个输入的精确词库行（`!fuzzy`、`is_dictionary()`、`canonical_pinyin` 字母数等于输入字母数，包括另一种切分的整词，以及排在更长补全词后面的整词），就一行也不动。补全出的更长词条（`canonical_pinyin` 字母数更多）不算覆盖。
- **让位是一次稳定排序。** 这一档按 `!item.fuzzy` 稳定排序：模糊行整体到前面，模糊行之间按 `fuzzy_candidates` 给出的顺序（库里的权重序），整句块和其余行之间的先后不变。

这和 `whole_sentence_insert_position` 的既有约定是同一条规则在模糊音下的延伸：整句是对词库没有整词时的补位，按用户选择的模糊读法覆盖整个输入的词条也是整词，整句不该压在它前面。九宫格 `rank_key` 的「同一覆盖范围内词库行先于整句、精确先于模糊」也是同一个方向，见 [九宫格切分路径排序](2026-10-08-nine-key-path-ranking.md)。

不开模糊音时 `query` 不经过这段代码。双拼的 `append_fuzzy` 不变。

## Alternatives considered

- **把模糊变体送进整句解码（词网格每个跨度也查模糊读法）。** 这是覆盖面最大的修法：长句里夹着 z/zh 混淆时（`womenzaizongguo` 想要「我们在中国」）也能解对，而本修法只管有整词覆盖整个输入的情况。不选它，是因为它改的是解码器本身：每个跨度的查询按模糊规则成倍增加，`lattice_span_cache` 的键要带上规则，模糊读法的整句还要和原拼音整句在同一个打分里比较，哪条读法该扣多少分要重新标定，九键和双拼共用的解码路径都会受影响。这次问题的范围是「已有整词却被替补整句压住」，用排序修就够了；长句模糊解码另作改进时再考虑。
- **整句和模糊行按权重比较。** 直觉上「中国」505879 远大于「总国」4940。但整句的 `weight` 是 `log_prob * 1000`，和词库频次不在一个量纲上，`merge.rs` 文件头和 `WordItem::weight` 的注释都明确说整句行不能按权重排序；九宫格的 `rank_key` 为此也是先按来源再按权重。按权重比只会在别的输入上出现说不清的翻转。
- **不设门槛，覆盖整个输入的模糊行总是排到整句块前面。** 规则更简单。但用产品选项（`sentence_alternatives = true`）实测，`zhongguo` 的列表是「中国 / 种国（整句）/ 中国人 / …」：精确整词后面紧跟着候补整句。不设门槛的话，开了 z=zh 的 `zhongguo` 会把「宗国」这类 z 读法插到「种国」之前、「中国人」之前，精确输入的列表跟着变。要求是精确输入不变，所以只在整句处于首位、原拼音没有整词时让位。
- **模糊行插到 `whole_sentence_insert_position` 算出的位置（精确整词之后、整句之前），不看整句是否在首位。** 与上一条同样会改动 `zhongguo` 这类精确输入的第二位以后，放弃。

## Consequences

- **收益**：开模糊音、原拼音没有整词时，模糊读法的整词成为首选：`zongguo` →「中国」，`zengzai` →「正在」，`sangban` →「上班」，`cuanqi` →「传奇」，`zongguorenmin` →「中国人民」。不开模糊音、或原拼音有整词（`zhongguo`、`zhidao`、`shishi`、`zaijia` 等）时列表逐行不变。
- **代价**：模糊行整体排到整句前面，不看权重，库里很冷的模糊整词也会排在整句之前（`zhuoye` 开 z=zh：作业、昨夜、左腋、坐夜，然后才是整句「琢也」）。这与「整句不压词库整词」的既有约定一致，精确整词同样如此。
- **已知上限**：长句里只有一部分音节需要模糊读法时（`womenzaizongguo`），整句仍只按原拼音解出「我们在总国」。要解决得把模糊读法送进词网格，见上面第一条备选；出现这类反馈时重访。
- **金样**：`fuzzy_session_quanpin` 和 `fuzzy_learning_canonical_key` 第 1 步（输入 `zongguo`）的前两个候选对调为「中国」（模糊，`Database`）、「宗国」（整句，`Generated`），其余步骤不变。真实词库集（`golden/real/*.jsonl`）录制时模糊音关闭，不经过这段代码。

## Verification

- `crates/engine/src/quanpin/dictionary/tests.rs` 的 `whole_key_fuzzy_rows_lead_a_sentence_that_stood_in_for_a_missing_word` 用夹具词库覆盖三种情况：不开模糊音首选整句「总国」且没有「中国」；开 z=zh 后前三位是「中国、种过、总国」；`zhongguo` 首选精确「中国」；加入精确整词「宗国」后它仍在模糊「中国」之前。修复前这个测试在第二种情况失败（「总国」在首位）。`stand_in_sentence_yields_only_to_whole_input_fuzzy_rows` 直接钉住让位函数：补全词不算覆盖、另一种切分的精确整词阻止让位、首行是词库行时不动。
- 在真实词库上用产品选项跑了 46 个 z/c/s 输入、模糊规则 0、1、7 三组，修复前后逐行对比：规则 0 全部相同；规则 1、7 只有原拼音无整词、整句居首的输入变化（上面「收益」列出的几条和 `zhuoye`），其余不变。真实词库金样集在修复前后输出逐字节相同。
