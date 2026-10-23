# 分组英文查询复用 SQLite 程序

## 根因与边界

固定产品基线 `e60da08cb07196e3008a0c83fdb8e4f6961dcc0f`。旧[九键诊断](../2026-10-23-nine-key-current-profile/README.md)中，逐键英文调用链的 1683 个独占样本有 1025 个位于 SQLite prepare/reprepare 之下。该采样混合两臂、含预热和外部构建，不能代表当前耗时；本轮机械核对旧采样基线与当前基线的完整分组英文 SQL 相同，见 `diagnostic-root.json.gz`。

实际 bundled SQLite 的生产入口红测执行 8 次、重新编译 8 次。分组模式始终以字符组 `[` 开头，不能提供字面前缀索引范围，却仍因直接绑定 `word GLOB ?3` 在每次执行时重新规划。SQLite [优化说明](https://www.sqlite.org/optoverview.html#the_like_optimization)记录了这个参数失效机制，[REPREPARE 计数](https://www.sqlite.org/c3ref/c_stmtstatus_counter.html)用于直接验证，没有用耗时阈值替代根因计数。

候选把已绑定的模式转为 TEXT 表达式，继续使用显式 `word >= ?1 AND word < ?2` 的范围和完整 GLOB。模式保持绑定，过滤、排序、限额、行解码、候选容量及错误恢复不变。不修改词库格式、不加缓存、不改搜索预算、质量基线、模型或平台宿主。CAST 增加少量 SQL 字节和虚拟机指令，必须确认它换来编译消除且没有改变计划。

## 行为与编译验证

生产入口专项 3 项通过：相同序列新查询 Run=8、RePrepare=0；冻结旧查询 Run=8、RePrepare=8。旧/新 VM step 为 716/724，不以指令计数充当 CPU 收益。12 组 EXPLAIN QUERY PLAN 相同，显式索引范围保持；另一连接更改 schema 后仍重新编译一次并看到新行。有效红测和绿测分别见 `red.log.gz`、`green.log.gz`，格式化后重跑见 `reprepare.log.gz`。

页解码专项 4 项通过，完整英文入口 448 次合成冷热对照及发布词库 168 次冷热对照一致，另有非法数字输入控制，保留字段、顺序、外层容量与模式缓冲。发布专项显式运行忽略项，没有把普通套件的忽略当作通过。完整测试通过：引擎 2490、golden 31、运行时 362，0 失败；引擎 55 项沿用忽略。见 `append.log.gz`、`published-english.log.gz`、`full-tests.log.gz`。

旧页查询的 SQL 已独立冻结，不能跟随生产常量变化而失去对照。原有完整英文入口对照和发布词库对照继续比较全部字段、顺序和外层容量；schema 变化另测，不能屏蔽必要的自动重新编译。固定发布库只有 `sqlite_stat1`，没有 `sqlite_stat4`；不宣称任意带 STAT4 统计的自定义库都不再进行参数相关规划。

## 固定输入及程序

本机为 `aarch64-apple-darwin`，Rust 1.97.1；完整工具链身份见 `inputs.json.gz`。

资源来源 `128a332cabc5fa42b1cf5f827033ad5697ff528b`，代次 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。全部 12 项大小与 SHA-256 本轮已实际核对，见 `inputs.json.gz`。英文库 SHA-256 为 `d216f66cab7edb9072ba7a07f3fe35623e76870438b76d360a89cc2a4795d3cd`。

`build_baseline.py` 拒绝无关产品输入差异，临时恢复固定基线源码构建并复制 release 例程，再用 `finally` 逐字恢复候选。构建时不得并发改写生产源码或强杀 Python。后续候选单独重新构建，二进制哈希分别记录，不分发程序或模型。

## 质量与延迟

三种布局的完整 JSON 质量报告和全部候选 JSONL 均与基线逐字节相同，见 `quality-comparison.json.gz` 及两侧质量记录。单次启动等待在 180.94 秒后以退出码 1 结束，`timed=false`、0 个计时臂。154 次观察中 load1 最小 13.50、最大 1023.96，且持续观察到 Cargo、rustc、Clang、Docker、Swift 和其它评测活动，没有取得连续 30 秒启动条件。未执行计时，不重试，不推断净耗时收益或默认九键 16 ms 达标。见 `abba.json.gz`、`abba-summary.json.gz`、`abba-run-status.json.gz`。

质量控制使用同一份 81 条公开合成集合，在全拼、九键、14 键分别比较完整 JSON 和全部候选 JSONL。`convert_eval` 关闭英文混输，因此它只验证中文控制；英文由独立冻结入口和发布词库对照覆盖。所有评测使用空用户目录，没有私人学习数据。

ABBA 要求启动前连续 30 秒无编译/相关 benchmark 且 load1≤10，最多等待 180 秒，未满足则记录 `timed=false`，不自动重试找绿。若启动，完整 A1/B1/B2/A2 串行，81 例、每模式 1791 键、预热 5 例，原 16 ms 门槛不变；保留全部退出码、负载和可见相关工具名/PID（含 Clang、GNU 交叉编译器、Swift、链接器和 Docker CLI），不收集进程参数。监测只证明未观察到列表中的工具，不等于系统无其它负载。观察到外部编译时整组不能用于推断收益，不筛选单臂。程序运行前后均核对 SHA-256。

## 门禁与证据

严格 engine/runtime all-targets Clippy、格式检查、笔记校验、`git diff --check` 通过。首轮完整 quick 退出 1，只有 iOS 生命周期 Swift 构建和语音目录 Cargo 命令超过原 120 秒限制；两项未出现产品断言失败。相同产品源码、原超时和断言下，两项独立复核均通过；随后 `CARGO_BUILD_JOBS=4 MSIME_CHECK_JOBS=1 bash scripts/verify-local.sh --quick` 完整复核退出 0，保留首轮失败和完整复核。

完整复核中 Windows 本机测试 176 项通过（2 项宿主差异排除、64 项需 Windows 构建）；Rust workspace、Android 和 wasm 目标、wasm 严格 Clippy、Android Java 宿主、Linux 外壳、Linux 原生 111 项测试、macOS 原生宿主与 Apple bridge 构建通过。Windows 交叉构建/pipe-only、HarmonyOS HAP、未准备 bundle 的 macOS Tauri workspace 阶段按原脚本跳过。没有设备或 GUI 验收。见 `quick-initial.log.gz`、两项 `*-recheck` 记录、`quick-final.log.gz` 和 `final-gates-status.json.gz`。

证据使用固定 `mtime=0` 的 gzip；`manifest.json` 记录原始、脱敏和压缩后的字节数及 SHA-256。只替换本机路径，保留计数、时间和结果，不提交程序、模型、完整 Cargo JSON 元数据、凭据或私人输入。以上编译与测试不构成设备或 GUI 验收。

## 与前缀缓存的合入复核

PR #9400 创建后，`develop` 的 `49f6494fed`（#9385）新增了缓存填充入口 `keypad_prefix_rows`。在 `5f452db36da91ce49afa7f6e2f66b1a286976c59` 上解决冲突时，保留前缀缓存，并把同一 TEXT 表达式应用到 `KEYPAD_PREFIX_ROWS_SQL`；原有限额查询也继续保留修复。

新增入口先以独立冻结旧 SQL 对照确认有效红测：生产入口 Run=8、RePrepare=8，旧对照也是 8；该失败见 `integration-red-valid.log.gz`。最初测试类型转换的编译失败也保留在 `integration-red.log.gz`。schema 变更用例同时覆盖两条查询，完整字段与顺序对照覆盖新增入口。

专项 4 项绿测通过：原有限额查询与缓存填充查询各执行 8 次，重新编译均为 0；独立旧对照均为 8。两条查询均在 schema 变更后重新编译一次并看到新行，见 `integration-green.log.gz`。

集成后的完整回归通过：引擎 2494 项、golden 31 项、运行时 364 项，零失败；引擎 56 项沿用忽略。固定发布英文词库的 168 次冷热对照和页追加专项 4 项再次通过。严格 engine/runtime all-targets Clippy 也通过。见 `integration-full-tests.log.gz`、`integration-published.log.gz`、`integration-clippy.log.gz`。

集成后的完整 quick 退出 0，包含 Windows 本机 176 项、Linux 原生套件 111 项零失败、macOS 原生宿主与 Apple bridge 构建；各项环境跳过保留在 `integration-quick.log.gz`，Linux 详细结果见 `integration-linux-native.log.gz`。没有设备或 GUI 验收。

上文程序哈希、质量与 ABBA 记录只对应原固定基线及候选 `e34ab4b31051733def428e296176a3b00a8e1de2`，不代表此次合入新前缀缓存后的程序。没有重建或覆盖原评测程序，也没有重试 ABBA。原基线复现应使用上述候选提交中的脚本；集成源码与验证范围另列于本节。

## 复现

只使用独立工作树的规定 `target/`，开启新工作树与构建树前保留至少 50 GiB 空间。指定与上述资源锁完全一致的目录：

```sh
export RESOURCE_DIR=/path/to/verified/resources
mkdir -p target/keypad-glob-evidence
CARGO_BUILD_JOBS=4 cargo test --locked -p msime-engine --lib keypad_reprepare_tests -- --nocapture
CARGO_BUILD_JOBS=4 cargo test --locked -p msime-engine --lib keypad_append_tests -- --nocapture
MSIME_TEST_ENGLISH_DICTIONARY="$RESOURCE_DIR/msime-english.db" CARGO_BUILD_JOBS=4 cargo test --locked -p msime-engine --lib english_page_append -- --include-ignored --nocapture
python3 docs/measurements/2026-10-23-keypad-glob-statement-reuse/build_baseline.py
CARGO_BUILD_JOBS=4 cargo build --release --locked -p msime-input-runtime --example convert_eval --example rerank_latency
cp target/release/examples/convert_eval target/keypad-glob-evidence/candidate-convert_eval
cp target/release/examples/rerank_latency target/keypad-glob-evidence/candidate-rerank_latency
python3 docs/measurements/2026-10-23-keypad-glob-statement-reuse/quality.py --variant baseline --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-keypad-glob-statement-reuse/quality.py --variant candidate --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-keypad-glob-statement-reuse/run_abba.py --resources "$RESOURCE_DIR"
```
