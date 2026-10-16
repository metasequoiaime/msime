# Agent Note: 运行时重排跨轮复用候选状态缓冲

Status: implemented

## Problem

上游 `chinese-ime-lm::Reranker::score_indices` 每次重排都会新建一个 `next Vec`，保存本轮最多九个候选的编码和 `TailRun`，再替换上一轮的 `resume`。上一轮状态必须在本轮评分期间继续存在，不能直接清空原向量；因此这个外层向量分配会落在九键每次按键热路径上。

## Decision

接入上游 PR #56（提交 `3fa3b57a5634781833e2e07a5cc3eae9727e6ea7`）。上游新增备用状态向量，在评分开始时交换当前轮与上一轮的外层存储，评分结束后清除旧元素但保留其容量；完全命中和共享前缀的遍历优先级不变，前缀失效时清空旧状态。`crates/engine`、`crates/input-runtime`、主锁文件和 fuzz 锁文件统一固定这个提交。

## Existing decisions

已检索 `Reranker`、`resume`、候选编码和上游缓存笔记。[候选索引固定缓冲](2026-10-16-runtime-rerank-selected-indices.md)只消除筛选阶段的两个临时数组，[候选编码所有权转移](2026-10-16-runtime-rerank-tail-ownership.md)只处理 token 缓冲复制；完全命中、同批候选和末位置前向的缓存生命周期继续沿用已有笔记。本片只复用重排状态的外层容量，不改变元素所有权和模型计算。

## Alternatives considered

- 每轮继续新建 `Vec`：实现最简单，但每次评分都申请和释放候选状态的外层存储。
- 在宿主清空并原地重建 `resume`：无法同时保留上一轮状态，完全命中和共享前缀无法正确匹配。
- 固定数组承载可变长度状态：需要额外初始化或不安全代码，且会把元素的动态生命周期硬编码进固定容量；双 `Vec` 只复用已经存在的外层分配。

## Verification

上游 PR 已通过 26 个 Rust 单元测试、14 个模型文件测试、全目标 Clippy（`-D warnings`）、Rustfmt、差异检查和笔记检查；新增测试确认第三轮评分复用备用向量地址，既有缓存寿命测试确认旧 `Arc<TailRun>` 不被备用缓冲延长。`msime` 接入后运行 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast`，结果为 `msime-engine` 2152 个单元测试、31 个 golden 测试和 `msime-input-runtime` 294 个单元测试全部通过；`cargo clippy -p msime-engine -p msime-input-runtime --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 和 `pnpm --config.verify-deps-before-run=false run verify-notes` 均通过。九键固定资源尚未在本轮重新下载，不能把这项局部复用直接换算成端到端 p95 收益。

## Consequences

连续重排复用候选状态外层向量容量，减少一次外层堆分配；备用向量清空元素后只保留容量，上一轮状态寿命与上下文失效规则不变。候选 token、模型状态和分数仍按原路径分配，模型前向成本不变；九键 16 ms 目标继续保持 active。
