# Agent Note: 词网格不再因为负的二元分扣分

Status: implemented

## Problem

#7583 报的长句首选里，`yidatuo` 出「益达拖」、`yidatuodade` 出「益达托大的」，而单词列表里「一大」排在「益达」前面（词频 301240 对 200000）。

词网格的每次转移原先加 `bigram_weight * ln(P(next|previous) / P(next))`，表里没有的词对按 0 算。随包的 `msime-bigram.bin` 里 句首→一大 是 -1.006，句首→益达 没有条目。乘上权重 2.0，「一大」在句首被扣约 2 nats，词频上的优势只有 ln(301240/200000) ≈ 0.41 nats，于是「益达」反超。

表只收出现得够多的词对，没收的词对多半比收了但偏少的还少见。负分只落在见过的词对上，常用词因此输给了根本没见过的冷门词。

同一个 issue 的「抽到无法接受」（应为「丑到无法接受」）不是这个原因：词库里没有「丑到」，丑→到 也没有二元条目，两个单字拿不到词组的音节奖励，比不过现成的词「抽到」。那是词库覆盖的问题，这里不处理。

## Decision

`crates/engine/src/lattice/decode.rs` 的束搜索把静态二元分截到不小于 0：`bigram_weight * max(0, bigram)`。表认为偏常见的词对照旧加分；表认为低于随机的词对，和表里没有的词对一样当作没有证据，不扣分。

个人模型的 `personal_transition` 仍拿未截断的静态分当参照概率，它的标定是对着原始分做的。三元重排（`rescore_with_trigram`）有同样的不对称，但三元权重在评测上怎么调都没有影响，这次不动。

`resources/eval/` 下 8 份基线按锁定词库（dict 内容与 `desktop-dictionary.lock.json` 一致，含 `sentence-model.safetensors`）重新生成。改动前它们已经和 develop 对不上（基线最后更新于 2026-09-30），所以下表的对照是同一份词库上 develop 与本改动各跑一遍，不是旧基线对新基线。

## 评测（锁定词库，带重排模型，top-1 / top-5）

| 集合 | 条数 | develop | 本改动 |
|---|---|---|---|
| sentences-v1 | 60 | 0.750 / 0.750 | 0.750 / 0.750 |
| sentences-v2（harvested） | 312 | 0.324 / 0.769 | 0.304 / 0.766 |
| sentences-neutral-v1 | 1105 | 0.608 / 0.833 | 0.617 / 0.847 |
| quanpin-words-v1 --limit 3000 | 2791 | 0.781 / 0.942 | 0.781 / 0.942 |
| 同上 --nine-key | 2791 | 0.575 / 0.851 | 0.574 / 0.851 |
| sentences-nine-key-v1 --nine-key | 81 | 0.506 / 0.519 | 0.519 / 0.519 |
| quanpin-words-v1 --grid fourteen | 2791 | 0.660 / 0.906 | 0.660 / 0.906 |
| sentences-nine-key-v1 --grid fourteen | 81 | 0.568 / 0.568 | 0.580 / 0.580 |

去掉重排模型、只量引擎时，sentences-neutral-v1 从 0.480 / 0.789 到 0.490 / 0.803，sentences-v2 从 0.462 / 0.760 到 0.468 / 0.753。

## Alternatives considered

- **只截断句首的转移** — 问题的例子全在句首，改动面最小。neutral 上它与全部截断几乎一样（带模型 0.615 / 0.845，只量引擎 0.488 / 0.800），略低；句中的转移有同样的不对称，没有理由只修一半。
- **句首完全不用二元分** — neutral 上数字最好（带模型 0.619 / 0.853，只量引擎 0.492 / 0.810），只量引擎时 sentences-v1 top-1 也从 0.617 到 0.633。但它连句首的正向信息（哪些词常开句）一起丢了，sentences-v2 带模型时 top-1 掉得最多（0.324 → 0.292），而且比全部截断只多 0.2 个点，不值得丢掉一类信号。
- **维持现状、把例子交给云候选和重排模型** — 报告人开着云候选，云端第二名就是「一大坨大的」；但报告人关了桌面神经联想，本地首选仍然是错的，而这个不对称在任何输入上都会把常用词往后推。

## Consequences

- **收益**：neutral 两个臂的 top-1 各提高约 1 个点、top-5 约 1.4 个点；九键整句和 14 键整句各多对 1 条。常用词不再因为「见过但少见」输给没见过的冷门词。
- **代价**：sentences-v2 带模型时 top-1 掉 2 个点（约 6 条），nine-key 词级掉 1 条。v2 是专挑当前打错的用例收割的，README 说明它不能当准确率看，但它的退化是真实的，以后改二元表或重新训练时要回头看这几条。
- **重访信号**：二元表改成带回退概率（absent pair 有了真实估计）时，这条截断应该去掉，负分重新有意义。

## Verification

- `crates/engine/src/lattice/decode.rs` 的 `an_unlikely_pair_does_not_lose_to_an_unseen_one`：句首→一大 为 -1.0、句首→益达 缺席时，`yi'da'tuo` 解出「一大拖」。去掉截断时它得到「益达拖」而失败。
- `cargo test -p msime-engine -p msime-input-runtime` 全过；`cargo clippy -p msime-engine --all-targets -- -D warnings` 干净。
- 上表 8 份评测与 `verify-local.sh` 的「sentence conversion eval」同参数，基线已更新为本改动的结果。
