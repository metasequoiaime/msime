# Agent Note: 九键模型临时路径只保留重排所需字段

Status: implemented

## Problem

`NineKeySession::rerank_sentences` 为每条候选整句构造 `SentencePath` 时，除了模型读取的词面和静态分，还复制规范拼音 `key` 与词段 `words`。这些字段只在词网格整句回写候选时有意义；九键入口只取模型排好的词面，不会把临时路径重新合并回词网格。长句刷新因此为模型前向前后承担了多余的字符串和向量申请。

## Decision

九键交给 `NeuralReranker` 的临时 `SentencePath` 将 `key` 和 `words` 设为空值，只复制 `word` 到 `sentence`，保留原有 `log_prob`、候选顺序、模型输入和重排结果。`NeuralReranker::rerank` 只读取 `sentence` 与 `log_prob` 并移动路径顺序；九键随后只取 `sentence` 生成缓存排名，所以空字段不会改变输出或缓存签名。

## Historical audit

已检索 `SentencePath`、`NeuralReranker::rerank`、`rerank_sentences` 和九键缓存相关笔记。[九键神经重排缓存借用候选行](2026-10-16-nine-key-rerank-cache-borrow.md)只减少缓存命中时的签名和排名复制；[运行时候选状态复用](2026-10-16-runtime-rerank-resume-buffer.md)只影响模型内部的连续前向；[词网格路径字段所有权](2026-10-15-lattice-outgoing-borrow.md)约束真实整句行的 `key` 与 `words`，不覆盖本处一次性模型输入。三者均保留，没有完全吸收或过时决定。

## Alternatives considered

- **继续复制完整路径**：实现最简单，也能让临时路径看起来与词网格路径相同，但模型入口不会读取这些字段，长句每次重排都会重复申请。
- **修改上游 `SentencePath` 或模型 API**：可以把模型需要的字段缩成专用类型，但要先做上游 PR 并扩大跨仓库契约；本片在现有 API 边界内使用空字段。
- **跨刷新缓存完整路径**：能进一步复用词段，但会延长候选行和会话状态寿命，且缓存失效规则与九键候选变化耦合；本片只收窄单次临时对象。

## Verification

- `cargo test -p msime-engine nine_key::tests --locked`：131 项九键测试通过。
- 固定 `target/verified-resources`（词库与键盘模型均按锁校验）分别用改动前、改动后二进制运行 `convert_eval --grid nine`；两份 81 条结果 JSON 逐字段相等。
- `cargo fmt --all -- --check` 与 `cargo clippy -p msime-engine --all-targets --locked -- -D warnings` 通过。
- 当前机器在完整逐键计时时出现明显系统抖动，模型臂 p95 观察值远高于历史范围，不能把本片宣称为稳定的端到端收益；九键 16 ms 目标继续保持 active。局部收益是去掉未读取字段的确定性申请，后续仍需独占环境下重新配对测量。

## Consequences

每个模型候选临时路径不再为规范拼音和词段拥有字符串或向量；模型的词面、静态分、候选顺序、缓存匹配和质量结果保持不变。临时路径若未来被改为重新进入词网格合并，必须恢复这些字段或改用明确的合并数据源。该切片不改变上游模型、搜索预算、文件格式或资源，也不单独证明九键 16 ms 达标。
