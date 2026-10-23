# Agent Note: n 元表桶内严格下界二分

Status: rejected — 严格循环在固定 release 预算中全面慢于 `partition_point`

## Problem

`NgramTable::lookup` 已把键切成单个高位桶，并用 `partition_point` 在桶内寻找首个不小于查询键。标准库实现保持固定的区间收缩步数和无数据相关的选择；候选方案改用经典 `lo < hi` 严格下界循环，命中或邻近查询可能少一次比较，桶尾未命中也不会探测桶外。当前只有比较次数模拟，没有真实代码生成和运行时收益证据。

## Proposal

只替换 `crates/engine/src/lattice/ngram.rs` 的桶内下界循环：保持现有 `index`、映射切片、键的字节格式、重复键取首项、桶尾判定、评分位模式和零分配契约。以本桶 `&[[u8; 8]]` 为边界，严格维护 `[low, high)`，命中后仍走原绝对行号读取评分；空桶和未命中返回正零。

先在 `ngram_lookup_tests.rs` 增加独立参考函数与完整键、相邻键、空桶、重复键、极值和特殊浮点位模式的逐位对照。旧实现必须先在候选预算测试上真实失败，再实现候选；若严格循环没有在固定配对预算中胜过现状，则撤回全部生产改动，只归档否决证据。

## Execution status

生产 `ngram.rs` 已恢复到固定基线 `653becfd13baff94bdeed8f9ef22c9a9a6c2a64f`，移除未经过红测的候选和直接调用私有 helper 的测试。既有 `bucket_lookup_release_budget` 对照的是更早的字节探针实现，继续保留用于原来的决定；本片新增独立的 `strict_bisect_release_budget`，以基线当前的完整 `partition_point` 查找为旧臂。冻结正文只将 `self` 改为 `this`，去空白后与基线完全相同。

先在恢复后的生产实现上完成了真实 release 红测，再接入历史严格循环候选。两次运行均使用同一固定资源、同一进程入口、同一 21 对交替顺序和 `--test-threads=1`；候选只替换桶内下界循环，语义对照全部通过，但性能门槛失败。不得把此前的 Python 语义模拟当作 Rust 测试通过；它不能预测 ARM64 上 `partition_point` 的代码生成。


## Verification

旧版红测在恢复后的 `partition_point` 生产实现上按预期失败，证明新/旧中位比门槛没有因为同一实现而误通过。候选 release 运行于 Rust 1.97.1 Apple arm64，资源目录为锁定的 `7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe`。逐行命中、相邻键、65536 个随机与桶尾查询的逐位对照通过；零分配契约也保持不变。

候选八种形状的中位比（候选/旧）如下：二元表 `hit 1.249460`、`random 1.303631`、`bucket-tail 1.513768`、`mixed 1.307300`；三元表 `hit 1.171490`、`random 1.237585`、`bucket-tail 1.470413`、`mixed 1.240337`。每项均高于要求的 `< 0.90`，所以测试退出码为 101。计时前后没有观察到 cargo、rustc 或其它构建干扰；正式样本均达到至少 20ms。候选生产代码已撤回，当前 `ngram.rs` 与 `origin/develop` 基线一致。

本片没有宣称九键端到端 p95 16ms 达标，也没有为被否候选继续跑完整质量、Clippy 或 quick；保留的测试与本笔记只记录否决证据，避免后续重走同一条路。

## Historical audit

已阅读[桶内键切片](../../implemented/testing/2026-10-15-ngram-bucket-key-slice.md)及[桶旁存在位图](../../rejected/testing/2026-10-19-ngram-bucket-presence.md)。继续沿用桶索引、只读映射和 `partition_point` 的语义边界；不重试线性扫描、只保存桶尾或存在位图。此前的 Python 模拟覆盖了 77573 个 bisect fixture、完整表及相邻查询，显示严格循环比较次数约为现状的 0.66–0.80，但这不是性能证明。

## Alternatives considered

- 保留 `partition_point`：标准库实现稳定且当前已有局部收益；若严格循环引入分支预测或边界检查成本，现状可能更快，因此把它作为不可回退的生产基线。
- 分支无关的固定步长二分：可能减少循环控制，但需要额外初始化和比较；没有现成 ARM64 代码生成证据，本片不同时引入第二种算法。
- 桶内线性扫描或存在位图：已有实测回退或命中负载失败，见关联笔记，不再重复。

## Acceptance criteria

1. 旧实现与候选实现对固定合成表和两张完整资源表的所有命中、相邻、随机、桶尾查询逐位一致；重复键取首项，查询零分配。
2. 候选 release 计时使用同一进程、同一输入、21 对交替先后；两张表的 `hit`、`random`、`bucket-tail`、`mixed` 每个形状新旧中位比 `< 0.90`，且没有观察到构建或其它 benchmark 干扰。每臂预先校准至至少 40ms，正式样本均至少 20ms；所有八个形状的 168 对耗时与预算判定完整输出后才退出。任一形状失败即撤回生产改动，不重测找绿。冻结完整旧入口，避免只测下界 helper 或拿较早的逐字节二分当作基线。
3. 固定 81 条九键入口的完整质量 JSON 与基线逐字一致；无采样 ABBA 和 16ms p95 预算按既有门槛记录，不能用局部查表收益替代端到端结论。
4. 通过相关引擎测试、fmt、clippy、notes 校验和仓库 quick 门禁后，单独提交、推送并开一个 PR；若被否，只提交否决笔记和复现实验，不保留候选代码。

## Risks

严格循环的数据相关分支可能在随机查询上比 `partition_point` 的 `csel` 更慢；安全索引表达式也可能重新引入 bounds check。低位桶索引和映射切片的任何偏差都会改变重复键或桶尾语义。局部查表预算即使通过，也不保证九键 16ms 目标达成。

当前系统盘不足仓库规定的 50GB，不启动新的本机构建树。旧 Android 设备任务的完成不能解除这个门槛；它不属于本片，也不能提供本片的 Rust 测试或性能证据。需要可用的远端编译入口或系统盘空间恢复后，按 `MSIME_NEURAL_MODEL_DIR=<固定资源> cargo test --release --locked -p msime-engine --lib strict_bisect_release_budget -- --ignored --nocapture --test-threads=1` 先运行旧版红测，并在计时期间另存进程监测。
