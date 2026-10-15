# Agent Note: 重排候选编码缓冲直接转移

Status: implemented

## Problem

上游 `chinese-ime-lm::Reranker::score` 为每个候选编码一次 token `Vec`，随后为了保存到下一轮 `resume`，在首次评分和完全命中复用两条路径再次调用 `to_vec()`。九键模型重排的 Transformer 前向之外，这个复制是每个候选都承担的额外分配和内存带宽成本；它还与既有的完整命中、同批前缀和末位置延迟前向缓存叠加。

## Decision

上游 `chinese-ime-lm` 通过 [PR #53](https://github.com/metasequoiaime/chinese-ime-lm/pull/53) 将截断后的编码向量直接移动进 `resume`：评分循环先对拥有的 `Vec<u32>` 原地截断，完全命中和新建 `TailRun` 都直接移动这份向量，不再复制 token 缓冲。上游合并提交固定为 `9cf7f0c80d528f852fb0124651a000f41bcd7a00`；`crates/engine`、`crates/input-runtime`、主锁文件和 fuzz 锁文件使用同一提交。

候选文字、截断窗口、比较顺序、续跑状态、浮点计算和缓存失效规则保持不变。测试专用指针夹具验证首次评分和完全命中复用时，保存的 token 缓冲地址就是编码结果的地址；上游原实现对这两个路径均失败。

## Existing decisions

[宿主句子重排复用完全命中的候选推理](../bug-fix/2026-10-15-runtime-rerank-complete-cache.md)规定上游重排状态的生命周期和缓存失效；[同批候选前缀复用](../bug-fix/2026-10-15-runtime-same-batch-cache.md)规定本轮候选的共享状态；[候选末位置按需前向](../bug-fix/2026-10-15-runtime-defer-terminal-forward.md)规定末 token 的前向边界。本项只减少编码向量所有权转移前的复制，继续沿用这些状态契约。

检索活跃的 `Reranker`、`resume`、候选编码和上游提交笔记，没有发现已经覆盖这次 token 缓冲所有权决定的笔记；没有完全吸收或过时提案。

## Alternatives considered

- **只在完全命中路径复用编码向量**：可以先覆盖最热的重复候选，但新候选路径仍为每个候选保留一次多余复制，且两条路径会有不同的所有权规则；不采用。
- **在 `Reranker` 上增加跨轮编码缓存**：可以跳过重复候选的字符编码，但需要处理上下文失效、候选顺序和容量上限，扩大缓存状态；不采用。当前切片只转移本轮已经生成的向量。
- **改为共享 `Arc<[u32]>`**：可以让多个重复候选共享 token，但会增加引用计数和切片构造成本；不采用。当前 `resume` 已通过 `Arc<TailRun>` 共享推理状态，token 列表只需移动一次。

## Verification

上游工作树在合并前通过 `cargo fmt --manifest-path reference/Cargo.toml -- --check`、`cargo test --manifest-path reference/Cargo.toml --locked`（20 个单元测试、14 个模型文件测试）和 `cargo clippy --manifest-path reference/Cargo.toml --all-targets --locked -- -D warnings`。指针测试在旧实现上先失败，改动后通过。

主项目更新四处固定来源后，运行引擎和 runtime 单测、Clippy、fmt、笔记检查与 `git diff --check`，并用固定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe` 和 `resources/eval/sentences-nine-key-v1.tsv` 重跑九键延迟基准。模型臂 mean 9.12 ms、p95 22.30 ms，重排增量 mean 3.33 ms、p95 12.91 ms；相对当前基线模型臂 p95 22.76 ms 有小幅下降，但仍超过 16 ms，目标继续 active。`bash scripts/verify-local.sh --quick` 在提交前运行。

## Consequences

每个候选少一次 token 缓冲复制和对应分配，`resume` 仍拥有自己的可变候选列表，状态生命周期不变。编码 `Vec` 的容量由原始候选长度决定，`truncate` 不改变它；超出模型上下文的候选因此可能保留未使用容量，但这是旧实现编码后复制时同样存在的容量上界，不新增常驻缓存。

这项改动不改变模型权重、前向矩阵、候选质量或公开接口；它只减少候选 token 的内存操作。九键端到端延迟仍需固定资源实测，不能从局部分配减少推断已达到 16 ms 目标。
