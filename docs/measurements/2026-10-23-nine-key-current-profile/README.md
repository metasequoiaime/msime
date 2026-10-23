# 九键当前基线采样与二元评分行绑定试验

## 结论与范围

默认九键 16 ms 目标仍未达成。这里保存最新基线的诊断栈及一次未接入的候选：同一评分行只核对一次二元表身份。候选通过确定性行为对照，但等待 180 秒仍未满足计时启动条件，`timed=false`，没有新旧耗时样本，也没有性能预算失败或提速结论。生产源码已逐字恢复；本目录不改变产品行为。

决策及历史方案分类见[否决笔记](../../../.agents/notes/rejected/testing/2026-10-23-lattice-bigram-row-binding.md)。否决表示本轮不接入，没有证明算法更慢。后续应沿精确调用栈寻找可验证的成本，不能把分配桶都视为不必要分配。

## 固定输入

| 输入 | 固定值 |
| --- | --- |
| 产品基线 | `1330287680040ab5a0ddbc36029c3f09dfc47138` |
| 资源来源 | `128a332cabc5fa42b1cf5f827033ad5697ff528b` |
| 资源代次 | `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe` |
| 资源锁 SHA-256 | `e3059c466dffbced51bc8ecfe38793decc0c98f5f5df60c98ff2438fa093dfe9` |
| 公开合成集合 | `resources/eval/sentences-nine-key-v1.tsv`，81 例、每臂 1791 键、预热 5 例 |
| 集合 SHA-256 | `b21fd929d6bfac057897620e384742018019a14c21aebd716f04f60bb819282f` |
| release 例程 SHA-256 | `80c9bf9b57fdb80a0e54de9cea632cbf03b876619cc5dcb46155fb3dcef9beb5` |
| 编译器与平台 | Rust 1.97.1，`aarch64-apple-darwin`，macOS 27.0.1 |

`inputs.json.gz` 保留锁内全部 12 个资源的大小、SHA-256 及工具链；每项均实际核对。程序和模型不随证据分发。

## 基线诊断

`profile.py` 只采样自己启动的 `rerank_latency` PID；`ps` 观察仅记录相关工具名和 PID，不收集其它进程参数。采样为启动后的前 20 秒，混合 engine/model 两臂，含初始化及预热，不覆盖约 32.36 秒进程中的全部 81 例。

这一轮 engine mean/p95 为 6.58/14.48 ms，model 为 10.26/26.42 ms，423/1791 键超过 16 ms，退出码 1 来自原预算。107 次观察中 38 次见编译，load1 为 16.36–18.33。这些墙钟受采样和外部编译影响，只能作诊断，不能作正式无采样基线或优化收益。

`analyze_profile.py` 逐节点检查子样本不大于父样本，按调用链最深的已知入口归类独占样本。主线程共 14461 个样本，其中 972 在 `type_case` 外，逐键路径 13489 个，归类如下，总和守恒。完整原始栈保留在 `profile.sample.txt.gz`，未删未分类栈。

| 入口 | 样本 |
| --- | ---: |
| 词库 | 3499 |
| 分配/释放 | 2881 |
| 模型矩阵 | 2259 |
| 二元备忘 | 1296 |
| 图解码 | 1191 |
| 九键 | 800 |
| n 元查表 | 728 |
| 复制/比较 | 587 |
| 模型其余 | 200 |
| runtime | 40 |
| 未分类 | 8 |

## 候选与已执行验证

`candidate.patch.gz` 是完整候选及测试补丁。`BigramMemo::fill_row` 先按旧顺序输出无效键的正零，遇到第一个有效键才核对表地址并执行原失效，余项复用原 entry 逻辑。全空行及空表不改变备忘身份，满容量命中不清空，未命中才清空。FNV、容量、评分、搜索预算均不变，没有新增缓存字段。

- 有效红测：旧入口在扇出 2 时核对 3 次表身份，期望 2 次，正确失败。首次 probe 生命周期编译错误独立存为 `invalid-lifetime-error.log.gz`，不计入红测。
- debug 四项专项测试通过；新入口扇出 1/2/8/32/128 时均核对 2 次。
- 冻结完整解码 640 配置各跑冷/热两轮，共 1280 次对照，全部路径字段、分数位模式、回调次序、备忘表身份、键值、长度及容量相同。另含空词、空表、表切换、负零/NaN/无穷值、满容量命中、缺失清空及超过容量长行。
- 普通 System 分配器 release 三项对照通过。构建临时停用全局计数分配器和两处 KeyHasher TLS，随后逐字恢复。debug 计数测试不编入 release。
- `frozen-comparison.json.gz` 记录完整冻结解码及旧单项评分方法与固定基线的机械核对。
- `patch-replay.json.gz` 记录精确补丁应用、三个文件 SHA-256 核对、逆向恢复及脚本 AST 检查。`build_probe.py` 最后新增了要求先应用补丁的 guard，guard 条件已实测；最终脚本版本未再次完整编译，成功构建发生在新增 guard 之前。

没有为撤回候选跑完整 engine/runtime、质量 JSON、无采样 ABBA、Clippy 或设备验收。三项 release 对照不代表全套产品测试。

## 配对门槛及未计时终态

预先固定 4/12 列 × 1/4/16 行 × off/cold/hot 共 18 形状，每形状 21 对、交替先后、每臂校准至少 10 ms。每格中位新旧比须 <1.05，宽行且有表的目标几何均值须 <0.98。

启动需连续 30 秒无编译/相关 benchmark 且 load1≤10，最多等待 180 秒。实际等待 180.5038 秒、172 次观察，其中 66 次见编译，load1 为 12.4570–34.4263。`run_probe.py` 以未启动计时终止，`analyze_pairs.py` 输出 `performance_budget_result: null`。没有修改门槛、重试找绿、配对样本或几何均值。

## 证据完整性

`manifest.json` 为每个压缩文件记录原始、脱敏和 gzip 的 SHA-256 及字节数。只将本机工作树绝对路径替换为 `<WORKTREE>`，保留栈、PID、时间和计数；gzip 固定 `mtime=0`。不提交可执行文件、模型、完整 Cargo JSON 元数据或私人输入。`candidate.patch` 未发生脱敏修改，其原始 SHA 与 `candidate-source.json.gz` 一致。

## 复现

在包含本目录文档、且产品源码与固定基线相同的独立工作树执行。若后续产品源码已改变，先建立固定基线工作树，再从本次文档提交读取本目录；不要把补丁强行套到不同基线上。只使用工作树的 `target/`，预留至少 50 GiB 可用空间。以下操作不会复现原计数红测，红测日志是接入前的实测记录。

先解压证据并重算解析结果，无需构建：

```sh
python3 - <<'PY'
from pathlib import Path
import gzip, hashlib, json
source = Path('docs/measurements/2026-10-23-nine-key-current-profile')
target = Path('target/current-nine-key-evidence')
target.mkdir(parents=True, exist_ok=True)
manifest = json.loads((source / 'manifest.json').read_text())
for name, record in manifest['files'].items():
    compressed = (source / name).read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == record['gzip_sha256']
    data = gzip.decompress(compressed)
    assert hashlib.sha256(data).hexdigest() == record['sanitized_sha256']
    (target / name.removesuffix('.gz')).write_bytes(data)
PY
python3 docs/measurements/2026-10-23-nine-key-current-profile/analyze_profile.py
python3 docs/measurements/2026-10-23-nine-key-current-profile/analyze_pairs.py
```

需要重建候选时，确认 `git diff -- crates/engine/src` 为空，新增测试文件也不存在；在 bash 子 shell 中用 trap 撤回候选。`RESOURCE_DIR` 指向与上述 12 项锁一致的完整资源代次。重新运行会覆盖 `target/` 下本地证据，提交的压缩证据保持原状。

```sh
export RESOURCE_DIR=/path/to/verified/resources
bash <<'SH'
set -euo pipefail
patch=target/current-nine-key-evidence/candidate.patch
git apply --check "$patch"
git apply "$patch"
trap 'git apply -R "$patch"' EXIT
cargo test --locked -p msime-engine --lib row_binding -- --test-threads=1
cargo test --locked -p msime-engine --lib complete_decode_and_memo_state_match_frozen_entry -- --test-threads=1
python3 docs/measurements/2026-10-23-nine-key-current-profile/build_probe.py
target/current-nine-key-evidence/paired-system row_binding --skip row_binding_complete_decode_pairs --test-threads=1
target/current-nine-key-evidence/paired-system complete_decode_and_memo_state_match_frozen_entry --test-threads=1
python3 docs/measurements/2026-10-23-nine-key-current-profile/run_probe.py --resources "$RESOURCE_DIR"
python3 docs/measurements/2026-10-23-nine-key-current-profile/analyze_pairs.py
SH
git diff --exit-code -- crates/engine/src
```

`run_probe.py` 等待超时会非零退出，trap 仍会恢复源码；此时可单独运行 `analyze_pairs.py`。`build_probe.py` 用 `finally` 恢复计数器接线，不能在其运行中强杀进程。

若重做基线采样，应在已撤回候选且源码等于固定基线时执行 `cargo build --release --locked -p msime-input-runtime --example rerank_latency`，然后运行 `profile.py --resources "$RESOURCE_DIR"` 和 `analyze_profile.py`。`sample` 只适用于 macOS。采样墙钟仍只能诊断。

## 文档交付验证

候选撤回后，`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均退出 0。quick 的 Rust workspace 编译及共享 Apple 桥接编译通过，其余平台阶段按文档范围跳过；这不构成设备验收。日志见 `final-notes.log.gz` 和 `final-quick.log.gz`。五个脚本 AST 检查、全部证据的哈希/解压/路径脱敏检查，以及两个解析器对解压证据的重放均通过。正常 pre-push hook 结果记录在 PR。
