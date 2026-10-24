# 全拼路径缓存短键实验：不采用

## 固定基线与资源

基线为 `e7fa91f121e0b071bddfaa96917f87869e37018e`，模型固定 `15caab4fdeb59c11f452b91884371cf30e2e5608`。修改前构建并保存 release 程序，`rerank_latency` SHA-256 为 `03df54b0f0b33c124d75c8efa0929fb643cf35564f02bc8cfc829ceb51105903`，`convert_eval` 为 `d6ab1a289d6b1b1bf72ee08c6dd2fd0ecef98e9db09442df9f754ca420176ac7`。12 个公开资源逐项验证大小及 SHA-256，见 `inputs.json.gz`。质量控制运行全拼、九键、14 键各 81 例，三次均退出 0；本片未形成采用版，也没有把这三次称为新旧质量等价证明。`convert_eval` 关闭英文混输。

## 实际路径计数

在独立诊断版本的 `query_single_path_miss` 中插入只输出数字的计数，保存克隆页及原 FIFO 顺序不变。诊断构建完成后立即逐字恢复源码，再运行公开九键集合：81 例、5 例预热、两臂各 1791 个正式按键。计数包含两臂和预热，不按正式按键数直接归一。两次完整运行的前四项计数一致，第二次补齐键长分布；日志包含受干扰的预算失败，不能作为正常延迟成绩。

- 未命中 183332 次，满容淘汰 183076 次；非空页 17444 次、空页 165888 次，共 1162724 行。
- 页复制共需 3505616 次逻辑申请，按现有精确容量复用规则只有 62932 次可复用，占 1.7952%，1072 次查询受益。未实现路径页回收扩展。
- 每次未命中仍拥有化完整键。31 字节以内 162834 次，占 88.82%；24 字节以内 129434 次，占 70.60%。这说明短键有确定性的申请节省空间，尚不证明 CPU 净收益。

## 红绿验证及未采用的实现

`inline_key_tests.rs` 冻结基线完整未命中函数，只改私有名字。实际红测退出 101：零行、未满缓存、1 字节键的旧入口申请 2 次，无法满足少一次的预算。首版实现后两个专项通过，0/1/4/128 行、满/未满、空键、Unicode 及 31/32 字节边界逐项比较所有字段及容量。短非空键少一次申请，空键和长键次数不变，FIFO 与独立结果控制通过。

原型完整引擎测试为 2517 通过、5 失败、56 忽略；5 个失败均为既有精确分配断言观察到少 1 或 2 次申请，没有修改它们的断言来迎合原型。失败日志原样保留。初次接入测试还遇到四处旧 fixture 用 `FifoCache::new(2)` 赋值的类型错误，仅在原型中改构造类型；最后连同原型一并恢复。没有声称原型通过完整测试或运行时套件。

## 三组局部 CPU 对照

使用普通分配器与固定 Rust 工具链，机械引用生产 `FifoCache` 及保存的键原型。每格 21 个交替顺序配对，每臂 20000 次操作，使用 `CLOCK_THREAD_CPUTIME_ID`。七种键长度 × 三种页行数 × 三种操作，共 63 格。模式 0 为容量已满的轮换插入，1 为热读取，2 为同键覆盖。空键只有覆盖；1 字节用一槽、两个键交替；其它键用 128 槽、256 个合成键。页行是仅含 `u64` 的合成占位类型，覆盖缓存键和节点成本，不代表真实 `WordItem` 字段复制或完整按键。初始化在计量外，最终缓存析构在计量内。

表中的比值为候选/旧实现的线程 CPU 中位配对比，低于 1 才是局部减少。

| 实现 | 空页轮换插入：1 / 8 / 23 / 31 字节 | 32/64 字节轮换插入，全部页行数 |
| --- | --- | --- |
| 安全枚举、`Borrow<str>` | 1.2905 / 1.3440 / 1.3765 / 1.7156 | 1.1562–1.2176 |
| 安全枚举、`Borrow<[u8]>` | 1.1578 / 1.0000 / 1.0230 / 1.0215 | 1.1307–1.2353 |
| 独立 `compact_str 0.10.0` 探针 | 1.0418 / 0.9624 / 0.9792 / 1.1524 | 1.1274–1.2776 |

首版 31 字节热读取回退到 3.21–3.23；改用完整字节借用后消除了这组显著回退，但没有取得短键淘汰插入的稳定收益，长键仍回退。自定义枚举为 40 字节，旧 `String` 为 24 字节，128 槽最多多 2048 字节的键内固定存储，另减去省掉的短键堆。成熟库探针也只有同键覆盖明显受益，相关淘汰插入基本持平或回退；项目没有新增该依赖。

共享机器负载会影响调度、缓存及频率，线程 CPU 不能消除全部干扰。这些局部结果只支持本次不采用，不能推断所有紧凑字符串方案在所有平台都慢。没有反复调常数或挑选个别覆盖形状作为采用依据。

## 恢复后的最新基线诊断

生产文件与固定提交逐字一致，摘要见 `restored.json`；没有更改产品、依赖、测试断言、资源或 16 ms 门槛。随后只采样自己启动的固定基线子进程，最长 60 秒，包含初始化、预热及两臂混合。

44405 个主线程样本中 42527 个在 `type_case` 内，1878 个在外，独占桶求和一致：词典 8640、模型线性层 7963、分配释放 7739、BigramMemo 5459、解码 4202。分配桶中全拼 5640、九键 1447、解码 380、模型 143、运行时 119、英文 9、未分类 1。203 次 PID/进程名观察全部存在外部工具，load1 为 95.681–116.555。采样不能与上一片的不同负载样本数量直接相减。

受扰动运行的模型臂 p95 为 76.21 ms，667/1791 键超原 16 ms 预算，退出 1；该结果没有证明空闲时的预算成绩。三种候选都在局部对照被否决，没有构建采用版完整输入计时，也没有启动 ABBA 或改变预算。默认九键 16 ms 目标仍未验证。

## 最终文档门禁

生产及 fixture 改动全部恢复后，`CARGO_BUILD_JOBS=2 MSIME_CHECK_JOBS=1 bash scripts/verify-local.sh --quick` 在 1431.522 秒后退出 0。契约、1748 篇活跃笔记与 4 篇归档封印、Rust 工作区检查及共享 Apple 桥接层编译通过。最终范围仅笔记和测量文档，各原生平台、Android、wasm、Linux 外壳及 Windows pipe-only 编译按范围跳过；macOS 上的 `msime-desktop` 因原生资源 bundle 尚未构建而排除。没有设备验收或新的产品性能修复。原门槛、超时和已知失败文件均未修改。

Python 语法、Rust 格式、冻结函数及证据摘要验证通过。门禁日志与退出状态分别为 `quick.log.gz`、`quick-status.json.gz`。

## 复现

在固定基线的独立 worktree，使用规定 `target/`。先构建并固定基线程序，再运行诊断。`diagnose.py` 会临时插入数字计数，构建成功或失败均恢复原文件；它拒绝其它源码改动或非固定提交。实验源码位于本目录，不参与产品构建。

```sh
CARGO_BUILD_JOBS=2 cargo build --release --locked -p msime-input-runtime --example convert_eval --example rerank_latency
mkdir -p target/next-hotspot-evidence
cp target/release/examples/convert_eval target/next-hotspot-evidence/baseline-convert_eval
cp target/release/examples/rerank_latency target/next-hotspot-evidence/baseline-rerank_latency
export RESOURCE_DIR=/path/to/verified/resources
python3 docs/measurements/2026-10-23-quanpin-inline-path-key/diagnose.py
python3 docs/measurements/2026-10-23-quanpin-inline-path-key/bench.py --variant utf8
python3 docs/measurements/2026-10-23-quanpin-inline-path-key/bench.py --variant bytes
CARGO_BUILD_JOBS=2 python3 docs/measurements/2026-10-23-quanpin-inline-path-key/probe_compact.py
python3 docs/measurements/2026-10-23-quanpin-inline-path-key/profile.py --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-quanpin-inline-path-key/detail_profile.py
```

复现用 `compact-bench.rs` 仅补充 `rustfmt::skip` 并格式化，避免格式化器解析尚待替换的外部模块路径；原始模板保存在 `compact-bench-original.rs.gz`。

`compact_str` 探针锁文件固定 crate 及校验和，MIT 许可随证据保存。其源码来自 crates.io `compact_str 0.10.0`，未修改；仅探针链接，不随产品发布。所有输出只有公开资源、合成值和进程名/PID，路径做过脱敏，压缩文件使用固定时间戳；`manifest.json` 列出逐文件摘要。
