# 全拼路径缓存工作集诊断

## 范围和固定输入

本片只交付诊断工具与结果，产品容量仍是 128，缓存和依赖没有改动。固定基线 `11cdc33ba64dc95adb3c4621a185ddd26088a524`，模型依赖固定 `15caab4fdeb59c11f452b91884371cf30e2e5608`。12 个公开资源逐项核对大小和 SHA-256，输入仅为公开合成集合 `sentences-nine-key-v1.tsv` 的 81 例，预热 5 例。原始 release 程序的摘要与上一片一致：`rerank_latency` 为 `03df54b0f0b33c124d75c8efa0929fb643cf35564f02bc8cfc829ceb51105903`，`convert_eval` 为 `d6ab1a289d6b1b1bf72ee08c6dd2fd0ecef98e9db09442df9f754ca420176ac7`。

诊断包装直接转发生产 `FifoCache`，不重新实现其查找或淘汰。完整键只在诊断进程的编号表里保留，日志只输出数字编号、行数、请求容量和 SHA-256。摘要覆盖 `WordItem` 的全部字段及句子词边界。只有可选前缀探测另行标识，便于发现改变缓存后上层控制流可能变化。诊断程序构建结束后即逐字恢复两个产品文件，并删除临时模块。

## 原容量的实际观察

841236 个缓存事件逐条核对通过，包含 4 个实例的建立与析构；两个实例没有查询，另外两个各访问 46219 个不同键。全部 657892 次查询中命中 474560 次，未命中 183332 次，恰好与前片独立数字诊断一致。每次未命中都对应一次插入，没有外部插入或可选探测。未命中中空页 165888 次、非空页 17444 次，共重建 1162724 行。

92438 次为各自清空周期内首次访问，90894 次为重复未命中。正式两臂共 3582 次按键刷新，各臂 1791 键；单次刷新不同查询键数 p50/p95/max 为 72/148/233，其中 400 次超过 128 个不同键。单次刷新未命中 p50/p95/max 为 43/133/240，部分键在同一次刷新内被淘汰又重查。预热的 124 次刷新独立记录，不混入正式刷新分位数。

## 固定序列重放

下表包含预热及两臂所有缓存操作。只在实际查询未命中时模拟填入，从原轨迹取得该次查询的完整内容摘要和容量；原地覆盖不刷新年龄。原 128 槽的每次实际命中均已核对。改变容量后如命中内容与该次实际结果摘要不同，会单独计数并禁止据此声称等价。

| 配置 | 查询未命中 | 非空页未命中 | 缓存页复制申请 | 键与候选页峰值请求字节 |
| --- | ---: | ---: | ---: | ---: |
| 原 FIFO 128 | 183332 | 17444 | 3505616 | 1412959 |
| FIFO 256 | 128588 | 10178 | 1909250 | 1413701 |
| FIFO 512 | 114040 | 7542 | 1257774 | 1974554 |
| FIFO 1024 | 110358 | 6390 | 946044 | 2427396 |
| FIFO 2048 | 106912 | 5726 | 789824 | 3620610 |
| FIFO 4096 | 106210 | 5280 | 646704 | 5134026 |
| 非空/空页分流 64/64 | 226042 | 6278 | 942632 | 2260959 |
| 非空/空页分流 96/32 | 262936 | 6090 | 867264 | 2987092 |
| 非空/空页分流 64/128 | 165572 | 6278 | 942632 | 2263866 |
| 非空/空页分流 128/256 | 123128 | 5812 | 765046 | 3429401 |
| 128 槽且不存空页 | 421572 | 5812 | 765046 | 3420529 |

256 槽在该固定序列上减少 54744 次未命中（29.86%）和 1596366 次缓存页复制申请（45.54%）。这些数字只覆盖插入缓存的页副本，不包括结果页、上层系列、模型或整个按键的分配。删除空页虽然少复制非空页，却把总查询未命中增至 421572；不能只按复制次数选择方案。

峰值是同时存活实例的键堆容量与候选页堆容量之和，不是 RSS，也不是整进程峰值。候选页包含外层 `Vec` 请求、四个字符串及嵌套词边界的请求容量。没有计入节点、哈希表、分配器元数据和诊断自己的编号表。arm64 上 `lru 0.18.5` 的节点还含 24 字节键、24 字节值和两个指针；增加槽位仍有额外固定成本，742 字节的候选堆差不能称为扩容总内存代价。

所有配置在当前固定序列里没有内容摘要不一致，也没有前缀探测结果变化；这是该集合的覆盖事实。没有改容量后实际运行，也没有把表中数字称为延迟收益。

## 首字母展开边界

`expand_initial_candidates` 会把首字母展开页写入路径缓存。原 FIFO 淘汰后，再查询可能重新取得有限页；延长展开页的存活会影响输出。本次实际轨迹没有这类外部写入，不能验证该边界。手工序列特意构造了展开页覆盖：原 128 槽淘汰后取得普通页，256 槽保留旧展开页；重放正确报告内容不一致。

因此下一候选是验证 256 槽及其语义边界，当前不修改产品常数。需要真实检查首字母展开、重置、词典和个人资料变更、网格及元数据内存代价，再决定是否实现。可选前缀探测还可能改变后续查询序列，固定序列不是完整反事实执行。

## 验证与失败

原始与诊断 `convert_eval` 的九键 81 例完整质量 JSON 和全部候选 JSONL 逐字节相同，二进制测前测后摘要不变。该工具关闭英文混输，只是诊断包装的中文控制，不是任何扩容候选的质量证明。手工可核算序列验证年龄不变、清空、空页分流、内容变化、请求字节及错误轨迹拒绝。

首轮诊断构建因 `sha2 0.11` 的输出没有 `LowerHex` 失败，原样保留失败日志。构建脚本在失败后也恢复源码；改用仓库已有的 `hex::encode` 后重建通过，没有改变依赖。

诊断版本按原 16 ms 预算运行，模型臂 p95 38.60 ms，退出 1。摘要、编号和日志会扰动执行，且共享机器还有外部工作，诊断计时全部无效。没有运行采用版 ABBA；默认九键 16 ms 仍未验证。

最终仅提交笔记、文档和测量证据。产品与依赖逐字恢复；Python 语法、独立 Rust 快照格式、笔记及证据摘要通过。完整本地 quick 在 382.816 秒后退出 0，契约、1749 篇活跃笔记与 4 篇归档封印、Rust 工作区及共享 Apple 桥接层通过。原生平台、Android、wasm、Linux 外壳及 Windows pipe-only 阶段按文档范围跳过；`msime-desktop` 因原生 bundle 缺失而排除。原门槛、超时与已知失败基线保持不变。没有设备验收，也没有声称本轮完成产品性能修复。

## 复现

在固定基线的独立 worktree 使用规定 `target/`。先构建并保存原始程序，再执行临时诊断构建；`diagnose.py` 拒绝非固定提交、产品改动或残留的临时模块，成功和失败都恢复产品源码。`RESOURCE_DIR` 指向已验证资源目录，脚本会重新核对锁定的 12 项资源。

```sh
CARGO_BUILD_JOBS=2 cargo build --release --locked -p msime-input-runtime --example rerank_latency --example convert_eval
mkdir -p target/cache-trace-evidence
cp target/release/examples/rerank_latency target/cache-trace-evidence/baseline-rerank_latency
cp target/release/examples/convert_eval target/cache-trace-evidence/baseline-convert_eval
export RESOURCE_DIR=/path/to/verified/resources
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/diagnose.py
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/quality.py --resources "$RESOURCE_DIR" --arm baseline
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/quality.py --resources "$RESOURCE_DIR" --arm diagnostic
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/selfcheck.py
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/replay.py target/cache-trace-evidence/trace.log --out target/cache-trace-evidence/replay.json
python3 docs/measurements/2026-10-23-quanpin-path-cache-working-set/working_set.py target/cache-trace-evidence/trace.log --out target/cache-trace-evidence/working-set.json
```

也可直接对本目录的 `trace.log.gz` 重放；解压后的轨迹只含公开合成输入产生的数字与摘要。日志的本机路径已脱敏，压缩使用固定时间戳，`manifest.json` 保存逐文件 SHA-256 与大小。
