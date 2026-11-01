# Agent Note: 排序决策经 CandidateScorer 用模型，原生模型后端在引擎之外

Status: implemented

## Problem

按键重排和停顿重排原来把 chinese-ime-lm 的 `Reranker` 写死在类型里：`rerank_pick`、`pick_contest` 取 `&mut Reranker`，`Runtime` 的 `reranker` 和 `settled_reranker` 是 `Option<Reranker>`，停顿评分请求也带着一个 `Reranker`。这个模型是纯 Rust 的单线程参考实现，约 12.5 GMAC/s，24.9M 一次决策 71 ms，chinese-ime-lm 的文档写明它不能上按键路径。

下一代重排模型（chinese-ime-lm #94 的 8x448 与手机用的 6x256）在 ctx-v1 上有上下文 91.8%、无上下文 83.4%，但要靠 ONNX Runtime 的多线程 int8 内核才进得了按键预算：桌面 2 线程按键 p95 约 3 ms，iPhone 17 约 3 ms。ONNX Runtime 是 C++ 原生库，而 `crates/engine` 是纯 Rust、还要编到网页引擎的 wasm（`docs/ARCHITECTURE.md` 的分层）。类型写死时，接新后端只能把原生库塞进引擎，或者在 `input-runtime` 里另写一份排序规则，两条都越界。

## Decision

`msime_engine::ordering` 定义 `CandidateScorer`，排序决策只经它用模型：

- `context_length()`：模型一次能读的字数，`rerank_context` 按它截上文。
- `log_probabilities(context, candidates)`：逐字平均自然对数概率。
- `best_where(context, texts, facts)`：带默认实现，规则与 `Reranker::best_where` 相同（首行可信或不回答按键时不动，只比较前 `COMPARED_LIMIT` 即 9 条回答按键的候选，少于两条不动）。

`Reranker` 实现这个接口，三个方法都转给它自己的实现，行为与引入接口之前逐字相同。`rerank_pick` 与 `pick_contest` 取 `&mut dyn CandidateScorer`，原来传 `&mut Reranker` 的调用处（`input-runtime` 的测试、`engine-wasm`）靠自动转换不必改。`Runtime` 的两个字段是 `Option<Box<dyn CandidateScorer>>`；`set_reranker` 与 `set_settled_reranker` 签名不变，另加 `set_candidate_scorer` 与 `set_settled_candidate_scorer` 挂任意实现。接口要求 `Send`，因为停顿评分把模型移到后台线程。

裁决点、词网格分混合、词典首选的豁免仍全部在 `rerank_pick` 里，后端只算分。原生后端（ONNX Runtime 的模型、跨按键缓存）放在引擎之外的 crate 里实现这个接口，由宿主一侧挂上；引擎和网页引擎的编译图里没有原生库。

## Alternatives considered

- **在 `crates/engine` 里直接加 ONNX Runtime 后端** — 最省事，排序和模型在一处，不用新接口。但引擎是纯 Rust 并且要编到 wasm，ONNX Runtime 进不了网页引擎，只能到处加 `cfg`；第三方嵌入引擎（`engine-capi`）也会被迫链一个用不到的 C++ 库。
- **用枚举代替 trait（`enum Scorer { Bundled(Reranker), Onnx(..) }`）** — 静态分发、没有装箱，调用点看得到全部实现。但枚举的定义要知道每个后端的类型，原生后端的类型就得进引擎，回到上一条的问题。
- **chinese-ime-lm 的 `Reranker` 自己支持 ONNX Runtime** — 别的输入法也能用上。但那个 crate 的定位是纯 Rust、无 unsafe、低工具链门槛的参考实现，给它加原生依赖等于换掉它的承诺；它也不需要知道 msime 的排序规则。

## Consequences

- **收益**：换模型后端不再碰排序规则，`rerank_pick` 的行为由同一套测试守住；引擎、网页引擎和 `engine-capi` 的依赖不变。
- **代价**：按键路径多一次虚调用和一次装箱，相对一次模型打分可忽略。默认的 `best_where` 是 `Reranker::best_where` 规则的第二份实现，两边改规则时必须一起改，由 `the_default_pick_matches_the_bundled_reranker` 守住一致。
- **重访信号**：如果排序决策需要模型给出逐字分之外的信息（比如逐候选的置信度、要求模型看拼音），这个接口要扩，届时连同默认规则一起改。

## Verification

- `crates/input-runtime/src/tests/candidate_scorer.rs`：只提供分数、挑选走默认规则的打分器，与随包 `Reranker` 的 `best_where` 在一万多种组合（候选数 1 到 12、哪些回答按键、首行是否可信、模型偏好哪一条）上逐条相同；经 `Box<dyn CandidateScorer>` 调 `rerank_pick` 与直接传 `Reranker` 结果相同。
- `cargo test -p msime-input-runtime -p msime-host-api -p msime-engine-wasm` 与 `cargo test -p msime-engine --lib ordering` 原有用例不改即通过。
