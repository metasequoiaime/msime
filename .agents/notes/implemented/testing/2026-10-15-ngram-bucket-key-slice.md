# Agent Note: n 元表在桶内借用定长键切片

Status: implemented

## Problem

固定百万条二元、三元表的 `NgramTable::lookup` 在桶内逐次从字节映射中取键，每个二分探针都计算字节范围并检查边界。循环结束后用整张表的 `count` 判界，落在桶尾的未命中还会读取下一桶的首键。九键采样中查表是已确认的热点；只减少源码行数不能证明查询变快。

## Decision

保留现有桶索引和文件映射。查询先借用本桶完整键区间，用 `as_chunks::<8>()` 看作 `&[[u8; 8]]`，交给标准库 `partition_point` 找第一个不小于查询键的位置。每次比较仍使用 `u64::from_le_bytes`；只在本桶内检查相等，再按原绝对行号读评分。空桶、未命中返回正零，重复键取首项，评分保留原位模式。

不分配或复制键数组，不增加 unsafe，不修改加载、格式验证、映射长度或桶索引。`as_chunks` 从 Rust 1.88 稳定，满足本仓库 Rust 1.89；`partition_point` 保留首个匹配的下界语义。依据为标准库 [定长切片 API](https://doc.rust-lang.org/std/primitive.slice.html#method.as_chunks)、[下界 API](https://doc.rust-lang.org/std/primitive.slice.html#method.partition_point)及[实现](https://doc.rust-lang.org/src/core/slice/mod.rs.html)。

## Existing decisions

已检索活跃树中的 `NgramTable`、桶索引、二分、`lower_bound` 与 `partition_point`。[活动 n 元表](../../implemented/testing/2026-10-14-active-dictionary-ngram-tables.md)约束映射的持有与资源生命周期，[二元评分缓冲](../../implemented/testing/2026-10-11-lattice-bigram-lazy-reserve.md)处理评分行分配；本项保持它们的决定与查表次数，不替代它们。日文 continuing 查询笔记处理字符串键构造，与本项无关。没有相同的桶内查找提案或实施记录。

## Measurements

固定资源代次 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`，基线 `e6ec6853f338a38ea472fad90e60ed394cc4e2c6`，Rust 1.97.1、macOS arm64 release。两张百万条表分别测命中、随机键、桶尾与混合四种形状，每组 8192 键、每臂四轮、80 对，奇偶交换先后。先逐位比较结果再计时，切片方案新/旧配对中位比为 0.5650–0.7049。

固定 81 条九键评测经引擎 `Session` 输入，关闭学习和频率调整、打开整句备选，记录约 16771180 次查表，每 32 次取一条，共 524100 条。回放按表分组：二元 495429 条（非零命中 3644、桶尾未命中 35572），三元 28671 条（非零命中 160、桶尾未命中 1898）。每臂四轮、40 对交换顺序的中位比为 0.6726 / 0.6991；全部样本评分位模式一致。临时轨迹只含固定评测资源派生的键，不进入产品或仓库，也不拿受记录影响的输入耗时做对照。

## Alternatives considered

- 只保存桶尾边界：改动最小，并能跳过下一桶的额外读取；实测命中形状回退到 1.38–1.41，实际轨迹中位比 1.0109 / 1.0256，没有支持落地的收益。
- 桶内线性扫描：平均十几条键连续存放，代码简单且顺序访存；实测实际轨迹中位比 1.3812 / 1.5177，四种形状也均回退，不采用。

## Testing

独立线性对照覆盖空表、空桶、首末桶、宽度 0/1/2/3/4/15/16/17/31/32/33/64、重复键、整数极值、负零及 NaN payload，查询零分配；该语义测试在旧实现上也通过。

默认忽略的手工 release 预算固定完整旧查找正文，先核对两张表共两百万键及各自前后相邻查询，六百万次评分位模式全部相同，再测四种负载。旧实现真实失败（中位比 1.0059），新实现最终复跑二元 hit/random/bucket-end/mixed 为 0.6275/0.6887/0.5704/0.6561，三元为 0.6607/0.7145/0.5798/0.6881，均低于 0.9。复现：指定固定资源为 `MSIME_NEURAL_MODEL_DIR`，运行 `cargo test --release --locked -p msime-engine bucket_lookup_release_budget -- --ignored --nocapture`。普通 CI 不依赖共享机器或其它 CPU 的速度。

固定同代资源全拼 141 条、九键 81 条的完整质量 JSON 与旧版相同。引擎 2109 项、golden 31 项、runtime 286 项通过，20 项忽略；两 crate 的全部 target 通过 Clippy。验证中发现的语音关闭竞态随[关闭后补读](../bug-fix/2026-10-15-voice-buffered-response-after-close.md)修复，结果槽测试按[发布屏障](2026-10-15-online-provider-result-completion-barrier.md)消除错误的时序假设。

无采样九键按旧/新/新/旧独立顺序运行同一 81 条集合，每轮 1791 次按键，计时期间没有本任务的构建、测试或其它 benchmark 并行。旧版 p95 为 40.44/26.82 ms，新版为 27.70/26.80 ms；旧版首轮有明显长尾尖峰，随后各轮接近，不能据此确认端到端延迟收益。查表局部收益有配对证据，九键 16 ms 预算仍失败，模型矩阵计算等剩余热点继续单独处理。

## Consequences

每个探针只解析借用切片中的八字节键，桶尾未命中无需访问下一桶；查询结果和内存分配保持。收益依赖编译器生成的查找循环和目标 CPU，不能把同机局部查表收益等同于宿主按键延迟或九键 16 ms 预算已通过。切片端点来自加载时已验证的桶索引，键仍按小端读取，不引入对齐假设。映射随资源代次只读持有，生命周期契约保持。

## Related experiments

[桶旁存在位图](../../rejected/testing/2026-10-19-ngram-bucket-presence.md)在本查找前增加32位缺位过滤。未命中有收益，但命中负载超过事先允许的回退，因此撤回；本篇的桶内键切片决定继续保持。
