# Agent Note: 挂着 2.0 时停顿后不再用 1.0 重排

Status: implemented

## Problem

2026-11-03 在 Mac 上用真实输入法做 220 段的完整验收（每键 143 ms，停 300 ms 再按空格）。今天合进去的三个排序修复让离线评测的 chat-test 从 73.6% 涨到 79.1%，真实输入法却只从 70.9% 到 73.2%：除了专门修的那 41 段，其余 179 段一段都没变好。

停顿约 300 ms 后，桌面宿主会做一次落定重排（`Runtime::rerank_settled`，开关是 `sentence_association.neural_desktop`）。它用的落定模型是随包的 `model-v1` 桌面版（`sentence-model-desktop.safetensors`），比逐键挂的 ONNX 模型（2.0）弱，把 2.0 排对的又换掉了。

离线复现：把用户机器上同一份落定模型放进 `convert_eval` 的资源目录，6x256 配 32 条的 chat-test 从 79.1% 降到 73.6%、dev 从 68.2% 降到 61.8%。复现的首选和真实输入法的首选在 220 段里有 192 段一字不差。

## Decision

`CandidateScorer::supersedes_settled_rerank`（默认 `false`）：挂着的逐键打分器报 `true` 时，`Runtime::settled_rerank_blocked` 为真，同步（`rerank_settled`）、异步（`begin_settled_rerank`）两条落定重排都不做，逐键排出的就是最终顺序。

- `OnnxScorer` 报 `true`，host-api 的 `SharedOnnxScorer` 转发。
- 随包的 `Reranker`（1.0）不报。

分平台的结果：

- **跑得了 ONNX 的平台（macOS、Windows、Linux、iOS、64 位安卓）**：只用 2.0。
- **跑不了的平台（HarmonyOS、32 位安卓、Win7）**：逐键和落定都照旧用 1.0。

落定模型照旧加载，「增强」关掉、逐键打分器摘下时，落定重排马上恢复。

## Alternatives considered

- **落定重排改用 ONNX 模型，配更大的池。** 停顿后有时间比较更多读法，可能比逐键更准。但要在会话线程之外跑共享的 ONNX 会话，还要另调池和预算；这次先停掉有害的那一步，量出来已经回到离线水平。
- **宿主侧在挂 ONNX 时把 `neural_desktop` 关掉。** 能达到同样效果，但要改每个桌面宿主，还会改写用户的偏好。放在 runtime 里，所有宿主一次生效，偏好不动。
- **把 1.0 整个删掉。** HarmonyOS、32 位安卓、Win7 没有 ONNX Runtime，1.0 是那里唯一的模型，不删。

## Consequences

- **收益**：真实输入法停顿后按空格的结果，和离线评测的 2.0 一致；聊天打字 chat-test 首选恢复到 79.1%。
- **代价**：落定模型在挂着 ONNX 的桌面上仍会加载（约 25 MB）却不用。如果要省这份内存，下一步可以在 ONNX 挂上后不加载它。
- **已知边界**：ONNX 模型装着、但这台机器超出按键预算没挂上时，没有逐键打分器，落定重排照旧用 1.0。

## Verification

- `crates/input-runtime/src/tests/candidate_scorer.rs` 的 `a_stronger_keystroke_scorer_leaves_no_settled_rerank`：报 `true` 时同步、异步两条都不重排；不报时照旧重排。
- host-api 的 `keyboard_reranker_prefers_the_onnx_model_when_one_is_installed`：宿主挂的那一份报 `true`。
- 离线：资源里带落定模型，6x256 配 32 条，chat-test 73.6% → 79.1%，dev 61.8% → 68.2%。
