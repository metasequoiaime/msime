# 全拼冷页复制复用 FIFO 淘汰页

## 固定输入与诊断

产品基线为 `6fa568ce12fa7d6c129fce7c4428eb4e8a1717d1`，模型固定 `chinese-ime-lm` 的 `15caab4fdeb59c11f452b91884371cf30e2e5608`。12 个资源文件逐项核对大小和 SHA-256，锁及文件摘要见 `inputs.json.gz`。集合为公开合成的 `sentences-nine-key-v1.tsv`，81 例，每臂 1791 键，预热 5 例，原预算 16 ms。

基线 release 程序在修改前构建并保存，后续不覆盖。`rerank_latency` 的 SHA-256 为 `6baa2e62c750a609d51df87c329b253470eefa623e7e5736ab958b573b68027e`；`convert_eval` 为 `f5c53d60f744bff051aae1f27c8b8b79e84262290130093ace334a1dee52ab32`。

`sample` 只观察自己启动的子进程，最多 60 秒，包含初始化和预热。主线程 49023 个样本中，逐键调用内为 47607，外部 1416，求和一致。按最近已知帧互斥归属，分配/释放 10031、模型线性层 9996、词典 9415、BigramMemo 5630、解码 4179；其余见 `profile-analysis.json.gz`。分配样本向上归属全拼 7237、九键自身 2032、解码 494、模型 182、运行时 75、英文 11。`detail_profile.py` 以同一个互斥分配桶重现该归属，并单列反解后的调用链；含嵌套泛型的业务符号按脚本固定优先级分类，不能当作独立耗时测量。

250 次进程观察中，203 次出现外部工具，load1 为 222.42–251.02。两臂混合且采样覆盖不完整、不平衡，不能由样本数推断正常延迟、净收益或完整预算。诊断运行虽然完成了原预算检查，但均值 32.86 ms、p95 134.71 ms、750/1791 键超限只属于受扰动运行。

## 确定性验证

`verify_frozen.py` 将当前测试内的两个完整旧函数还原私有名称、去掉注释、统一 rustfmt 后，与固定提交机械比较。冻结查询核心不会调用修改后的投影核心。首个真实 SQLite 红测退出 101：满缓存一行仍有 22 次申请，未满足少 4 次的断言；未满缓存 23 次相同。

实现后的 6 项专项通过：

| 行数 | 满缓存旧/新申请 | 精确减少 | 未满旧/新申请 |
| --- | --- | --- | --- |
| 1 | 22 / 18 | 4 | 23 / 23 |
| 4 | 40 / 27 | 13 | 41 / 41 |
| 32 | 208 / 111 | 97 | 209 / 209 |
| 128 | 784 / 399 | 385 | 785 / 785 |

专项核对全部字段、外层及字段容量、嵌套边界、冷热、模糊、标记、逐行访问、输出独立性、FIFO 和数据库失效。四个合成谓词异常由 `catch_unwind` 接住，日志中的预期 panic 不代表测试失败；每次均确认原缓存及其年龄保留。相同容量的嵌套复制为零申请，且实际指针复用。局部确定性分配减少不等于净耗时收益。

## 质量对照

候选版 `rerank_latency` SHA-256 为 `efc7f9e22363643c16322403a68cf1c4e774f814ce56e13b68f506009d348d61`，`convert_eval` 为 `4fd9235b5612c0a1e996e926745b5899e06d0fc9635d40ca6b40ec44fb4929a1`。构建来源及三个改动生产文件的摘要见 `candidate-build.json.gz`，构建成功后再次逐项校验 12 个资源文件。

全拼、九键和 14 键各 81 例，完整质量 JSON 与全部候选 JSONL 均与固定基线逐字节相同。没有修改黄金基线。`convert_eval` 关闭英文混输，只作为中文控制。每布局的报告与候选摘要见两个 `*-quality.json.gz`；每次调用均使用新的空用户目录，没有私人学习数据。

## 回归与静态检查

完整引擎 2505 项通过、56 项沿用忽略；golden 31 项、运行时 364 项及文档测试通过，零失败。新增 FIFO 专项确认原位替换保留年龄、满容插入复用节点且零节点申请。严格 engine/runtime all-targets Clippy 的 `-D warnings` 检查通过。

首次完整 `CARGO_BUILD_JOBS=2 MSIME_CHECK_JOBS=1 bash scripts/verify-local.sh --quick` 退出 1，唯一失败是语音目录检查中的 Cargo 命令超过原有 120 秒限制。预先构建同一个生成器后，未修改的 `scripts/test-voice-language-catalog.py` 在 4.425 秒内退出 0，原有超时和断言均保留。首次门禁的 Linux 原生 111 项测试、Linux 桌面外壳、macOS 原生宿主和 Apple 桥接层编译均通过。Windows 可移植测试 164 项通过，2 项宿主差异排除、76 项需 Windows 构建；当前无 MinGW，Windows 原生及 pipe-only 交叉编译跳过。HarmonyOS 缺少暂存依赖，ArkTS 编译跳过。没有设备或原生界面验收。

完整门禁以同一命令重跑，741.824 秒后退出 0，语音目录检查在原有超时内通过。重跑时宿主库已构建，Windows 可移植测试增至 176 项通过、2 项宿主差异排除、64 项需 Windows 构建；其余跳过原因不变。`quick.log.gz` 保留首次失败，`quick-recheck.log.gz` 和状态文件保留完整通过结果，没有修改门禁或已知失败基线。

## 计时协议

完整 ABBA 只尝试一次启动门禁：连续 30 秒未观察到编译器、相关评测或 Docker CLI，且 load1≤10；最多等 180 秒。没有取得条件就记录 `timed=false`，不自动重试。脚本独占创建 `abba.json` 并持续保存启动观察，拒绝覆盖已有记录。若启动，按 A1/B1/B2/A2 串行执行，使用相同固定集合和原 16 ms 预算，运行前后校验程序哈希，保留所有退出码与进程名/PID 观察。观察不包括进程参数，也不能证明没有其它系统负载。

实际唯一一次启动观察持续 180.462 秒，共 161 次，load1 为 120.354–162.353，每次均观察到外部工具，没有满足启动条件。记录 `timed=false`、0 个计时臂，驱动退出 1 表示启动条件未满足；没有重试，也没有可报告的 ABBA 耗时收益。退出后再次核对两份程序哈希，均未改变。默认九键 16 ms 目标仍未验证。

## 复现

在仓库外的独立 worktree 使用规定 `target/`，构建前确认磁盘空间。先用相同资源锁准备公开资源目录。以下基线脚本会暂时恢复本片的两个生产文件，完成后逐字恢复候选源码；运行时不要在同一 worktree 并行构建。它拒绝主工作区、其它产品改动或覆盖已有固定程序。本次实际基线是在改动前直接构建，脚本仅供复现。

```sh
export RESOURCE_DIR=/path/to/verified/resources
python3 docs/measurements/2026-10-23-quanpin-evicted-page-reuse/build_baseline.py
CARGO_BUILD_JOBS=2 cargo build --release --locked -p msime-input-runtime --example convert_eval --example rerank_latency
cp target/release/examples/convert_eval target/post-sqlite-evidence/candidate-convert_eval
cp target/release/examples/rerank_latency target/post-sqlite-evidence/candidate-rerank_latency
python3 docs/measurements/2026-10-23-quanpin-evicted-page-reuse/verify_frozen.py
CARGO_BUILD_JOBS=2 cargo test --locked -p msime-engine --lib evicted_page_tests -- --nocapture
CARGO_BUILD_JOBS=2 cargo test --locked -p msime-engine -p msime-input-runtime
python3 docs/measurements/2026-10-23-quanpin-evicted-page-reuse/quality.py --variant baseline --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-quanpin-evicted-page-reuse/quality.py --variant candidate --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-quanpin-evicted-page-reuse/run_abba.py --resources "$RESOURCE_DIR"
```

诊断采样由 `profile.py` 运行固定基线程序，再执行 `analyze_profile.py` 和 `detail_profile.py`。采样与预算计时须分开，采样脚本不使用空闲门禁，因为其结果只用于诊断。
