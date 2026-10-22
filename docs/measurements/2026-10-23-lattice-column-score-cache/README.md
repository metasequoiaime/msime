# 九键评分缓存调查

基线 `5ea4ec53128cd921fcd83b80472d9323645aadc5`，资源提交 `128a332cabc5fa42b1cf5f827033ad5697ff528b`、代次 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。完整公开合成集合 `sentences-nine-key-v1.tsv`，81 条、模型开/关两臂各 1791 键、5 条预热；预算仍为 16 ms。调查不改变候选预算、模型或评测数据。

## 探针

`baseline.sample.txt.gz` 是受外部编译干扰的 20 秒 CPU 采样，只定位热点，不提供正常延迟成绩。栈顶叶子包括模型 `linear_into`、`dot_i8`、`BigramMemo::score`、`NgramTable::lookup`、分配和解码。`profile-run.json.gz` 保存干扰监测。

实际模型调用来自 `Runtime::order_presentation` 的呈现重排；九键字典已经禁用逐切分神经重排。`score_probe.rs.gz` 以确切上下文、有序全文候选列表模拟缓存，每个重排器独立持有，换上下文清空。843 次调用、7587 个候选，容量 1/16/32/64/128 分别命中 0/9/9/9/56 次。当前数据不支持实现整批模型评分缓存。

`row_probe.rs.gz` 与 `column_probe.rs.gz` 只观察单项备忘存在时的不同前词行。两轮均统计 6128340 行、202733504 次单项评分；行内没有重复出边词。逐行 LRU 的 64/256/1024 槽可省 4546958/94483882/155759315 次评分，按列分组的 16/64/256 槽可省 109779183/177330893/177972335 次。内存是每 4096 行按内容容量与部分容器字段抽样估算的最大值，不含哈希桶、LRU 节点和分配器元数据，不能当实际峰值或容量验收；模拟器未观察对象析构，跨对象地址重用的边界也不能精确预测产品命中率。具体值见 `column-probe-counters.json.gz`。探针使用完整词串，未靠纯指纹判断相等，不打印输入文本。

三个临时构建脚本均在 `finally` 中恢复源码，独立程序 SHA-256 另存。探针有大量分配与散列开销，其延迟不能作为正常成绩。所有生产源码和接线已恢复。

## 列缓存候选

`column-candidate.patch.gz` 保存完整生产差异、冻结入口与测试，作为可审计快照，不接入编译。候选保存最多 64 列、256 KiB 已入槽容量，构造当前列最多另持有 256 KiB；固定 LRU 节点和分配器元数据另受条目数约束。完整有序词列表只散列一次，命中逐词完整核对；列内以确切前词取原始 `f32` 行。表 `Arc` 持有和切换失效防止身份地址重用。个人/搭配/三元评分及假设顺序沿用旧入口。

旧实现红测重复图仍有 608 次单项备忘查找；候选为 0。5 项专项测试通过，覆盖碰撞、内嵌零与空词、特殊浮点、表身份、双上限和改变图/模型后的完整输出等价。`column-frozen-comparison.json.gz` 核对冻结入口与固定基线去掉注释和空白后完全一致。没有声称通过完整引擎、golden、runtime 或平台设备测试。

## 完整配对

普通系统分配器的独立 release 测试程序，在同一进程调用当前基线的完整冻结解码和候选入口。固定 6 种图形 × 3 种评分模式（无表、表无备忘、表有备忘）× 2 种附加图模式 × 冷/热，共 72 格。每格 21 对，交替先后；两臂先校准到至少 5 ms。原始对值全部保留，不筛选通过格。这里使用合成词网格与小型 n 元表，不替代锁定真实资源上的运行时评测。

有备忘的热多列为目标，目标格比值几何均值要求 `< .90`，每格新/旧比中位数要求 `< 1.05`。冷模式每次创建新备忘并承担构造、分配和析构；热模式同一备忘预热三次后计时。旧臂与候选臂各有独立备忘；对照不会借用候选缓存。测试程序仍含原有 `cfg(test)` 的 `KeyHasher` TLS 调用计数，两臂经过单项备忘时均承担它；候选会跳过更多单项访问。不能把这个测试程序等同于无计数器的产品二进制，更不能换算为端到端收益；若重访并考虑采用，需要补上不含该计数器的发布入口与实际运行时对照。

第一轮 `column-v1-paired` 的 72 格全部输出，退出 101，但 33 次监测中发现编译重叠，不能用作正式回退结论。完整记录保留。随后使用同一程序和门槛，增加连续 30 秒无编译且一分钟负载不超过 10 的启动条件再跑整轮；900 秒没有满足启动条件，未运行计时；旧脚本未保存该轮 preflight，`idle-wait-result.json.gz` 明确记录这一缺口。随后只要求连续 30 秒无编译的补充等待也在 180 秒后超时，其 preflight 由修正脚本保存。没有有效复测轮次。

## 结论

本轮不增加生产缓存：整批模型评分重复率低，列缓存功能正确但没有取得满足性能门槛的有效证据。候选已全部撤回，没有为它继续跑质量、端到端 ABBA、完整引擎/golden/runtime 或设备测试。恢复后 quick、格式、笔记和差异检查通过；不能据此声称产品提速或九键 16 ms 达标。

## 复现

在固定基线的独立 worktree 中操作，产物仍放 `target/`。资源目录由仓库评测校验器验证；不更改锁文件或数据集。普通分配器构建脚本临时停用计数分配器，并在 `finally` 中逐字恢复；不要与其他构建同时运行。

```sh
mkdir -p target/reranker-profile-evidence
gzip -dc docs/measurements/2026-10-23-lattice-column-score-cache/column-candidate.patch.gz > target/reranker-profile-evidence/candidate.patch
git apply target/reranker-profile-evidence/candidate.patch
python3 docs/measurements/2026-10-23-lattice-column-score-cache/build-column-system-benchmark.py
python3 docs/measurements/2026-10-23-lattice-column-score-cache/run_column_timed.py --paired --label reproduce --quiet-seconds 30 --idle-timeout 900 --max-load 10
```

`manifest.json` 对原始字节、脱敏字节和交付 gzip 分别记录 SHA-256。`<checkout>`、`<user-home>` 等是本机路径占位符；gzip 使用固定 `mtime=0`。日志内部的原始哈希需按 manifest 的 `raw_sha256` 关联，不直接与脱敏后文件比较。没有提交程序、真实输入、账号资料或凭据。
