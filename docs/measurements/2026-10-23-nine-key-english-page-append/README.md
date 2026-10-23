# 九键英文前缀页直接追加

## 改动与边界

固定基线为 `3a706bd021661aadcfcdf0cc477e28282c7928d8`。最新[九键诊断](../2026-10-23-nine-key-current-profile/README.md)仍有英文行解码分配；源码确认每个前缀页先建独立 `Vec<WordItem>`，随后逐行搬入总结果并销毁页容器。本片消除这个没有后续消费者的容器，最终候选字段仍独立拥有。

分组英文查询直接追加到总结果，SQL、每前缀五条、最多 64 前缀、最终五条、展开顺序、同文行取首条及稳定排名不变。空结果不预留，首次有效行仍按原理论上限预留。普通前缀继续覆写复用已有行。失败只丢弃本页；首次部分写入后失败恢复零容量，后续页失败保留前页。错误路径可能短暂预留 320 行总容量，旧路径此时只预留单页；这一代价不保留到下一次刷新。

[决策笔记](../../../.agents/notes/implemented/testing/2026-10-23-nine-key-english-page-append.md)说明替代方案和旧笔记审计。没有新增缓存、依赖、unsafe、文件系统行为或宿主改动。默认九键 16 ms 目标仍未证明达到，不把申请次数换算成 CPU 或端到端提速。

## 确定性验证

有效红测在旧生产入口上得到两页命中共 56 次申请，要求减少两次而失败；候选为 54 次，全部字段与最终容量相同。首次夹具引用私有模块的编译错误独立保留，不计为有效红测。

| 合成组码 | 非空成功前缀页 | 旧申请次数 | 新申请次数 |
| --- | ---: | ---: | ---: |
| `2` | 3 | 51 | 48 |
| `22` | 9 | 112 | 103 |
| `222` | 27 | 220 | 193 |
| `2222` | 0 | 137 | 137 |
| `9` | 0 | 22 | 22 |

三处完整冻结旧方法（九键英文入口、分组查询、行解码）独立保留于测试，不调用新的追加 helper；与固定基线机械核对一致，仅忽略注释、空白和私有方法重命名。448 次九键/14 键、专用/混输/无中文回退、锁定及冷热对照的内容、顺序、外层容量及模式字符串均相同。

已校验发布英文词库另跑 168 次完整对照，遍历八个公开合成英文输入的各前缀、两种键盘与冷热两轮。该用例默认忽略，只有设置 `MSIME_TEST_ENGLISH_DICTIONARY` 才执行；本轮已实际执行通过。

词库四项专项覆盖 NULL、非法 UTF-8、整数/实数展示、平局、重复展示、零/负权重、限额、普通英文覆写、无连接、prepare 失败、首步失败、部分读取后失败和缺列错误。部分失败恢复零容量及前页不变，普通覆写保留外层指针与容量。

完整测试通过：引擎 2487 项、golden 31 项、运行时 362 项，0 失败；当时 54 个引擎忽略项。随后新增的发布英文词库忽略项已在最终四项英文入口专项中显式执行，四项词库专项也重跑通过。严格 engine/runtime all-targets Clippy、全仓 Rustfmt、笔记与差异检查均通过。完整测试不包含设备验收。

## 固定输入及质量控制

Rust 1.97.1，`aarch64-apple-darwin`。资源来源 `128a332cabc5fa42b1cf5f827033ad5697ff528b`，代次 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。资源锁内全部 12 项实际核对大小及 SHA-256，明细见 `inputs.json.gz`。英文库 SHA-256 为 `d216f66cab7edb9072ba7a07f3fe35623e76870438b76d360a89cc2a4795d3cd`。

`quality.py` 在全拼、九键、14 键上各执行固定 81 条公开合成集合 `resources/eval/sentences-nine-key-v1.tsv`。基线与候选六次运行均退出 0，三个完整 JSON 报告及全部候选 JSONL 逐字节相同。`convert_eval` 关闭英文混输，这组结果仅作中文质量控制；英文变化由上述完整冻结入口和发布词库对照覆盖。

`build_baseline.py` 先拒绝其它产品输入差异，再临时恢复两份基线源码，构建两例程并复制程序，最后用 `finally` 逐字恢复候选。候选随后重新 release 构建，没有复用基线程序冒称候选。二进制 SHA-256 在 `baseline-build.json.gz` 和 `candidate-build.json.gz`，不分发程序或模型。

## 延迟观察

单次 ABBA 完整结束。每臂均为 81 例、1791 键、预热 5 例，基线与候选程序前后 SHA-256 不变。启动满足连续 30 秒无相关工具、load1≤10；后两臂观察到外部编译，因此整组不能用于推断净耗时收益，也不筛选前两臂另作结论。

| 臂 | 版本 | engine mean/p95 ms | model mean/p95/p99/max ms | 超过 16 ms | 退出码 |
| --- | --- | --- | --- | ---: | ---: |
| A1 | 基线 | 4.73 / 9.60 | 7.78 / 20.35 / 25.73 / 34.63 | 220/1791 | 1 |
| B1 | 候选 | 4.84 / 9.85 | 7.99 / 20.25 / 25.91 / 83.11 | 237/1791 | 1 |
| B2 | 候选 | 5.95 / 10.95 | 9.20 / 22.74 / 45.14 / 171.42 | 303/1791 | 1 |
| A2 | 基线 | 4.87 / 9.82 | 7.87 / 20.36 / 25.35 / 38.84 | 231/1791 | 1 |

四臂原 16 ms 门槛全部失败。B2、A2 分别在 24、9 次观察中见到外部编译；全组 load1 为 4.6450–9.4697。候选的均值、长尾及最大值均保留，不能据此归因于代码或宣称无回退；没有重复测量找绿。本片采用依据是成功页容器确定性消除及字段、顺序、容量和错误恢复对照，默认九键预算仍未达到。

## 门禁范围

首轮 `bash scripts/verify-local.sh --quick` 退出 0：Windows 本机兼容 164 项通过，Linux 桌面外壳编译及原生 111 项测试通过，Rust workspace、Android、两种 WASM、Android 宿主 Java、macOS 原生宿主及共享 Apple 桥接通过。Windows 交叉、Harmony 真打包与 pipe-only 因前置缺失跳过。macOS 配置和编译不等于打开宿主验收，Android Java 不等于模拟器验收。

新增发布词库对照后，最终四项英文入口专项、四项词库专项、严格 Clippy、全仓 Rustfmt 和第二轮完整 quick 均退出 0。正常 pre-push 还会运行 quick，其终态在 PR 正文记录。

## 证据完整性

`manifest.json` 对每份压缩证据列出原始、脱敏及 gzip SHA-256 和字节数。脱敏只替换本机工作树、资源目录与用户目录路径，保留测试结果、计数、时间和 PID；gzip 固定 `mtime=0`。不包含程序、模型、私人输入或完整 Cargo JSON 元数据。公开合成集合和候选转储不含用户学习数据，评测使用临时空用户目录。

## 复现

使用独立工作树，产品输入须为上述基线加本片改动；其它产品改动会被基线构建脚本拒绝。不要在基线构建时并发构建、测试或强杀 Python；脚本暂时改写两份源码，正常失败会在 `finally` 恢复。只使用规定的 `target/`，预留至少 50 GiB 空间再启动构建。

```sh
export RESOURCE_DIR=/path/to/verified/resources
mkdir -p target/nine-key-english-append-evidence
CARGO_BUILD_JOBS=4 cargo test --locked -p msime-engine --lib dictionary::english::keypad_append_tests -- --nocapture
MSIME_TEST_ENGLISH_DICTIONARY="$RESOURCE_DIR/msime-english.db" CARGO_BUILD_JOBS=4 cargo test --locked -p msime-engine --lib english_page_append -- --include-ignored --nocapture
python3 docs/measurements/2026-10-23-nine-key-english-page-append/build_baseline.py
CARGO_BUILD_JOBS=4 cargo build --release --locked -p msime-input-runtime --example convert_eval --example rerank_latency
cp target/release/examples/convert_eval target/nine-key-english-append-evidence/candidate-convert_eval
cp target/release/examples/rerank_latency target/nine-key-english-append-evidence/candidate-rerank_latency
python3 docs/measurements/2026-10-23-nine-key-english-page-append/quality.py --variant baseline --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-nine-key-english-page-append/quality.py --variant candidate --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-nine-key-english-page-append/run_abba.py --resources "$RESOURCE_DIR"
```

ABBA runner 启动前最多等 180 秒，要求连续 30 秒无编译/相关 benchmark 且 load1≤10。未满足条件保存 `timed=false` 并退出，不自动重试。若启动，整组 A1/B1/B2/A2 串行，完整 81 例、每模式 1791 键、预热 5 例，保留原 16 ms 门槛和所有退出码。外部编译观察到重叠时不能从该组推断收益；不能筛通过臂。
