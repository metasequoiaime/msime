# Agent Note: 九键整句重排借用词面

Status: rejected — 普通 System 配对计时未达到冷重排收益门槛

## Problem

九键 `NineKeySession::rerank_sentences` 在模型缓存未命中时，会把已有的 `WordItem` 逐条复制为临时 `SentencePath`，只为给模型提供词面和词网格静态分。模型评分只读取 `sentence` 与 `log_prob`；`key`、`words` 和 `typo_edges` 在这条路径不会被读取，复制它们增加了每次九键刷新都要支付的堆分配和字符串带宽。

## Proposal

在 `NeuralReranker` 增加只接收借用词面、静态分和调用方排列缓冲的评分入口。九键直接从已拥有的 `WordItem` 借用最多 `MAX_RERANK_PATHS` 条词面，调用该入口取得排列，再只复制模型结果需要写入 `RerankEntry.ranked` 的顺序字符串。缓存签名继续在未命中时保存词面和 `comparable_weight`，命中比较、上下文、模型评分、稳定排序、候选放置和缺失模型行为保持不变。26 键的 `SentencePath` 路径不改。

## Historical audit

已检索 `rerank_sentences`、`SentencePath`、`RerankEntry`、候选路径所有权、运行时重排文本缓冲和九键重排缓存。已有九键重排路径字段笔记只裁剪 26 键临时路径中不参与模型评分的字段；本片覆盖九键引擎内缓存未命中入口的 `WordItem` 到模型边界。已有九键重排缓存借用笔记只处理缓存命中比较；本片不改变缓存签名和命中契约。模型上游的 `Reranker::log_probabilities` 已接收借用文本切片，主项目不复制模型实现。

## Alternatives considered

- 保留 `SentencePath` 临时转换：接口复用简单，但每个候选都复制不用的键、词段和错误边计数。
- 让 `NeuralReranker` 接收 `WordItem`：会把模型层耦合到九键候选类型，扩大引擎边界；借用文本与静态分已经足够表达契约。
- 跨刷新保存借用词面：候选快照会替换，借用寿命无法跨调用；只在当前刷新内借用，缓存继续拥有字符串。
- 改变模型候选上限或九键缓存：会改变质量和路径契约，与本片无关。

## Acceptance criteria

1. 先在生产实现上加入冻结旧入口的对照测试，证明完整九键缓存未命中重排仍会为每条整句创建临时路径，并在候选入口中红测失败。
2. 候选实现逐位保持模型排列、缓存签名、缺失模型和候选放置语义；测试覆盖 0/1/2/12 条整句、混入非整句行、重复词面、上下文变化和热缓存命中。
3. 普通 `System` 分配器的 release 配对计时覆盖 2/6/12 条整句、冷缓存和热缓存，候选/旧中位比目标：冷重排 < 0.95，热重排 < 1.05；同一进程交替运行，校准至少 5ms，无构建干扰。任一目标失败即撤回生产改动，只提交否决证据。
4. 通过相关 engine 测试、完整质量 JSON 对照、Clippy、fmt、`pnpm run verify-notes` 和 quick 后，才保留生产改动并交付独立 PR。九键默认 Runtime 的 16ms p95 仍需单独测量，不能由局部分配收益替代。

## Risks

借用文本数组和排列缓冲必须在模型调用结束前保持候选快照不变；缓存未命中时仍要拥有独立的 `ranked` 字符串，不能把短暂借用逃逸到下一次刷新。空模型、少于两条和模型返回长度不符时要继续走旧的整句移除规则。局部减少复制不等于端到端按键预算达标。


## Verification result

候选实现已在最新 `origin/develop`（`395b2f10a5`）上完成验证。`cargo test --locked -p msime-engine --lib nine_key:: --no-fail-fast` 通过（212 项通过，13 项忽略），借用词面一致性测试通过，九键整句质量 JSON 与 `resources/eval/baseline-nine-key-sentences.json` 逐字节一致。普通 `System` 分配器的 release 配对计时交替覆盖 2、6、12 条整句，校准至少 5ms；新/旧中位比分别为 `1.0096`、`0.9982`、`0.9986`。冷重排预设门槛为 `<0.95`，三档均失败，因此撤回 `neural.rs` 与 `nine_key.rs` 的生产改动，不提交该优化。

同一候选的九键端到端 `rerank_latency` p95 为 `19.88ms`，仍超过 `16ms` 帧预算；局部临时路径计时没有证明能改善该目标。
