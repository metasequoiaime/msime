# Agent Note: 大模型只比较得了九条时改用小模型配整池

Status: implemented

## Problem

Mac 上自动打字验收（typing-chat-v1 的 test，220 段），2.0 的首选正确率是 71–78%，每轮错的几乎都是同一批句段。用 `convert_eval` 复现：8x448 配 9 条，chat-test 有 58 段首选错，其中 36 段的正确答案根本不在候选列表里。这些句段在 2.0 里没有任何改进的可能。

原因是交给逐键模型比较的整句池太小。九条整句常常前半句都一样，开头一选错（`fangan` 成了「反感」，`shangxiande` 成了「死伤闲的」），九条就全错了。

[整句池按设备速度定](2026-11-02-sentence-pool-by-device-speed.md)让 Mac 留在「8x448 配 9 条」：大模型一页 4.1 ms，桌面 p99 ≤ 4 ms 的预算里只放得下九条。那篇笔记考虑过「6x256 配 32 条」，因为 test 上有升有降没有采用，并写明「不为池大一些换模型」。这一篇推翻那一条。

## Decision

两个模型都装着，大模型过了换档线（`LARGE_RERANK_MODEL_BUDGET_MS`）、但按 `sentence_pool_for` 在按键预算里只比较得了九条（`SENTENCE_POOL`），而小模型比较得了整池（`MAX_SENTENCE_POOL`，32 条）时，挂小模型配整池。这一步写在 host-api 的 `OnnxRerankers::small_for_pool`，选档时和刚加载、用量过的结论两条路都会走到。

- 小模型的自测结论同样按换档线记下（`accept_calibration`），在线内时照样排上屏后联想，和大模型一样。
- 大模型能比较更多条的机器不受影响：iPhone 17 这类手机上大模型能比较 24 条，照旧用它。只装了一个模型的、内存不够只能用小模型的、大模型过不了换档线的，也都和以前一样。
- 诊断行写成 `keyboard reranker onnx small`，细节带 `(large … ms compares 9, small … ms compares 32)`。

按机器类型，结果是：

| 机器 | 以前 | 现在 |
|---|---|---|
| Mac（M 系列） | 8x448 配 9 条 | 6x256 配 32 条 |
| iPhone 17 | 8x448 配 24 条 | 不变 |
| Pixel 3 XL | 6x256 配 9 条 | 不变 |

## 评测

2026-11-03，同一台 Mac（M 系列，2 线程），锁定的词库，资源里去掉随包的 `sentence-model.safetensors`，`convert_eval --onnx-model … --sentence-pool …`。聊天集是从 typing-chat-v1 按标点切出的句段，上文是同一段会话里前面的文字（最多 64 字）：dev 110 段、test 220 段，与自动打字验收用的是同一批。首选准确率：

| 集合 | 8x448 配 9 | 8x448 配 16 | 8x448 配 32 | 6x256 配 32 |
|---|---|---|---|---|
| typing-chat dev（110） | 62.7% | 65.5% | 66.4% | 68.2% |
| typing-chat test（220） | 73.6% | 76.4% | 77.7% | 79.5% |
| sentences-v1（60） | 83.3% | — | — | 83.3% |
| sentences-neutral-v1（1105） | 83.2% | — | — | 83.9% |
| sentences-v2（312） | 78.8% | — | — | 76.9% |
| sentences-particles-v1（103） | 81.6% | — | — | 81.6% |
| quanpin-words-v1（2791） | 72.5% | — | — | 72.5% |

正确答案出现在候选列表里的比例：chat-test 从 83.6% 到 90.0%。

延迟用 `rerank_latency` 在同一台 Mac 上量，sentences-v2 加 chat-test 共 532 条、每臂 11351 次按键，两种配置交替跑两轮。下面是带模型时的整键耗时（ms）：

| 配置 | p95 | p99 |
|---|---|---|
| 8x448 配 9 | 2.93 | 3.74–3.75 |
| 6x256 配 32 | 2.30–2.31 | 3.09–3.12 |

小模型在 ctx-v1 上有上下文 90.4%、无上下文 83.0%，守得住有上下文 ≥ 90%、无上下文 ≥ 80% 的底线。

## Alternatives considered

- **8x448 配 32 条。** 每个集合都不比 9 条差，这是它最强的理由（上一篇的 test 表）。但桌面整键 p99 按那篇的系数是 0.7 + 4.1 × 1.86 ≈ 8.3 ms，超出 p99 ≤ 4 ms 的预算一倍。而且在聊天集上，它比 6x256 配 32 条还低 1.8 个点。
- **8x448 配 16 条。** 池 16 的 p99 实测 5.84 ms，也超出预算。聊天集上又比 6x256 配 32 条低 2.7 / 3.1 个点。
- **维持 8x448 配 9 条（上一篇的结论）。** 理由是在 ctx-v1 选词和 sentences-v2 上 6x256 配 32 条会掉几条。sentences-v2 这里确实低 1.9 个点。但用户日常的聊天打字高了约 6 个点，延迟还更低，p99 也从贴着 4 ms 变成留有余量。用户报告「每次都错的那几句一点进步都没有」，量的就是聊天打字。
- **改整句解码的打分，让池里本来就有正确答案。** 试过两条：给「X的」「X了」这类条目扣成词奖励，和句首的「吗」「吧」让位。前一条在 sentences-neutral 上掉了 1.1–5.1 个点，没有采用；后一条要等宿主把消息之间的回车告诉 runtime 才起作用，另开 PR。

## Consequences

- **收益**：桌面聊天打字首选约高 6 个点，桌面整键 p95、p99 各快约 0.6 ms。
- **代价**：sentences-v2 首选低 1.9 个点，全拼词 top5 低 1.1 个点（top1 不变）。桌面挂的是精度略低的模型，「增强」在只比较少数几条的场合（九宫格、单字裁决）比以前稍弱。
- **重访信号**：大模型提速（比如换 ONNX Runtime 或量化方式）后，桌面上它能比较 16 条以上时，要重新比较「8x448 配大池」和「6x256 配 32 条」。

## Verification

- host-api：`keyboard_reranker_prefers_the_onnx_model_when_one_is_installed` 用按调用先后给池的规则覆盖这条路径：大模型九条、小模型整池时挂小模型配 32 条，并且排上屏后联想；放开再挂时用量过的结论，仍是小模型。同一个测试里「两个模型都只放得下九条时仍用大模型」的断言不变。
- 评测命令：`convert_eval --resources <去掉 sentence-model 的资源> --resource-lock <对应的锁> --onnx-model <rerank-model-small> --sentence-pool 32 --set <句段集>`；延迟：`rerank_latency --onnx-model … --sentence-pool …`。
