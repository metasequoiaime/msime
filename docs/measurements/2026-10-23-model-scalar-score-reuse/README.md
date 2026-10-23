# 单候选模型分数跨批调查

固定基线 `ed24a0a800f026ca5e2ee912603f9c3956136034`，模型源码 pin `15caab4fdeb59c11f452b91884371cf30e2e5608`，资源来源 `128a332cabc5fa42b1cf5f827033ad5697ff528b`，资源代次 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。数据为完整公开合成集 `resources/eval/sentences-nine-key-v1.tsv`：81 条、每臂 1791 键、五条预热，16 ms 预算保持。

## 方法

临时在 `ordering::rerank_pick` 的静态证据评分返回后观察真实分数，所有模型调用、输入次序和状态保持。探针从同一模型文件读取词表，按上游的单 Unicode 字符到 token 映射、UNK=1、BOS=2、最长候选决定的上下文余量及尾部截断构造键；上下文 token 改变即清空。每个重排器分别模拟 16/64/256/1024 个候选的 LRU，每条保存首次填入的分数，命中移动位置但不更新分数。上一批和本批的完全命中单独计数，避免把上游已经省下的评分算成新增覆盖。回退分支只计调用并清空诊断状态，不额外调用模型。

只输出计数与分数位模式，不输出候选文本。实际只有一个重排器、零次回退，所有有效上下文都相同；窗口变化、跨对象指针重用和回退后的缓存寿命没有由这组数据覆盖。虽然跨批完全命中可跳过本次评分，也可能削弱下一批的共享前缀续跑，命中率不能直接换算为可省时间。探针使用线性查找和大量临时分配，其延迟不作正常产品成绩。

## 结果

两轮完整诊断一致：含预热 843 次调用、7587 个候选；第二轮增加预热分界与位差定位，预热为九次调用、81 个候选，之后为 834 次、7506 个。上一批和本批完全重复均为零。

| 单候选槽数 | 命中候选 | 命中调用 | 每次命中数 | 位差次数 |
| --- | ---: | ---: | ---: | ---: |
| 16 | 0 | 0 | 0 | 0 |
| 64 | 0 | 0 | 0 | 0 |
| 256 | 81 | 9 | 9 | 0 |
| 1024 | 504 | 56 | 9 | 1 |

所有命中都在预热之后。1024 槽的 843 个逐次命中位置，与[上一轮整批 128 槽](../2026-10-23-lattice-column-score-cache/README.md)完全一致，没有一至八条候选命中的调用。单候选粒度没有在这个集合显示额外覆盖。

1024 槽第 733 次调用的一个候选出现 1 ULP 位差（确切位置和位模式以 `result.json.gz` 为准），最大绝对差 `4.76837158203125e-7`。上游 `TailRun::joined` 通过 `carried + rest` 分段累加，同样 token 的最终分不保证逐位独立于续跑历史。这里没有证明该差异改变首选，也没有据此修改浮点规则。

两轮探针均退出 1，原因是 16 ms 预算未通过；诊断开销使这些时间不能用于判断正常程序的收益或回退。本轮不增加生产标量缓存，不提交模型状态或词表副本。九键延迟目标仍未解决。

## 复现

在包含本证据目录的仓库外独立工作树执行，使用已校验的同代资源。构建脚本校验两份待接线源码的 SHA-256 等于固定基线；未来源码变化时应检出本调查提交或把已提交的证据目录带到基线工作树，不能跳过校验。产物只放标准 `target/`，不要与其它构建共用此工作树。模型词表只生成到 `target/`，不写入证据交付目录。

```sh
mkdir -p target/model-score-reuse-evidence
gzip -dc docs/measurements/2026-10-23-model-scalar-score-reuse/scalar_probe.rs.gz > target/model-score-reuse-evidence/scalar_probe.rs
export MSIME_PROBE_RESOURCES=/path/to/verified/resources
python3 docs/measurements/2026-10-23-model-scalar-score-reuse/build_probe.py
python3 docs/measurements/2026-10-23-model-scalar-score-reuse/run_probe.py
python3 docs/measurements/2026-10-23-model-scalar-score-reuse/analyze.py
```

`build_probe.py` 的交付版本增加上述基线哈希前置检查，其诊断接线和构建命令与实际运行相同。脚本用 `finally` 逐字恢复 `ordering.rs` 与评测入口，独立复制 release 程序，`binary.json.gz` 保存二进制与源码哈希；`source-restoration.json.gz` 另核对当前源码等于基线。`v1-*` 是未增加预热标记的首轮，没有删去其运行记录。`probe.patch.gz` 是诊断接线快照，绝对路径脱敏后只供审查；复现由脚本重新生成接线。

`manifest.json` 记录原始、路径脱敏后及 gzip 字节的 SHA-256，压缩使用固定 `mtime=0`。内部原始哈希需与 `raw_sha256` 关联，不能直接拿来比脱敏交付文件。没有提交程序、词表、账号资料或凭据。恢复后的 notes、全仓 Rustfmt、diff 与 quick 门禁通过，日志随目录交付。quick 覆盖契约、笔记、Rust 工作区与共享 Apple bridge，其余平台阶段按文档范围跳过；实际推送门禁见本轮 PR。没有为未采用的方案运行产品质量、ABBA 或设备验收。
