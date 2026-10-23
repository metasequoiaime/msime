# 整句解码缓存输出投影测量

基线为 `8f777281cf26125ce26ac7e1d631d9b42447c78c`。候选实现命中缓存后借用整句路径，在截断、模型选择和去重之后才复制实际输出；冷解码仍复制一份入缓存，再把原拥有路径交给输出。路径预算、束宽、质量规则和 16 ms 门槛均不变。

资源来源提交为 `128a332cabc5fa42b1cf5f827033ad5697ff528b`，校验代次为 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。机器和二进制哈希见 `environment.json.gz`。文件名里的 `candidate-32` 只是复用上次 runner 的候选臂标签，本次没有改为 32 条路径，也没有采用上次已撤回的数字词图原型。

## 数据与完整性

`manifest.json` 同时记录原始字节、去除本机路径后的字节以及交付 gzip 文件的 SHA-256。日志中的 `<checkout>`、`<resource-checkout>`、`<other-checkout>` 和 `<user-home>` 是脱敏占位符；压缩使用固定 `mtime=0`。记录文件内的日志哈希对应原始字节，应通过 manifest 的 `raw_sha256` 关联，不能直接与脱敏后的日志比较。没有提交可执行文件、私人输入、账号资料或凭据。

- `red.log.gz` / `green.log.gz`：同一真实 SQLite 缓存测试，旧实现 25 次申请，候选实现 2 次；目标值始终为 2。
- `frozen-comparison.json.gz`：冻结合并入口与输出助手（共八个函数）与固定基线去掉单行注释和空白后逐字一致。
- 两臂九键、同集全拼的 JSON 和完整候选 JSONL：均逐字节相同。每组 81 条；九键 top1 `.617`、found `.728`，全拼 top1 `.790`、found `.840`。质量运行的耗时不作性能结果。
- `summary.json.gz` / `timed-runs.json.gz`：保留全部计时轮次、失败格、负载和编译监测。每轮均为完整矩阵，不拼接或筛选格。
- `production-v1.patch.gz`、两版符号表：保存第一版生产差异与包装层修正的依据。
- 引擎、runtime、Clippy、quick 及 Linux 门禁日志：保留实际验证范围和因缺少前置而跳过的阶段。`clippy.log.gz` 中旧测试的冗余闭包已用等价函数项修复，最终结果在 `clippy-final.log.gz`。

## 配对方法

普通系统分配器下，冻结旧入口与新入口在同一个 release 测试程序内交替先后运行。矩阵固定为 4 种路径数（0/1/6/12）×3 种已有候选（空、全部重复、全部重复加 65 条占位）×2 种 `emit`（0/1）×2 种模型模式（无模型、真实合成 safetensors 键盘模型）×2 种入口（拥有、借用），共 96 格。每格 21 对，最快臂校准至少 5 ms。

借用且路径数大于 1，并且全部重复或无模型 `emit=1` 的目标格，新/旧比的中位数必须小于 `.98`；其他格（包括全部拥有入口）必须小于 `1.05`。门槛没有随结果调整。

第一版无编译重叠，但 6 个拥有控制格失败；第二版恢复拥有入口的空路径提前返回，并把共享主体强制内联。其后若干整轮通过但出现编译重叠，只作参考。`idle-v2-3` 没有编译重叠，仍有一个空路径控制格以 `1.140691` 失败；原始配对显示新旧臂均出现间歇性的调度跳动。后续轮次增加启动条件：连续 30 秒无编译进程且一分钟负载不超过 10。全部历史轮次继续保留。

`low-load-v2-1` 与 `low-load-v2-2` 两轮各 96 格全部通过，合计 70 次监测无编译重叠。目标格比值分别为 `.154673–.915850`、`.153663–.923812`，控制格最高为 `1.030961`、`1.032203`。

## 九键端到端结果

正式低负载 ABBA 四臂均无监测到的编译重叠。模型开启结果如下，单位为 ms：

| 臂 | mean | p95 | 退出码 |
| --- | ---: | ---: | ---: |
| 基线 A1 | 7.69 | 20.00 | 1 |
| 候选 B1 | 7.92 | 20.40 | 1 |
| 候选 B2 | 7.75 | 20.10 | 1 |
| 基线 A2 | 7.65 | 19.87 | 1 |

四臂均未达到 16 ms，失败退出码完整保留。候选两臂 p95 的均值比基线两臂高约 1.6%，本轮没有端到端提速证据。采用依据限于真实缓存命中的申请减少、两个完整矩阵的目标格收益和控制格低于 5% 的回退上限；不把局部复制收益外推为整体按键延迟改善或总体目标完成。之前受编译重叠影响的整组 ABBA 也保留，不混入正式四臂。

## 复现

在仓库根目录运行，`RESOURCES` 指向已通过仓库评测校验的上述资源代次。不要与其他构建或计时并行运行普通分配器构建脚本；它临时停用测试计数分配器，复制程序后在 `finally` 中逐字恢复原文件。输出始终使用规定的 `target/`。

```sh
EVIDENCE=docs/measurements/2026-10-23-sentence-decode-cache-projection
mkdir -p target/cache-projection-evidence
python3 "$EVIDENCE/build-system-benchmark.py"
python3 "$EVIDENCE/run_timed.py" --paired --label reproduce-paired --quiet-seconds 30 --idle-timeout 900 --max-load 10
cargo build --locked --release -p msime-engine --example ninekey_latency -p msime-input-runtime --example convert_eval --example rerank_latency
```

端到端 ABBA 需要分别把固定基线和候选的 release `rerank_latency` 复制到 `target/cache-projection-evidence/baseline-rerank_latency`、`candidate-32-rerank_latency`，再运行：

```sh
python3 "$EVIDENCE/run_timed.py" --abba --label reproduce-abba --resources "$RESOURCES" --quiet-seconds 30 --idle-timeout 900 --max-load 10
python3 "$EVIDENCE/summarize.py"
```

每臂使用完整 `resources/eval/sentences-nine-key-v1.tsv`，81 条、每模式 1791 次按键，5 条预热。预算超限会保存非零退出码，runner 继续记录其余臂。ABBA 的四臂必须作为一整组解读，不能用其中一对代替整组。
