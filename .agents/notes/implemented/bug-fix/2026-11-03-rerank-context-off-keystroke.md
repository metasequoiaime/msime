# Agent Note: 上屏后的新上文在后台编码，不留给下一次按键

Status: implemented

## Problem

2.0 默认开以后在 Mac 上做真机验收（自建窗口里按 143 ms 一键自动打 typing-chat-v1 的 30 条消息，「增强」开关交替），2.0 让每键多花约 15 ms：输入法处理一个键（扣掉候选窗绘制）p95 从 9.4 ms 到 24.0 ms、p99 从 14.0 ms 到 39.8 ms，远超桌面整键 p99 ≤ 4 ms 的线。此前 `rerank_latency` 和 `keystroke_latency` 量出来只有 p95 2–3 ms。

原因是那两个工具从不上屏：`keystroke_latency` 打完一个词就取消，上文永远是空的。真实会话每段都上屏，上屏后上文变了，`OnnxScorer` 的缓存只对旧上文成立，于是新一段里第一次用到模型的那个键要先把整段上文（大模型 `context_chars` 是 64 个字）从头编码一遍，再算候选前缀。用宿主自己的 `runtime-options.json` 回放、每段按高亮候选上屏（`crates/host-api/examples/rerank_replay.rs`），这一步约 9 ms，机器忙时到 18 ms；每段一次，正好落在 p99 上。真实的 macOS 输入法里同一主线程还在画候选窗、做候选翻译，同一次编码更慢。

## Decision

上文一变就在别的线程把它编码好，按键线程只算候选前缀：

- `CandidateScorer::context_changed(context)`：带空的默认实现。`context` 是此后按键排序所依据的全部上文，没有按 `rerank_context` 截短。约定是不在调用线程上做耗时的事——它就在上屏那一次按键里被调。
- `Runtime::remember_commit` 在更新 `ai_context` 之后、上文不为空时调它。每次上屏（`transition`）和宿主送来上文（`seed_context`）都经过这里；失焦清空上文不经过这里，不通知。
- `OnnxScorer::prime_context(context)`：与按键时同样取末尾 `context_chars` 个字，缓存已经是这段上文就什么都不做，否则清空缓存重新编码。按键时的那段逻辑与它共用 `encode_context`。
- host-api 的 `SharedOnnxScorer` 实现 `context_changed`：把上文发给这个会话自己的后台线程（`msime-rerank-context`），线程在锁里调 `prime_context`，排队里有好几段时只编码最新那段。线程在第一次上屏时才起，会话的这一份放开时（发送端随之放开）结束。

用户在编码完之前就按下一个键时，按键在同一把锁上等它做完，花的时间与以前按键自己编码相同，不会更慢。候选不超过 16 个字（`CANDIDATE_ROOM`）时，按键交来的上文与预先编码的末尾 64 个字一致，直接复用；更长的候选让 `rerank_context` 把上文截到 48 个字，那一次照旧重新编码。

只有 host-api 挂的 `SharedOnnxScorer` 在后台编码。评测示例直接挂的 `OnnxScorer` 和随包的 `Reranker` 用默认的空实现，行为不变：同步编码只会把这几毫秒从下一个键挪到上屏那个键上，没有意义。

## Alternatives considered

- **上屏时同步编码** — 不用线程，也不用管锁。但上屏本身就是一次按键（空格），这几毫秒只是从下一个键挪到了空格上，逐键 p99 不变。
- **上屏后沿用缓存、只把上屏的字接在上文链后面** — 上屏的那个候选的前缀节点本来就在缓存里，理论上可以接着用，完全不用重算。但大模型的上文窗口是 64 个字，聊几句以后上文就一直是满的，每次上屏窗口都整体滑动，位置全变，缓存里的东西都不能用；要接着用得先改成分块滑动并能从缓存张量里挑行，改动大，收益只在上文没满时才有。
- **把 macOS 的 ONNX Runtime 线程数从 2 降到 1** — 曾怀疑输入法进程里的第二个线程调度不及时。采样（`sample`）显示主线程在 ONNX Runtime 里的时间几乎都在算矩阵乘，等线程池的只占约 7%，不是瓶颈。

## Consequences

- **收益**：回放里每段第一个用到模型的键不再等上文编码。同一负载下交替跑三轮，2.0 开时整键 p99 从 23.7–26.6 ms 降到 7.6–9.2 ms，每段前 6 个键的 p95 从 14.5–15.9 ms 降到 4.1–5.3 ms；首选正确率不变（80.5%）。装进真实的 macOS 输入法，用同一个打字驱动「增强」关开交替各两轮（每轮 41 段、约 910 键）：处理一个键扣掉候选窗绘制，2.0 开的 p95 / p99 是 26.5 / 37.0 ms，关是 27.7 / 37.9 ms，2.0 不再拉长尾部（修之前开比关多 14.6 / 25.8 ms）；首选正确率开 78.0%、关 63.4%。这一次两边的绝对值都比修之前那次（关 9.4 / 14.0 ms）高，测量时机器负载到了 9–30，同一负载下两边相同。
- **代价**：每个挂着 ONNX 重排的会话多一个常驻线程（不打字时阻塞在通道上，不占 CPU）。上屏后约 10 ms 里共用的模型被后台线程占着；同一时刻另一个会话也在用这个模型时要等它。
- **已知上限**：每键本身的模型时间（只算候选前缀）没有变，回放里 p95 约 3.5–5.5 ms，随机器负载变化；真实 macOS 输入法里主线程还要画候选窗，单这一项每键约 24 ms（萤火虫皮肤），那是另一回事。

## Verification

- `crates/onnx-rerank/tests/golden.rs`：`priming_the_context_scores_like_encoding_it_on_the_keystroke` 预先编码后逐键打分与参考运行时一致；`a_keystroke_after_priming_another_context_starts_over` 预先编码的上文与按键交来的不同时照旧从头来。
- `crates/input-runtime/src/tests/candidate_scorer.rs`：`the_scorer_hears_each_new_context_after_a_commit`，宿主送上文和上屏各通知一次，交来的是全部上文，失焦不通知。
- `crates/host-api/src/tests.rs`：`the_session_scorer_primes_the_context_off_the_keystroke`，连着几次通知后紧接着打分，分数与不经后台线程时相同，会话放开后线程结束。
- 回放：`cargo run --release -p msime-host-api --features onnx-rerank --example rerank_replay -- <runtime-options.json> <cases.json> <bundle Contents> on`。

相关：[排序决策经 CandidateScorer 用模型](../architecture/2026-11-01-candidate-scorer-boundary.md)。
