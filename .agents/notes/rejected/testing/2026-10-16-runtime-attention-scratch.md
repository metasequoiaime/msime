# Agent Note: attention 双缓冲未通过配对门禁，不接入运行时

Status: rejected — 有上文形状无稳定收益，四个形状超过预设性能门槛

## Problem

九键逐键 p95 仍超过 16 ms。已有热路径采样出现矩阵运算和堆申请释放；前缀和尾部前向的每层 attention 重建输出与 softmax 分数，是一项可独立验证的候选。最新启动采样只覆盖词库准备，不能据此认定按键瓶颈发生变化。

## Proposal

上游在一次 run_prefix 或非空 run_tail_states 内跨层复用 attention.output 和 scores，调用结束即释放；每轮清零输出，softmax 只消费本 head 已重写的可见前缀。不改变模型、搜索预算、候选顺序、KV 所有权或缓存寿命。

## Existing decisions

已检索 attention、前向缓冲和工作区。[前馈缓冲](../../implemented/testing/2026-10-16-runtime-feed-forward-scratch.md)和[最终 KV](../../implemented/testing/2026-10-16-runtime-rerank-packed-kv.md)部分相关，分别继续持有前馈与 KV 决定；矩阵行遍历的既有负结果保持。本片没有覆盖或翻转旧决定，没有完全吸收或过时提案。

## Alternatives considered

- 保留每层申请：局部代码直接，但有重复分配；本轮无上文有收益、有上文没有收益，最终保持原实现。
- 跨按键持有工作区：可减少更多分配，但扩大缓存状态及持有内存，且直接评分接口需要同步；本片只验证单次同步调用。
- 同时复用 QKV、query 和投影：可以省更多分配，但增加覆盖和借用边界；本轮先独立验证 attention 输出与分数，没有把其结果外推到更大范围。

## Verification

上游 [PR #64](https://github.com/metasequoiaime/chinese-ime-lm/pull/64)已合并为 `856e094af0d3c8b85f51181f531a9c45bdff162d`，[固定否决笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/856e094af0d3c8b85f51181f531a9c45bdff162d/.agents/notes/rejected/testing/2026-10-16-attention-scratch.md)保存完整否决笔记、可复现补丁、全形状结果及旧/新完整模型表。基线 `34cc088a7be5d426292fcd8d4d788a613966ba00`；模型 SHA-256 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`；完整模型使用本仓库 `60ae46ff73afeecbfe1efa2a367cd06ae554d715` 的九键合成集 81 例。

暖缓冲地址回归先红后绿；Rust 1.97.1 和 1.75.0 各 37 单测、15 模型格式测试及 doc tests 通过，两个编译器的定向真实模型前缀/尾部 KV、隐藏状态、完整分布逐位一致。Clippy、fmt、笔记与差异检查及独立只读审查通过。最初误启动既有完整词表计时后主动终止，相关计时不采信；正式正确性命令另行重跑。

完整六层残差 attention 链包含输入复制、工作区构造和析构，覆盖 192/768 宽、1/4/12 行、offset 0/19，共 12 种形状各 40 对，交替旧新先后。全部形状采集后一次判断；每 0.5 秒监测编译器和构建器，配对全程未观察到构建重叠，不宣称机器独占。

| 宽度 | 行数 | offset | 新/旧中位比 | 预设门槛 | 结果 |
| --- | --- | --- | --- | --- | --- |
| 192 | 1 | 0 | 0.8408 | < 0.99 | 通过 |
| 192 | 1 | 19 | 1.0062 | < 0.99 | 失败 |
| 192 | 4 | 0 | 0.9575 | < 1.03 | 通过 |
| 192 | 4 | 19 | 1.0311 | < 1.03 | 失败 |
| 192 | 12 | 0 | 1.0103 | < 1.03 | 通过 |
| 192 | 12 | 19 | 1.0406 | < 1.03 | 失败 |
| 768 | 1 | 0 | 0.9398 | < 1.03 | 通过 |
| 768 | 1 | 19 | 1.0318 | < 1.03 | 失败 |
| 768 | 4 | 0 | 0.9813 | < 1.03 | 通过 |
| 768 | 4 | 19 | 1.0296 | < 1.03 | 通过 |
| 768 | 12 | 0 | 0.9762 | < 1.03 | 通过 |
| 768 | 12 | 19 | 1.0226 | < 1.03 | 通过 |

两次完整模型表退出 0，但分别 44/27 次采样观察到其他构建，只作受干扰观察，不归因收益或回退；没有为寻找通过结果反复测量。十五字列没有有效候选，零值不能当成真实十五字决策。

本仓库仅增加本篇笔记。`pnpm --config.verify-deps-before-run=false run verify-notes`、差异检查与 `env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过；Rust 工作区及共享 Apple bridge 编译通过，各平台阶段按文档改动范围跳过。没有设备验收，也没有把上游实验局部结果当作主项目逐键实测。

## Consequences

本片只记录负结果，主项目仍固定 `6ce2cbb5b76e189a1ef6ff11eaa0435bfd9983a0`，不修改生产源、资源、质量基线或性能预算。减少申请不能直接推断整体提速；不能以无上文约快 16% 的局部形状掩盖有上文失败。没有证明编译器层面的回退根因，别名和内联等解释仍只是猜想。九键 16 ms 目标保持 active。
