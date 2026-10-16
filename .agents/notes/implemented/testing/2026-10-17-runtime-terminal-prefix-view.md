# Agent Note: 运行时末位置续跑借用两段 KV

Status: implemented

## Problem

九键完整按键 p95 最近为 17.83 ms，16 ms 预算仍未通过。上游末位置续跑消除了原 KV 与缓存单行先拼成临时前缀、再写最终 KV 的中间复制，需要固定合并来源接入并核对主项目质量与逐键延迟。

## Decision

接入上游已合并 PR #67 的固定提交 `6acc7839572fb7df221a8e4910467c9dedd72fb8`。两个 crate 清单、主锁和 fuzz 锁统一来源；私有前缀分别借用原 KV 和末位置缓存单行，按可见宽度直接写最终存储。空前向、普通截断与零层保留；末缓存仍仅持有每层一行。公开 API、浮点计算、候选覆盖和缓存寿命保持。

## Existing decisions

已检索末位置、KV 借用、terminal prefix。[普通续跑借用](../../implemented/testing/2026-10-16-runtime-rerank-kv-borrow.md)仍持有可见长度截断，本片补齐其留下的末位置中间拼接；[最终 KV 构造](../../implemented/testing/2026-10-16-runtime-rerank-packed-kv.md)仍持有最终精确容量；[末位置按需前向](../../implemented/bug-fix/2026-10-15-runtime-defer-terminal-forward.md)仍要求每层仅缓存单行。本次部分补充这些决定，没有完全吸收或过时提案。

## Upstream verification

上游 [PR #67](https://github.com/metasequoiaime/chinese-ime-lm/pull/67) 合并至实际默认分支 `main`；[固定提交完整笔记](https://github.com/metasequoiaime/chinese-ime-lm/blob/6acc7839572fb7df221a8e4910467c9dedd72fb8/.agents/notes/implemented/testing/2026-10-17-terminal-prefix-view.md)保留全部方法与整表。地址回归在旧路径真实失败；324 合成几何与多轮续跑的 KV、隐藏状态与累计分逐位一致。完整续跑 64 形状、每形状 21 对交替计时，旧生产控制 3 个形状未过冻结门槛，新实现全部通过；固定真实模型 16 形状也全部通过。三个配对门禁均没有构建观察；真实六层热末状态、单后继四形状 new/old 中位 0.209–0.361，只说明局部微秒收益。

固定旧源码公开接口对照中，全拼与九键 141 例、1692 轮、15228 个缓存分数逐位相同，冷分数、强制可比首选和完整分布保持。Rust 1.97.1 与最低 1.75.0 各通过 40 单测、15 模型文件测试和 doc tests，7 个手动项忽略；Clippy、fmt、笔记门禁与独立审查通过。三轮整模型 benchmark 全表均保留；两轮旧版有外部构建干扰，而且未交替配对，不用这些串行差值主张稳定整体收益。

## Alternatives considered

- 在主项目复制模型实现：就地调试容易，但会分叉数值与缓存契约，继续由上游持有模型计算。
- 固定上游分支：接入更早，但来源可移动，固定已审查正式合并提交。
- 缓存完整末位置前缀：借用连续存储简单，但候选会长期重复持有原 KV，保留上游单行内存边界。

## Verification

基线为主项目 `10f4594f76417b35fc31e8d976541d9892322412`。接入前构建同基线旧 pin 的 release 质量评测程序，接入后按相同默认口径对照完整全拼 60 例和九键 81 例 JSON，不加可选 `--keyboard-neural`，不改质量基线。固定模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`，vocab 8192、6 层、4 头、宽 192、context 64。

默认并行完整 engine/runtime 测试、全目标 Clippy、工作区 fmt、笔记门禁、本地 quick 和独立审查是交付检查。九键完整 1791 键延迟在本任务其它编译结束后运行，并每秒记录外部构建观察；预算维持 16 ms，失败如实记录，局部微秒收益不能折算端到端。

## Consequences

首次末位置补算仍保留完整 KV 构造后拆出单行的原复制；此片未删除模型主要矩阵成本。两轮上游旧版整模型基准有外部构建干扰且非配对，只依据无构建观察的局部交替配对支持局部收益。主项目四处 pin 统一；fuzz 锁另外补齐基线已有的 `serde_ignored` 与 `png` 依赖边和缺失的 `serde_ignored 0.1.14` 包项，没有升级其它包。


## Results

主项目基线 `10f4594f76417b35fc31e8d976541d9892322412`，接入合并提交 `6acc7839572fb7df221a8e4910467c9dedd72fb8`。四处 pin 已逐项核对。固定模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`，配置为 vocab 8192、6 层、4 头、宽 192、context 64。

`cargo check --locked -p msime-engine -p msime-input-runtime` 退出 0。`env -u MSIME_EVAL_RESOURCES MSIME_NEURAL_MODEL_DIR=<固定模型目录> CARGO_BUILD_JOBS=4 cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 通过：engine 2235、golden 31、runtime 317，engine 忽略 37；Clippy `-D warnings`、全仓 fmt、`pnpm run verify-notes` 和 `git diff --check` 通过。

默认 `convert_eval` 未加 `--keyboard-neural`，全拼 60 与九键 81 例的新旧 JSON 均完全相等。全拼 top1/MRR 为 0.833/0.842，九键 top1/MRR 为 0.617/0.640；没有改变质量分布。

接入后九键 release 延迟完整集合每臂 1791 键、丢弃 5 个预热 case：engine mean/p50/p95 为 3.70/3.58/7.22 ms，model mean/p50/p95 为 6.85/5.26/18.28 ms，reranker 增量 mean/p50/p95 为 3.15/0.34/12.30 ms，137/1791 键超过 16 ms，程序退出 1。采样 21 次观察到 12 个构建进程（最多 2 个），因此这轮延迟不作为系统空闲下的稳定比较；它确认当前预算仍未通过。原始日志和监视 JSON 保存在工作区外 `runtime-terminal-prefix-view-nine-latency.log` 与 `.monitor.json`。

独立只读审查核对四处 pin、质量 JSON、延迟门禁与本笔记，无阻断或重要问题。


## Verification environment recovery

quick 首轮因系统盘耗尽，临时文件申请与 macOS 构建失败。第二次申请失败后触发了已有清理缺陷，删掉 worktree 的 `.git` 等根部隐藏普通文件；已将本任务五个改动文件从保留副本迁回同一分支的重新登记 worktree。缺陷、合成失败注入与修复见[门禁临时文件保护](../process/2026-10-17-verify-local-temp-failure.md)。后续一次运行中编辑了正在执行的脚本，Bash 在原读取偏移遇到语法错误，已中止该轮；最终必须在脚本稳定后完整重跑，不能将这些轮次算作通过。没有修改其他会话的工作目录。


## Fuzz lock consistency

完整 `cargo metadata --manifest-path fuzz/Cargo.toml --locked --offline --format-version 1` 首次退出 101。独立检出未改动的主项目基线 `10f4594f76417b35fc31e8d976541d9892322412`，同一命令同样退出 101，证明这项 fuzz 锁缺口在接入前已有。两上游固定提交的 `reference/Cargo.toml` 无差异。离线重新解析后只补已有 `client-core -> serde_ignored`、`client-services -> png` 边及与主锁相同的 `serde_ignored 0.1.14` 包项，未升级已有包；再次完整 `--locked --offline` 退出 0。诊断 worktree 干净、基线是 `origin/develop` 祖先，已立即移除。


## Quick gate

脚本稳定后，完整 `bash scripts/verify-local.sh --quick` 退出 0。为避开系统盘余量波动，临时环境把无参数的 `mktemp` 转到本 worktree 的 `target/tmp/`；没有跳过测试或更换构建目录，并发为 2。覆盖全部仓库契约（含本次临时文件红绿回归）、actionlint、1183 篇笔记与 4 个归档封印、Rust workspace、Android 目标、WASM、Android Java、macOS 原生宿主与共享 Apple bridge；本机 Windows 可移植测试 155 通过，2 项宿主差异排除、66 项需 Windows。macOS bundle 未预先构建，Rust workspace 阶段排除 `msime-desktop`，后续原生 macOS 宿主实际编译通过。最终轮 Harmony 设置页缺依赖而跳过；Linux Docker、完整 Windows 交叉、HarmonyOS 真工程与 pipe-only 缺前置跳过。没有将编译与进程内评测称为设备按键验收。


## Nine-key latency recheck

所有本任务构建结束后复测同一完整集合，仍为 81 例、每臂 1791 键、5 个预热 case；19 次每秒采样未观察到构建进程，但不排除其它系统活动或采样间短进程。结果如下，单位 ms：

| 测量臂 | mean | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: | ---: |
| engine only | 3.63 | 3.51 | 7.09 | 9.05 | 12.48 |
| with model | 6.60 | 5.07 | 17.43 | 22.65 | 31.62 |
| reranker adds | 2.97 | 0.27 | 11.94 | 17.16 | 24.80 |

124/1791 键（6.9%）超过 16 ms，程序因 p95 预算失败退出 1。首次有构建观察的 18.28 ms 结果继续保留；本次没有同轮交替旧 pin 对照，不能将两个新 pin 轮次间或历史日志间的差值当成稳定收益。原始记录为工作区外 `runtime-terminal-prefix-view-nine-latency-recheck.log` 与 `.monitor.json`。16 ms 目标继续 active。
