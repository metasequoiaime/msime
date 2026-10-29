# Agent Note: 「增强」只挂 runtime 逐键重排器，引擎内键盘模型不再跟同一开关

Status: implemented

## Problem

移动端整句联想「增强」写的是偏好 `sentence_association.neural_keyboard`。host-api 拿它同时打开两层模型：一层是 runtime 的逐键重排器（`ffi::keyboard_reranker` 挂到 `Runtime::set_reranker`），另一层是引擎自己的 `NeuralReranker`（`engine_sentence_association` 把偏好原样写进 `SentenceAssociationOptions::neural_keyboard`，九键在 `NineKeySession::rerank_sentences` 跨切分重排一次，全拼和双拼在词库里重排，模型挑的整句作为 `NeuralKeyboard` 行插在词网格最好的整句之后，见 [九键遵守句子联想设置](2026-10-09-nine-key-sentence-options.md)）。

2026-10-28 的分层测量（`typing_sim` 严格首选准确率 CFA，`rerank_latency --model-arm runtime|engine|both`，M4 Pro）把两层拆开量了一遍：

| 键盘 | 关 / 只 runtime / 只引擎 / 两层 | 九键按键 p95（ms） |
| --- | --- | --- |
| 九键聊天集 | 0.518 / 0.530 / 0.517 / 0.530 | 关 5.8–6.5，只 runtime 10.3，只引擎 11.7，两层 15.9–17.4 |
| 九键 Tatoeba | 0.502 / 0.517 / 0.502 / 0.517 | |
| 14 键 | 0.633 / 0.657 / 0.633 / 0.657 | |
| 全键盘 | 0.766 / 0.782 / 0.766 / 0.788 | 全键盘 p95：关 0.8，只 runtime 3.8，只引擎 12.3，两层 13.6–13.7 |

九键和 14 键的增益全部来自 runtime 那一层；引擎那一层在九键上单独开反而略降，与 runtime 同开时不加分，全键盘只多 0.6 点。引擎层却让九键按键 p95 再涨 6–7 ms、全键盘再涨约 10 ms，两层同开比只开 runtime 多约 40 MB 内存（+70 MB 对 +30 MB），安卓模拟器上两层同开九键 p95 70 ms。要决定「增强」能不能默认打开，先得把它降到只付 runtime 那一层的代价。

## Decision

- 「增强」只挂 runtime 的逐键重排器。是否挂由 host-api `keyboard_reranker_enabled` 决定：偏好 `neural_keyboard` 打开、且本版本 `edition.features.neural_keyboard` 为真。建会话（`ffi/session.rs`）和应用偏好（`HostSession::apply_pending`）都用它；`ffi::keyboard_reranker` 改为接收这个布尔值和资源目录，不再读 `EngineOptions` 里的引擎开关。
- `engine_sentence_association` 把引擎的 `SentenceAssociationOptions::neural_keyboard` 固定为 false。引擎层的代码、选项和测试原样保留，默认关，留给直接构造选项的评测和实验（`convert_eval --keyboard-neural`、测量补丁里的 `rerank_latency --model-arm engine|both`）。
- 不新增偏好字段。引擎选项本身就是独立开关，评测直接设它；给用户加一个隐藏偏好要动偏好 schema、各端合并逻辑和发版偏好快照，而真机上需要比较的只有「关」和「只 runtime」两臂。
- 「增强」的缺省值仍是关；是否默认打开等真机测量后另行决定。
- 设置文案不改：Android「增强增加神经模型重排」、iOS、HarmonyOS（共享设置页）的描述都不区分层次，只开 runtime 层后依旧准确。
- `apply_local_mode_resource_gates` 里对引擎开关的版本门控保留：宿主不再打开它，这一行对 host-api 构造的选项不起作用，但仍表达「不带模型的版本不交给 Engine」这条约束。
- 桌面不受影响：桌面设置页写的是 `neural_desktop`，它只控制 runtime 的 settled 重排（`Runtime::set_settled_rerank_enabled`），从来不进引擎，没有同样的耦合。桌面宿主同样经 host-api 读 `neural_keyboard`，界面不提供这个开关；手改偏好文件打开它时，桌面也只挂 runtime 层。

## Alternatives considered

- **删除引擎内的键盘模型层**：最彻底，少一份模型加载和上下文缓存。但它是现成的实验臂，`convert_eval`、`rerank_latency` 的分层测量都靠它；以后如果给九键单独训练模型、或者改放置规则，它可能重新有用。只断开偏好到它的连线就能拿到全部延迟收益，删代码没有额外收益。
- **给引擎层加一个独立的隐藏偏好（例如 `neural_keyboard_engine`，默认关）**：能在真机上直接比较两层同开。代价是偏好 schema、Android/iOS/HarmonyOS 的合并保留逻辑和发版偏好快照测试都要跟着改，而两层同开的数据已经在模拟器和本机上量过；需要时再加。
- **保持两层同开，只优化引擎层的耗时**：`.agents/notes/rejected/testing/2026-10-19-nine-key-rerank-empty-shortcut.md` 已经试过在引擎层里抄近路，收益不够；而且引擎层本来就不加 CFA，优化它不如不开它。

## Consequences

- **收益**：打开「增强」的九键、14 键首选准确率与两层同开相同，九键按键 p95 从 16–17 ms 降到约 10 ms，内存增量从 +70 MB 降到 +30 MB；全键盘 p95 从 13.6 ms 降到 3.8 ms。
- **代价**：全键盘 CFA 比两层同开少 0.6 点（0.788 → 0.782）。打开「增强」后候选里不再出现引擎插入的 `NeuralKeyboard` 整句行（候选预览里的 `sentence_model` 来源），排序变化全部来自 runtime 重排。九键笔记里「词网格关闭而模型打开时由模型行占位」的组合，宿主不再走到：「增强」这一档本来就同时打开词网格。
- **重访信号**：如果出现专门给九键或全键盘训练、在引擎层确实加分的模型，或者真机测量显示 runtime 层单独的延迟仍超预算，需要重新评估两层的分工。

## Verification

- `cargo test -p msime-host-api`：`enhanced_mounts_the_runtime_reranker_without_the_engine_layer`（偏好开、full 版挂 runtime 重排器，五笔版不挂，交给 Engine 的选项里引擎层关着）、`sentence_model_switches_follow_the_preferences`（建会话和应用偏好之后引擎层都关着）、`keyboard_reranker_follows_the_keyboard_model_switch`（需要 `MSIME_NEURAL_MODEL_DIR` 或 `MSIME_EVAL_RESOURCES` 指向带 `sentence-model.safetensors` 的目录；开关切换时 runtime 重排器挂上又摘掉，引擎层始终关）。
- 延迟和 CFA 的分层数字来自未入库的测量补丁（`rerank_latency --model-arm`、`typing_sim --latency`，叠在 `typing_sim` 示例 PR #10183 上）。
