# Agent Note: 重排候选编码缓冲直接转移

Status: implemented

## Problem

上游 `chinese-ime-lm::Reranker::score` 为每个候选编码一次 token `Vec`，随后为了保存到下一轮 `resume`，在首次评分和完全命中复用两条路径再次调用 `to_vec()`。九键模型重排的 Transformer 前向之外，这个复制是每个候选都承担的额外分配和内存带宽成本；它还与既有的完整命中、同批前缀和末位置延迟前向缓存叠加。

## Decision

上游 `chinese-ime-lm` 通过 [PR #53](https://github.com/metasequoiaime/chinese-ime-lm/pull/53) 将截断后的编码向量直接移动进 `resume`：评分循环先按原有窗口限制 Unicode scalar 编码，完全命中和新建 `TailRun` 都直接移动这份向量，不再复制 token 缓冲。完整文本编码入口保持不设上限。上游 [PR #54](https://github.com/metasequoiaime/chinese-ime-lm/pull/54) 随后修复超窗口容量回退，当前固定提交为 `541ccb1e86c5bec67a6aa8b0d9dc6ae9bbb7f12f`；`crates/engine`、`crates/input-runtime`、主锁文件和 fuzz 锁文件使用同一提交。

候选文字、截断窗口、比较顺序、续跑状态、浮点计算和缓存失效规则保持不变。测试专用指针夹具验证首次评分和完全命中复用时，保存的 token 缓冲地址就是编码结果的地址；回放上游原评分流程时，这两个路径均因缓冲指针变化失败。

## Existing decisions

[宿主句子重排复用完全命中的候选推理](../bug-fix/2026-10-15-runtime-rerank-complete-cache.md)规定上游重排状态的生命周期和缓存失效；[同批候选前缀复用](../bug-fix/2026-10-15-runtime-same-batch-cache.md)规定本轮候选的共享状态；[候选末位置按需前向](../bug-fix/2026-10-15-runtime-defer-terminal-forward.md)规定末 token 的前向边界。本项只减少编码向量所有权转移前的复制，继续沿用这些状态契约。

检索活跃的 `Reranker`、`resume`、候选编码和上游提交笔记，没有发现已经覆盖这次 token 缓冲所有权决定的笔记；没有完全吸收或过时提案。

## Alternatives considered

- **只在完全命中路径复用编码向量**：可以先覆盖最热的重复候选，但新候选路径仍为每个候选保留一次多余复制，且两条路径会有不同的所有权规则；不采用。
- **在 `Reranker` 上增加跨轮编码缓存**：可以跳过重复候选的字符编码，但需要处理上下文失效、候选顺序和容量上限，扩大缓存状态；不采用。当前切片只转移本轮已经生成的向量。
- **改为共享 `Arc<[u32]>`**：可以让多个重复候选共享 token，但会增加引用计数和切片构造成本；不采用。当前 `resume` 已通过 `Arc<TailRun>` 共享推理状态，token 列表只需移动一次。

## Verification

上游工作树在合并前通过 `cargo fmt --manifest-path reference/Cargo.toml -- --check`、`cargo test --manifest-path reference/Cargo.toml --locked`（20 个单元测试、14 个模型文件测试）和 `cargo clippy --manifest-path reference/Cargo.toml --all-targets --locked -- -D warnings`。最终指针测试回放旧评分流程时失败，恢复优化后通过。最初使用复制计数的红绿测试已被实际指针对照替换。

主项目更新四处固定来源后，运行引擎和 runtime 单测、Clippy、fmt、笔记检查与 `git diff --check`，并用固定资源 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe` 和 `resources/eval/sentences-nine-key-v1.tsv` 重跑九键延迟基准。当前模型臂 mean 8.63 ms、p95 21.14 ms，重排增量 mean 3.11 ms、p95 12.52 ms；模型臂 p95 仍超过 16 ms，目标继续 active。该轮 `MSIME_BUILD_JOBS=4 CARGO_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，含 Rust 工作区、Android 目标与 Java 宿主、WASM、Linux 设置组件及 87 项原生测试、macOS 与共享 Apple bridge 编译；Windows 交叉构建和 HarmonyOS 完整构建因缺环境跳过。引擎实际为 2139 项通过、28 项既有忽略，另有 31 项 golden 通过，runtime 288 项通过。

## Consequences

每个候选少一次 token 缓冲复制和对应分配，`resume` 仍拥有自己的可变候选列表，状态生命周期不变。初次直接移动版本使用 `truncate`，只缩短长度而保留原始候选容量，确实使缓存的持有空间相对旧版截断切片复制发生回退。当前改为编码前限制窗口：8 位置模型面对 65536 字的合成长候选时，编码及缓存长度均为 7，容量受窗口数量级约束；原始直接移动实现该测试保留 98306 容量并失败。不再把原始输入决定的瞬时容量上界与旧缓存的窗口上界混为一谈。

这项改动不改变模型权重、前向矩阵、候选质量或公开接口；它只减少候选 token 的内存操作。九键端到端延迟仍需固定资源实测，不能从局部分配减少推断已达到 16 ms 目标。

## Bounded encoding verification

上游容量预算先在直接移动版本失败，修复后汉字、ASCII 和 emoji 冷热查询通过。固定 #53 的完整评分流程进行 180 轮对照，分数位模式、缓存 token 顺序、KV 和累计分一致；Unicode 前缀对照覆盖空串、组合符、补充平面字符和未知字。Rust 1.97.1 与 1.75.0 各通过 23 项单元测试与 14 项模型文件测试，保留既有三精度序列与原位移动回归。主项目门禁与九键测量已完成：模型臂 p95 21.14 ms，重排增量 p95 12.52 ms，九键 16 ms 目标仍未达成。
