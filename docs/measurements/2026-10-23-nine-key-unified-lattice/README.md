# 九键统一数字图试验证据

当前原型撤回，生产代码保持原有实现。[决策记录](../../../.agents/notes/rejected/testing/2026-10-23-nine-key-unified-lattice-trial.md)包含收益、失败和未验证边界；`result.json` 汇总质量及原始可执行文件、资源哈希，`SHA256SUMS` 校验交付文件。

## 文件

- `baseline-*`：固定基线；`candidate-*`：12 条终点原型；`candidate-32-*`：32 条终点原型。
- `prototype.patch`：最终生产和测试改动；`prototype-12.patch`：首版，含测试对私有 fixture 的错误引用，只用于重建首版 release 评测，不用于声称测试通过。
- `*-runs.json`：每次命令、退出码、日志哈希、耗时及编译进程/负载观测；补充的 `log_file` 按哈希定位交付日志。原始绝对路径只记录实验位置，复现可用任意符合仓库要求的外部 worktree。
- `*.jsonl.gz`：评测工具输出的全部合成候选，固定 gzip mtime=0；没有真实输入。补丁的空白上下文行省去前导空格，`candidate-tests.log` 去掉末尾多余空行；源码内容与测试输出文字保持。`*.json` 是完整质量报告，`*.log` 是编译、测试或评测输出。
- `sentence-differences.json`：81 条句子的新旧名次与首选，逐条展示 4 条改对、1 条改错，以及 found 的 7 条新增和 7 条丢失。
- `baseline-nine-words-interrupted-full.log`：遗漏采样参数的中断轮；对应记录退出 -15，不能用作质量结果。后续词级轮显式使用 `--limit 3000`，实际采样 2791 条。

## 复现

从 `657503e4c3d1b2bff7c0a93108a2db945a30ce9c` 创建仓库外的隔离 worktree。准备 `result.json` 所列代次的资源，使用已有标准 `target/`，保留磁盘余量；不在共享主 checkout 应用补丁。以下命令在该 worktree 执行，`EVIDENCE` 指向本证据目录的绝对路径，`RESOURCES` 指向锁定资源目录。

```sh
cargo build --locked --release -j 4 -p msime-input-runtime --example convert_eval --example rerank_latency
mkdir -p target/unified-lattice-evidence
cp target/release/examples/convert_eval target/unified-lattice-evidence/baseline-convert_eval
cp target/release/examples/rerank_latency target/unified-lattice-evidence/baseline-rerank_latency
python3 "$EVIDENCE/run_measurements.py" --resources "$RESOURCES" --variant baseline --quality --latency --long-latency --words
git apply --check "$EVIDENCE/prototype.patch"
git apply "$EVIDENCE/prototype.patch"
cargo test --locked -p msime-engine digit_lattice --lib
cargo test --locked -p msime-engine --lib nine_key:: --no-fail-fast
cargo build --locked --release -j 4 -p msime-input-runtime --example convert_eval --example rerank_latency
cp target/release/examples/convert_eval target/unified-lattice-evidence/candidate-32-convert_eval
cp target/release/examples/rerank_latency target/unified-lattice-evidence/candidate-32-rerank_latency
python3 "$EVIDENCE/run_measurements.py" --resources "$RESOURCES" --variant candidate-32 --quality --latency --long-latency --words
git apply -R "$EVIDENCE/prototype.patch"
```

九键完整测试预期有 6 项失败；不要用 `set -e` 让预期失败阻止后续证据收集。延迟程序超过 16 ms 时返回 1，runner 会保存该结果并继续；0/1 之外的退出码会终止。runner 不把退出 1 当作门禁通过。补丁反向应用后仍要检查工作区，以上命令不替代最终 quick 门禁。

原始实验将质量、完整延迟、12 字延迟、词级分别调用；没有 ABBA。复现中的单次合并调用仅方便重跑同一组命令，不能将不同机器、负载或顺序的数值直接当成配对结果。原基线源码构建后恢复原型再构建，最终版本测试夹具修复晚于 release 构建，生产源码相同；`prototype.patch` 是包含该修复的最终可编译版本。撤回后没有继续跑其他句子集、完整 engine/golden、Clippy 或正式交替延迟验收。

## 最终交付检查

撤回原型后，`git diff 657503e4c3 -- crates/engine` 为空。`CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，包括 Rust workspace 检查与共享 Apple 桥构建；原生平台阶段按文档改动范围跳过，桌面 Tauri 因未准备原生 bundle 由门禁正常跳过。`pnpm run verify-notes`、`git diff --check`、最终补丁 `git apply --check`、全部交付 SHA-256、运行记录与日志哈希关联、JSONL 解压和脚本语法检查均通过。没有将原型的 6 项失败计入已通过测试。
