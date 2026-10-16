# Agent Note: 全拼系列热前缀直接追加候选

Status: implemented

## Problem

固定主项目 `01edc14f05` 的九键采样可见 `query_series` 下 `query_single_path` 的候选克隆与分配。代码核对发现较短前缀的热页先整体克隆成临时 `Vec<WordItem>`，再由 `append_query_rows` 预留最终容量并搬运；临时页容器没有长期消费者。最终候选字段副本仍有消费者，不能按整个克隆成本宣称节省。

## Decision

较短前缀采用私有追加入口：热命中且目标已有容量时，按原 `reserve(rows.len())` 后从借用页逐行克隆到总结果；目标尚无容量时仍整体克隆接管。冷页保留原缓存克隆、查询与结果接管。最长组仍先取得独立页，以保留长词补全排序、首位及首页规则；稀疏去重回退保持原方法。借用只在本次追加内，不改变 FIFO、失效、个人模型、公开接口或依赖。

## Existing decisions

检索 `query_single_path`、`query_series`、候选克隆、行缓存和前缀追加。[热原始行投影](../../implemented/testing/2026-10-14-quanpin-row-cache-projection.md)处理 DictRow 到 WordItem 的临时原始页，保留其冷缓存容量决定；本片处理上层已经构造好的 WordItem 页，不替代原决定。[系列前缀键借用](../../implemented/testing/2026-10-14-quanpin-series-prefix-borrow.md)处理键字符串，本片保留该键及截断规则。没有同一追加所有权决定，也没有需要吸收的旧提案。

已核对标准库 [Vec 的 reserve/extend](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.reserve) 和 [Cloned 迭代](https://doc.rust-lang.org/std/iter/struct.Cloned.html)文档，使用与仓库热行投影相同的 `iter().cloned()` 范式；不引入其它库。

## Alternatives considered

- 改为共享候选字符串或返回借用结果：能省去更多深复制，但跨越公开拥有型候选与学习、修改和失效边界，不在本片实施。
- 移除单路径缓存：减少长期副本，但让热页重新构造所有候选，且改变展开缓存与 FIFO 契约，不采用。
- 冷页直接搬进缓存再生成输出：可减少页容器，但可能转入过量 SQL 预留容量；保留原冷路径。

## Verification scope

以固定提交的完整系列及单路径方法对照，分配预算先失败再实施。热路径每个非空、已有目标容量的较短前缀恰好省一次申请，冷路径不增加。冷/热、多音节、空页、别名、Unicode、非规范键和首字母覆盖输出全部字段、顺序与容量；缓存容量、FIFO 及 reset/drop 后输出独立性保持。完整 engine/runtime 测试、Clippy、fmt、笔记、质量 JSON 与 quick 作为提交门禁。局部分配预算不代表九键延迟达到 16 ms。

## Risks

热页采用逐行克隆，需保持字段和嵌套容器容量与 Vec 深克隆一致。目标零容量时必须保留接管规则；冷页插入与查询不能增加重复查找。最长组继续独立排序，不能向已有结果直接追加后再混排。

## Verification

修复前，冻结旧系列与生产路径的双音节热查询同为 110 次，预算要求省一次而失败。修复后三个专项通过，双音节 110→109、三音节 149→147、别名规范键双音节 99→98；对应冷查询分别 323→323、448→448、304→304。单音节、缺词与空输入冷热均保持原次数。

专项比较完整字段、嵌套字符串与向量容量，涵盖词网格开关、冷/热、多前缀、首字母、空页、Unicode、嵌入分隔符与参数不匹配。带纠错、分词、固定位置与模糊标记的合成缓存行在目标零容量、预留但为空和已有候选时均保持；两个槽的 FIFO 读取不刷新年龄，reset/drop 后输出仍独立。

完整测试初次只有四个精确次数预算失败。历史前缀键对照同步后续较短前缀追加，保留旧拼接和截断，继续仅隔离旧键预算；本片冻结的完整旧系列与单路径不调用新追加 helper，独立检验本片节省。会话末尾输入实际 127→126、光标处输入 194→192；临时撤生产优化后旧预算通过，再收紧预算验证旧路径失败，证明差额来自本次追加。

## Consequences

已有结果容量的热短前缀每个非空页少申请、搬运和销毁一个中间 WordItem 向量，最终字段仍逐行深复制；没有消除 WordItem 字符串或长期缓存副本。私有 helper 分离冷未命中处理，避免重复缓存查找。最长组排序、长词首座、冷缓存紧凑副本和稀疏去重保持。没有新增文件系统读写、权限或 Android 资源行为，不要求本片 Android 设备验收。局部收益不证明 CPU 或九键 16 ms 目标通过。

### Full gates

`env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_NEURAL_MODEL_DIR=/Volumes/Workspace/worktrees/rerank-eval-resources cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 复跑通过：2172 个引擎单测、31 个 golden、294 个运行时单测，33 个既有忽略项。全目标 Clippy `-D warnings`、全仓 Rustfmt、release 两个评测入口及笔记门禁通过。独立审查核对完整旧方法与 `fd9ba47349` 一致、追加所有权和预算调整，无阻塞问题。

全拼 60 例、九键 81 例的 `convert_eval` JSON 与固定既有报告整体相等，逐例结果、来源桶和汇总均未变化，不更新质量基线。

本地 `env -u MSIME_EVAL_RESOURCES CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 通过，覆盖仓库契约、Rust 工作区、Android/WASM、Android Java、macOS 原生宿主与共享 Apple bridge。Windows 本机兼容套件 151 项通过，2 项因宿主差异排除，65 项需要 Windows 构建；完整 Windows 交叉、Linux 容器与 HarmonyOS 真打包因本机前置缺失跳过。没有宿主 GUI 或设备输入验收。

## Runtime observations

本任务构建、测试、quick 和质量评测终态后，按旧、新、新、旧串行运行 `rerank_latency --resources /Volumes/Workspace/worktrees/rerank-eval-resources --set resources/eval/sentences-nine-key-v1.tsv --grid nine`，默认丢弃 5 个预热 case，每臂测量 1791 键，16 ms 预算不变。模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`。

旧二进制来自 `01edc14f05`，SHA-256 为 `0a0cd5e3edab56480a244b3b2b370a2e67732866627b1ea94130ca38b558ae5d`；新二进制基于 `fd9ba47349` 加本片，SHA-256 为 `a68c64f8a69a630a740a4c843634f9427e2bc2f7cf69445b003c3637dd93f408`。两个基线间的 crates、Cargo 清单/锁文件、工具链与评测集无其它差异，测前测后哈希一致。

| 轮次 | engine mean/p95 ms | model mean/p95 ms | adds mean/p95 ms | model max ms | 超 16 ms 键数 | 并行构建观察数 |
| --- | --- | --- | --- | --- | --- | --- |
| 旧 1 | 3.63 / 7.04 | 6.68 / 17.72 | 3.04 / 12.03 | 30.98 | 128 | 18 |
| 新 1 | 3.50 / 6.72 | 6.51 / 17.29 | 3.00 / 11.93 | 32.19 | 122 | 7 |
| 新 2 | 3.44 / 6.61 | 6.40 / 16.95 | 2.95 / 11.93 | 29.67 | 115 | 0 |
| 旧 2 | 3.54 / 6.78 | 6.56 / 17.32 | 3.03 / 12.21 | 31.87 | 122 | 0 |

四轮均因模型臂 p95 超预算退出 1，不是运行错误。每秒监测构建进程，前两轮观察到其它构建，因此整组新旧比较作废；不挑后两轮来证明收益，也不反复重测。表格仅保留完整观察。没有观察到构建的新版本轮次也为 16.95 ms，九键 16 ms 仍未达标，性能目标保持 active。本片的采用依据是完整旧系列对照的确定性分配预算与行为/容量验证，不宣称稳定的 CPU 或运行时延迟改善。

同类的[替代合并行流](2026-10-16-quanpin-alternative-row-stream.md)消除热主候选的临时页，两处都保留最终字段的独立所有权。
