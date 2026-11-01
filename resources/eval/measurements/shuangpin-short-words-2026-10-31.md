# 双拼触点短词纠错评测（2026-10-31）

本次量 #10018 的 1–2 音节完整短词补查；没有改默认参数，没有启用纠错首选。逐组成绩、摘要和输入哈希见[结果 JSON](shuangpin-short-words-2026-10-31.json)，实现边界见[决策笔记](../../../.agents/notes/implemented/feature/2026-10-31-shuangpin-touch-short-words.md)。

## 固定输入

- 基线 `0e2824c8a2` 与分支基础 `f19522212a56bc020212a3f47acea658d162130e` 的 `crates/` 树相同（`ddce062cc7883be93339a9aee06c8e635fde7996`）。比较的两份可执行文件、五个生产文件均记录 SHA-256。本文数字是在生产代码 `f803f9bfae` 上量的（`short_words.rs` SHA-256 `f2d25cfe…`），之后还有三个改生产代码的提交：`10ef8a45c0`、`5f19810bba` 改 `personal_rerank.rs`，只在会话有前一个词、个人模型给出非零置信度时生效，词级评测和延迟评测都没有前一个词，不影响这些数字；`87b3249588` 在 `short_words.rs` 加的排名比较永远不成立（纠错行按排名降序逐个插入），已由 `20e633f8fb` 去掉，此后 `short_words.rs` 与测量版本只差一行注释（SHA-256 `57febdb4…`），候选与顺序不变。2026-11-01 在 `20e633f8fb` 上用同一资源目录（stderr 打出 `no settled model at ...`）按[评测说明](../README.md)的词级命令重跑一次并 `--update-baseline`，提交的 `baseline-shuangpin-touch-words.json` 即由本次 current 臂重新生成，报告与本次测量的 `words-current.json` 逐字节相同（SHA-256 `cc4bf4e6…`）。
- `resources/desktop-dictionary.lock.json` 对应 `dict-v2.0.41`、词库提交 `09a69f20b4726f75d4aa318975e3e891c1135aaf`。资源逐文件大小与 SHA-256 都已核验；有小句子模型，没有桌面 settled 模型。
- `quanpin-words-v1.tsv` 前 3000 行中至多两个音节的 2772 条，五方案各可编码 2748 条，全部是两音节。单音节行为由合成回归覆盖，不能从本词集推导单字召回。
- 五方案 `xiaohe,ziranma,shoudao,microsoft,smart_abc`，两份 Android 导出几何；`none,clean,noisy,realistic`，σ 为 `0,0.15,0.25,0.35`，纵向偏置为 `0,0.1`，种子 `20261027`，共 125 组。
- 主机 `Mac13,1`、10 个物理核、32 GiB，macOS 27.2，Rust 1.97.1。模拟触点不是手机真机盲打数据。

## 词级结果

σ=0.25、纵向偏置 0，数字是候选前九位包含金标准的比例（不是首选准确率）：

| 方案 | noisy 基线 → 本次 | noisy found 基线 → 本次 | realistic 基线 → 本次 |
| --- | --- | --- | --- |
| 小鹤 | 0.049 → 0.544 | 0.073 → 0.566 | 0.889 → 0.943 |
| 自然码 | 0.049 → 0.548 | 0.078 → 0.569 | 0.885 → 0.944 |
| 首道 | 0.044 → 0.551 | 0.068 → 0.571 | 0.898 → 0.946 |
| 微软 | 0.049 → 0.557 | 0.079 → 0.587 | 0.871 → 0.929 |
| 智能 ABC | 0.047 → 0.523 | 0.073 → 0.556 | 0.885 → 0.930 |

40 组 clean 共 109920 例，实际首选变化 0，金标准降位 3 例；最差单组 1/2748（0.0364%，低于 0.5%）。两份完整 dump 的 288540 条去重记录逐行比较，所有首选候选对象完全相同，包括 noisy；其中 13740 条无触点记录逐字节相同，61369 条记录在首选之后有变化。完整 dump 体积较大，不提交仓库；JSON 保留双方 SHA-256 和比较计数。

首选不变来自明确的座位约束：原候选表为空时不补首选；同文前缀恰是第一条时也不把它替换成完整纠错词。例如真实词库的 `gq` 若已有「给」作为首位前缀，此处仍保留其原消耗语义，不能宣称所有 `gq → 给` 都已覆盖。其他位置的同文前缀会被完整词替换，Session 回归核对完整消耗与键对学习。

## 无触点对照

全拼、九键、十四键、各自前缀与句子、助词，以及五方案双拼四份句集的 G0 逐字节比较结果记录在 JSON。比较对象同时包含报告与完整候选明细，不用相同的汇总准确率替代行为对照。

## 延迟与边界

按[既有触点量法](../../../.agents/notes/implemented/feature/2026-10-27-shuangpin-touch-correction.md)跑两臂：相同误触键、都加载模型，一臂带落点、一臂不带。每方案五轮，每轮随后运行双臂都不带落点的 A/A。下表的增量是两臂 p95 之差，不是逐键配对差的 p95。

| 短词专集 | 带触点 p95 中位数（最大） | p95 之差中位数（范围） | A/A 差中位数 |
| --- | --- | --- | --- |
| 小鹤 | 1.75 ms（1.84） | +0.62 ms（+0.36–+0.72） | +0.01 ms |
| 微软 | 1.27 ms（1.73） | +0.36 ms（+0.11–+0.55） | 0.00 ms |

短词专集 20 次运行均退出 0，均在 16 ms 预算内。小鹤 +0.62 ms 超过历史 0.5 ms 增量门槛的数值，且高于 A/A 噪声；不宣称短词查询没有开销。历史门槛指定的集合是 `sentences-v1.tsv` 加 `sentences-particles-v1.tsv`，这份集合另行保留五轮结果。

首次历史句集测量与跨平台编译并发：第一轮小鹤带触点 p95 为 12.70 ms；第二轮平面臂 16.85 ms、触点臂 16.27 ms，预算退出 1。本次保留这次失败数据，不改预算，不把负载中的失败记为通过。跨平台编译结束后，独占测量队列完成历史句集的 20 次运行，全部退出 0；小鹤带触点 p95 中位数 5.17 ms（最大 5.22），两臂 p95 之差中位数 +0.07 ms（+0.04–+0.09），A/A -0.01 ms；微软分别为 5.10 ms（最大 5.15）、+0.05 ms（+0.04–+0.08）、A/A +0.05 ms。该历史句集的 16 ms 预算与 0.5 ms 增量门槛均通过，不能外推为短词专集没有可测开销。逐轮命令和原始数值保存在 JSON 的 `latency_sentences_quiet_rounds`。

## 复现

在基线和本 PR 源码各自构建 `cargo build --locked --release -p msime-input-runtime --example convert_eval --example rerank_latency`，保存两份 `convert_eval`。设置 `resources` 为经当前锁核验的资源目录；对每份可执行文件运行（输出名中的 `ARM` 分别替换为 `base`、`current`）：

```sh
mkdir -p target/short-word-evaluation
./target/release/examples/convert_eval \
  --resources "$resources" \
  --set resources/eval/quanpin-words-v1.tsv --max-syllables 2 --limit 3000 \
  --scheme shuangpin --shuangpin-profile xiaohe,ziranma,shoudao,microsoft,smart_abc \
  --autocorrect --touch-mode none,clean,noisy,realistic \
  --touch-sigma 0,0.15,0.25,0.35 --touch-bias-y 0,0.1 \
  --seed 20261027 --jobs 5 --assert-top1-unchanged \
  --report target/short-word-evaluation/words-ARM.json \
  --dump target/short-word-evaluation/words-ARM.jsonl
```

G0 的全拼族命令和集合在[评测说明](../README.md)；两份同源输入的报告与 dump 同时做字节比较。双拼 G0 用四份句集 `sentences-v1.tsv`、`sentences-neutral-v1.tsv`、`sentences-v2.tsv`、`sentences-particles-v1.tsv`，五方案、`--autocorrect --touch-mode none`。

历史延迟对每个 `profile`（小鹤、微软）交替跑五轮以下命令，每轮紧接同命令加 `--no-touch-points`；微软用 `android-portrait-microsoft.json`。短词专集把两个 `--set` 换成前 3000 行中至多两音节的过滤 TSV，其余参数相同。

```sh
./target/release/examples/rerank_latency \
  --resources "$resources" \
  --set resources/eval/sentences-v1.tsv --set resources/eval/sentences-particles-v1.tsv \
  --scheme shuangpin --shuangpin-profile "$profile" --autocorrect \
  --key-geometry resources/eval/key-geometry/android-portrait.json \
  --touch-sigma 0.25 --touch-bias-y 0 --touch-mode realistic \
  --seed 20261027 --touch-arms
```
