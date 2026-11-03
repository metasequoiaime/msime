# Agent Note: 逐键模型进整句解码的束搜索

Status: implemented

## Problem

Mac 上自动打字验收（typing-chat-v1 的 test，220 段）到 81.4% 以后，剩下的错大约一半是「正确整句根本不在候选里」。用户截图里的「下周寺上线」「下周死伤闲的反感」都是这一类：整句解码只按词频、n 元表和成词奖励走束搜索，「下周 | 死伤 | 闲的」凭成词奖励一路领先，「下周四」在前几列就被剪掉。等到交给逐键模型比较的 32 条整句里已经没有「下周四上线的方案」，模型再准也挑不到。

`convert_eval` 上 6x256 配 32 条，typing-chat dev 正确答案出现在候选里的只有 83.6%，test 90.0%。模型自己在 ctx-v1 上的选词准确率是 90%，端到端首选只有 68–79%，差距主要在这里。

## Decision

逐键模型在整句解码的束搜索里给假设打分。

- 引擎新增 `ordering::SentenceScorer`：给定上文和一批文字，返回每条的逐字平均对数概率。`CandidateScorer::sentence_scorer` 默认没有，ONNX 后端（`msime_onnx_rerank::SharedOnnxScorer`）给出一个与逐键打分共用同一个模型和前缀缓存的实现。runtime 挂打分器时经 `Session::set_sentence_scorer` 一并交给引擎，`QueryRequest::sentence_scorer` 带进全拼词典。
- `lattice::decode::BeamModel`：每一列按词网格的分剪到束宽之后，给留下的每个假设的整段文字打模型分，把这一步新增的模型分（逐字之和，乘 `BEAM_MODEL_WEIGHT` = 1）加进假设的分，随假设带到后面的列。最后一列的整句同样加上。末尾那条边是句尾语气词的整句不加这一步：模型不知道句子在这里结束，句尾的「呀」「吧」在它看来像句中，交给 `SENTENCE_FINAL_PARTICLE_BONUS`。
- 模型分只对一段上文成立。全拼词典里按文字存一份备忘（`beam_model_memo`），上文变了清空。每次按键整句都从头解，前面几列的假设和上一键一样，模型分从备忘里取，只有新的一列要跑模型。挂着模型时束搜索不接前缀存档（`BeamPrefixCache` 里没有模型分），整句解码缓存和候选缓存的键都带上文。
- 只在主读法上用。同一串拼音的备选切分（`nian'gao` 之外的 `ni'an'ga'o`、`niang'a'o`）每个都要整句解一遍。一开始每个都让模型打分，一次按键跑五六遍模型，p99 8 ms、最慢一键 55 ms。改成只进这次查询主读法（纠错时是纠错后的读法）的束搜索，`QuanpinDictionary::beam_model_segments` 记着它，其余切分照旧只按词网格解。准确率没有变差（下表），延迟回到预算内。

不挂 ONNX 打分器的平台（只有 1.0 的句子模型）不受影响：`sentence_scorer` 是 `None`，解码与以前完全一样。

## 评测

2026-11-03，同一台 Mac（M 系列），`convert_eval`，锁定词库，资源里去掉随包的句子模型，6x256 配 32 条。聊天集是 typing-chat-v1 按标点切出的句段，上文按消息加换行。首选准确率，括号里是相对 develop 的改对 / 改错：

| 集合 | develop | 每个切分都打分 | 只打主读法 |
|---|---|---|---|
| typing-chat dev（110） | 68.2% | 76.4%（11 / 2） | 76.4%（11 / 2） |
| typing-chat test（220） | 79.1% | 81.8%（9 / 3） | 82.3%（10 / 3） |
| sentences-v1（60） | 83.3% | 88.3%（3 / 0） | 88.3%（3 / 0） |
| sentences-neutral-v1（1105） | 84.2% | 85.6%（30 / 14） | 86.1%（33 / 12） |
| sentences-v2（312） | 76.6% | 79.5%（11 / 2） | 79.2%（11 / 3） |
| sentences-particles-v1（103） | 85.4% | 88.3%（3 / 0） | 87.4%（2 / 0） |

只打主读法：六个集合合计改对 70，改错 20。正确答案出现在候选里的比例，typing-chat dev 83.6% → 92.7%，test 90.0% → 93.6%。

权重在 typing-chat dev、sentences-neutral、sentences-v2 上试了 0.5、1、2：1 在三个集合上都不比别的差，2 让 neutral 和语气词集变差。

延迟用 `rerank_latency` 在同一台 Mac 上量，sentences-v2 加 chat-test，每臂 11351 次按键，与 develop 交替跑两轮。带模型时的整键耗时（ms）：

| 配置 | p95 | p99 | max |
|---|---|---|---|
| develop | 2.32 | 3.23–3.28 | 48–52 |
| 每个切分都打分 | 3.64–3.89 | 7.60–8.01 | 55–58 |
| 只打主读法 | 2.53–2.57 | 3.79–3.91 | 12 |

桌面 p99 ≤ 4 ms 守住了。最慢一键比 develop 低，原因没有细查。

## Alternatives considered

- **只扩大交给模型比较的整句池。** 6x256 已经是 32 条，在桌面预算里再大放不下；池再大，束搜索里早早剪掉的前缀照样不在池里。
- **剪枝之前给整列打分。** 一列在剪枝前可能有几百个假设，一次按键的模型调用会多一个数量级。剪枝后打分，模型分随假设带到后面的列，后面几列的剪枝已经看得到它。
- **接上前缀存档、在存档里存模型分。** 存档按音节和个人模型版本作键，要再带上文和模型分才能复用；备忘已经让重解的前几列不再跑模型，只用引擎时整键 p95 只有 0.45 ms，重解词网格不是瓶颈，没有做。
- **每个切分都打分。** 见 Decision 最后一条，准确率不比只打主读法高，延迟超出预算一倍。

## Consequences

- **收益**：聊天打字首选高 3–8 个点，正确整句进候选的比例高 3.6–9.1 个点。「下周四上线」这类前半句被成词奖励带偏的句子改对。
- **代价**：桌面整键 p95 多约 0.2 ms、p99 多约 0.6 ms。只有主读法得到模型的帮助，正确答案在备选切分里的句子（`xi'an` 对 `xian`）照旧只按词网格解。
- 双拼和九键没有接，整句仍按词网格解；它们要接时，词典照全拼的做法把 `SentenceScorerHandle` 带进解码即可。

## Verification

- `crates/engine/src/lattice/decode.rs` 的 `the_beam_model_scores_prefixes_and_remembers_them`：词网格首选「下周死上线」，挂上只认「下周四上线」的打分器后首选变成它；同一段上文再解一遍不再调用打分器。
- `cargo test -p msime-engine`、`-p msime-input-runtime --features onnx-rerank`、`-p msime-host-api --features onnx-rerank --lib`、`-p msime-onnx-rerank` 全部通过。
