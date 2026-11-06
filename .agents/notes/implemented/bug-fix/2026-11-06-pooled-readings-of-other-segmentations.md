# Agent Note: 整句池挪进来的另一种切分读法，降位时一起认

Status: implemented

## Problem

macOS 0.58.0 上开着整句联想「增强」（ONNX 逐键重排）打 `wenjian`，首页是「文件、问间、问及按、问及安、问及案、问及暗」，「稳健」「闻见」「文建」「文件夹」「文件的」这些词库词被挤到第二页以后（#10998）。五笔混输拼音打同一串也一样。只关掉「增强」，或者用 `msime-mcp` 的 `lookup_candidates` 查（它不挂打分器），顺序就是「文件、问见、文件夹、文件的、稳健、文件中、闻见、文建」，wen'ji'an 的读法排在第 9 位以后。

用 0.58.0 和 develop 的 `msime-host-api` 各编一个经 C ABI 打字的小程序，在维护者本机词库（dict-v2.1.6）和偏好的副本上复现：差别只在于有没有挂报整句池的逐键打分器。落定重排（「桌面神经联想」）不参与，逐键打分器比它强时它不跑。词库版本也不是原因，换回 dict-v2.1.0 结果相同。

两条规则叠在一起：

1. 打分器报了整句池（这台 Mac 自测后报 32 条）时，全拼引擎为了让另一种切分（wen'ji'an）的读法留在模型的比较范围里，用 `quanpin::series::defer_pooled_sentences` 把它们放进主切分（wen'jian）整句块的座位，也就是紧跟在整串词库词「文件」后面，见[整句池笔记](../feature/2026-11-02-sentence-pool-for-reranker.md)。
2. 模型比完以后，`ordering::runner_up_order_with` 把没选中的读法挪到表尾，但它只认和第一条读法字数相同的行。第一条是两字的「问间」，三字的 wen'ji'an 读法一条也认不出来，二十几条全部留在原位。

没有整句池时引擎把另一种切分的读法排在主切分的整串词之后，这正是查询工具看到的顺序，所以只有挂着重排的用户碰得到。

## Decision

`ordering::runner_up_order_pooled(scheme, rows, touch_supported, pool)` 多收一个参数：这张表解码时引擎拿到的整句池。runtime 的 `demote_runner_up_readings` 传 `Runtime::sentence_pool()`，键位网格上传 0，与 `rerank` 给模型比较范围的取法一致。`runner_up_order_with` 和 `runner_up_order` 是池为 0 的特例。

池超过引擎默认的条数（`lattice::decode::SENTENCE_PATHS`，6），且方案是全拼或五笔（拼音回退走同一个全拼引擎）时，降位多认一种行：字数和第一条读法不同、同样读满整串（`answers_key`）的词网格行，即另一种切分的读法。对它们：

- 排在最前的那一条留下，坐到它后面最后一个读满整串、不是纠错的词库行之后；后面没有这样的词库行时留在原位。这就是没有整句池时引擎给它的位置。
- 其余的和同一种切分多出来的读法一起挪到表尾，保持原来的先后。
- 第一条读法后面留几条（两字留 1 条，三字以上留 3 条）只从和它同字数的读法里数，与以前相同。

模型的比较不受影响：`order_presentation` 先 `rerank` 后降位，另一种切分的读法照样在比较范围里，模型挑中的照样提到首位。首选在任何情况下都不变：降位不动第一行。

wenjian 在本机副本上改完后的首页是「文件、问间、文件夹、文件的、稳健、文件中」，「问及按」排在「文建」之后。

## 评测

`convert_eval`，dict-v2.1.2（`desktop-dictionary.lock.json`，生成自 `24852caa38`），develop `3e28ac010d` 对照本改动，同一台 Studio。

不挂 ONNX 模型（随包 `sentence-model.safetensors`，不报整句池）：`verify-local.sh` 的十三份集合（sentences、harvested、neutral、words、九键与 14 键的词级、前缀、长句、particles 三种键盘）逐份报告相同。

挂 ONNX 模型（`--onnx-model`，整句池按 `--sentence-pool`）：

| 模型、池 | 集合 | top1 | top5 | top9 |
|---|---|---|---|---|
| 6x256，32 | words（3000 条，实取 2791） | 0.728 → 0.728 | 0.907 → 0.949 | 0.930 → 0.986 |
| | particles（103） | 0.874 → 0.874 | 0.981 → 0.961 | 0.981 → 0.981 |
| 6x256，9 | words | 0.728 → 0.728 | 0.918 → 0.949 | 0.986 → 0.986 |
| | particles | 0.874 → 0.874 | 0.971 → 0.961 | 0.981 → 0.981 |
| 8x448，9 | words | 0.727 → 0.727 | 0.918 → 0.949 | 0.986 → 0.986 |
| | particles | 0.893 → 0.893 | 0.981 → 0.971 | 0.990 → 0.990 |

sentences、harvested、neutral 三份在三个臂上都逐份相同。words 的收益来自四字词：不断更新、不良后果这类词以前被二十几条另一种切分的读法挤到第 22 到 30 位，现在回到第 4 位。particles 少了 chulia 的「处理啊」（2 → 6）、shejia 的「设计啊」（3 → 5）、xinxia 的「信息啊」（3 → 8），三条都还在第 9 位以内，也正是不挂模型时它们的位置（chulia 不挂模型时在第 6 位）。

## Alternatives considered

- **字数不同的读法一律当读法，不看整句池。** 最强的理由是规则最简单，wenjian 的首页最干净（「问及按」排到单字后面）。不用，是因为没有整句池的表里另一种切分的读法本来就排在整串词之后，那里的第一条常常就是要的句子：不挂模型时 particles 的 chulia 从第 6 位掉到第 215 位，九键 particles 的 top5 从 0.786 掉到 0.573，14 键从 0.883 掉到 0.757。
- **只按整句池放开，另一种切分的读法全部挪到表尾。** 6x256 池 32 时 words top5 到 0.949，与本篇相同，但 particles 的 top5 和 top9 都从 0.981 掉到 0.951：「处理啊」「设计啊」「信息啊」掉到第 130 到 215 位，模型没把它们提到首位时就找不回来了。
- **另一种切分排在最前的那一条留在整句池给它的座位上（通常是第 3 位），其余挪到表尾。** 最强的理由是 particles 一条不丢（池 32、池 9 都与 develop 相同），words top5 也到 0.944 和 0.948。不用，是因为 wenjian 的首页仍然是「文件、问间、问及按」，一条生造组合排在「稳健」「文件夹」前面，正是 #10998 报的现象；而把它放回整串词之后，particles 那三条仍在第 9 位以内，与不挂模型时一致。
- **在引擎里改 `defer_pooled_sentences`，不把另一种切分的读法放进主切分的座位。** 那样它们在池为 32 时会落到比较范围之外，整句池笔记里 fangan 的「两个方案都挺好的」又会变回「两个反感」。比较范围要它们靠前，展示要它们靠后，两件事分别由引擎的座位和 runtime 的降位负责。

## Consequences

- **收益**：挂着逐键重排的全拼和五笔混输里，另一种切分的读法不再成批排在整串词库词前面；首选不变，不挂模型的各平台、九键和 14 键的顺序逐字节不变。
- **代价与已知上限**：particles 里「词 + 啊」这类要靠另一种切分的句子，模型没把它提到首位时从第 2、3 位退到第 5 到 8 位，与不挂模型时相同。锚点是「最后一个读满整串的词库行」，表里没有整串词库行时那一条留在原位。将来引擎换了整句池的座位安排，或者别的方案（双拼）也接上整句池，要同步这里的方案判断。

## Verification

- `crates/engine/src/ordering.rs` 的 `readings_of_another_segmentation_go_behind_the_words`、`kept_readings_come_from_the_leading_segmentation`、`other_widths_need_a_whole_key_reading`。
- `crates/input-runtime/src/tests/sentence_pool.rs` 的 `readings_of_another_segmentation_stay_behind_dictionary_words`（全拼 wenjian，池 9 和 32）、`wubi_mixed_pinyin_keeps_generated_readings_behind_words`、`without_a_pool_readings_of_another_segmentation_keep_their_seats`、`a_whole_key_dictionary_word_keeps_the_lead_over_pooled_readings`（wuliji）。
- 评测按上表重跑：`convert_eval --resources <锁定词库> --set resources/eval/quanpin-words-v1.tsv --limit 3000 --onnx-model <rerank-model-small> --sentence-pool 32`，particles 换 `sentences-particles-v1.tsv`。
