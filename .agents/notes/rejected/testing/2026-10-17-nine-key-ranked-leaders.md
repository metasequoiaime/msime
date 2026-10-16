# Agent Note: 九键领先表复用排名键与获胜行

Status: rejected — 完整流水线126形状中10个目标未达到预定收益，恢复全部生产与测试改动

## Problem

`17db04aa0a86a7d3df6991c64e7e5eebe1e85df4` 的 `push_ranked` 已求键并保存每词最佳键，`rank_candidates` 对大于 32 条的已接纳候选再次求键，再申请去重集合。矩阵推理仍是更大的热点，本片只处理重复排名工作，不据此宣称达到 16 ms。

## Proposal

领先表值增加原始候选下标，严格改善才更新；相同键保留最早接受行。仅在最终没有单字/笔画过滤且已接纳候选大于 32 条时，按 `(key, original_index)` 排序获胜行。先建立完整的旧座位到新座位排列，赢家放前，输家补齐尾部；逐循环交换候选和排列，每次交换固定至少一座，最多 n-1 次。完成后截到唯一词数，再执行原简拼交错、个人调频与 128 条截断。候选向量容量不变。短表/最终过滤回退旧排名，但同样承担扩大 map 的成本。

## Prior decisions audit

- [缓存排名键](../../implemented/testing/2026-10-16-nine-key-cached-rank.md)：部分重叠。本片尝试翻转其中暂缓在 push 携带键的决定；短表和过滤分支仍沿用该决定，接入时互链。
- [领先表延迟分配](../../implemented/testing/2026-10-10-nine-key-leading-map-storage.md)：部分重叠，保持空表零申请与首次 reserve128，只增加值大小。
- [复制前支配过滤](../../implemented/testing/2026-10-16-nine-key-dominated-row-projection.md)：部分重叠，读取新值的 key，限制不变。
- [复制前过滤](../../implemented/testing/2026-10-16-nine-key-filter-before-clone.md)、[冷页投影](../../implemented/testing/2026-10-16-nine-key-cold-cache-projection.md)：只依赖前置过滤与查询契约，本片不改变其决定。
- 检索 proposed/implemented/rejected 的 `领先表`、`缓存键`、`cached.rank`、`线性排列`，其余命中涉及其他输入方案或平台，不属于本片；没有待清理的同题提案。

## Alternatives considered

- 沿用现状：生命周期契约最简单，且已有完整验证；代价是 push 与最终排名重复求键和独立去重，故保留为冻结对照和回退。
- 排序下标子集后逐项追索：与标准库缓存键实现相近且工作区少，但不采用缺乏明确线性上界的追索循环。
- 构造完整新到旧排列后调用 `ordering::apply_order`：复用现有循环实现，但构造输家尾部需要额外标记或再次哈希。完整逆排列只需一个 usize 向量并具备线性交换上界，先测这个方案。
- 原序 retain 获胜行后继续旧缓存排序：实现简单，但逐行查询领先表并重复求键，暂不采用。
- 原位替换同词旧行：可避免留输家，但晚到改善会保留早到座位，破坏跨词同分稳定顺序。

## Acceptance criteria

先冻结完整旧 push 与旧排名，对照全部 WordItem 字段与向量容量。debug 红绿要求不带最终过滤的大列表整个流水线每输入最多求键一次。覆盖空/1/8/32/33/64/128/512/2048、全部唯一/相同键重复/连续改善、接受数大于32但唯一数较少、三网格、prefer_exact/initials_lead、实际交错和调频、最终过滤保留与删除前部获胜行。

性能用普通 System 分配器，完整输入复制、push、map 申请/析构、排序、置换、后处理与结果析构均计时。冻结 126 个形状：7 长度（8/32/33/64/128/512/2048）×3模式（混合全唯一/同键全唯一/17词连续改善）×3网格×2最终过滤开关；每形状21对、交替先后、旧臂校准至少10ms，全部收集后统一判断。无过滤且长度至少64的目标中位新旧比必须小于0.98，其余控制小于1.05；不筛形状、放宽或重跑找绿。每对原始耗时保留。记录短表 map 膨胀和完整流水线分配次数/峰值；短表分配次数不得增加，容量必须相等。

通过后再运行固定资源质量 JSON 整体比较、真实九键 ABBA、全测/Clippy/fmt/notes/quick/pre-push 和独立审查；未过则恢复生产并交付负结果。

## Risks

更大的 map 增加短表和过滤回退成本；获胜行下标只在最终过滤前有效；HashMap 迭代顺序不能决定同分顺序；置换不能提前截断或克隆 WordItem；正式计时不能同时运行构建或采样，外部构建干扰必须记录。

## Result

完整旧臂与新臂在普通 System 分配器下运行全部 126 形状、每形状 21 对，77.42 秒后统一失败退出 101。36 个目标中 26 个通过、10 个失败；90 个控制全部通过。没有重跑、筛选或放宽门槛。逐对原始数据全部存于 [2646 对计时](../../../../docs/measurements/2026-10-17-nine-key-ranked-leaders.csv)。每秒读取进程名的监测共75次，外部构建观察累计0、峰值0；计时没有同时构建或采样。比值仅支持这台机器本次完整合成流水线的结论，不代表真实逐键延迟。

三种模式为 0 混合键且词全唯一，1 同键且词全唯一，2 17 个词连续严格改善。模式1不是同词重复。网格0/1/2分别为九键、十四键、Migala2九键双拼；filtering是会删除前部座位的按词过滤，用来覆盖下标失效后的旧路径。固定prefer_exact=true、initials_lead=false；两参数全组合由正确性测试覆盖。下表保留全部形状，中位数是21个逐对比值排序后的第11项。

| 输入数 | 模式 | 网格 | 最终过滤 | 中位新/旧 | 上限 | 结果 |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | 0 | 0 | false | 1.003172 | 1.05 | 通过 |
| 8 | 0 | 0 | true | 1.005973 | 1.05 | 通过 |
| 8 | 0 | 1 | false | 1.002421 | 1.05 | 通过 |
| 8 | 0 | 1 | true | 1.006998 | 1.05 | 通过 |
| 8 | 0 | 2 | false | 1.005140 | 1.05 | 通过 |
| 8 | 0 | 2 | true | 1.014684 | 1.05 | 通过 |
| 8 | 1 | 0 | false | 1.002074 | 1.05 | 通过 |
| 8 | 1 | 0 | true | 1.004231 | 1.05 | 通过 |
| 8 | 1 | 1 | false | 1.006275 | 1.05 | 通过 |
| 8 | 1 | 1 | true | 1.006098 | 1.05 | 通过 |
| 8 | 1 | 2 | false | 1.007001 | 1.05 | 通过 |
| 8 | 1 | 2 | true | 1.010311 | 1.05 | 通过 |
| 8 | 2 | 0 | false | 1.003141 | 1.05 | 通过 |
| 8 | 2 | 0 | true | 1.002721 | 1.05 | 通过 |
| 8 | 2 | 1 | false | 1.001082 | 1.05 | 通过 |
| 8 | 2 | 1 | true | 1.003752 | 1.05 | 通过 |
| 8 | 2 | 2 | false | 1.003619 | 1.05 | 通过 |
| 8 | 2 | 2 | true | 1.006442 | 1.05 | 通过 |
| 32 | 0 | 0 | false | 1.002447 | 1.05 | 通过 |
| 32 | 0 | 0 | true | 1.006219 | 1.05 | 通过 |
| 32 | 0 | 1 | false | 1.004269 | 1.05 | 通过 |
| 32 | 0 | 1 | true | 1.004243 | 1.05 | 通过 |
| 32 | 0 | 2 | false | 1.004582 | 1.05 | 通过 |
| 32 | 0 | 2 | true | 1.002931 | 1.05 | 通过 |
| 32 | 1 | 0 | false | 1.002277 | 1.05 | 通过 |
| 32 | 1 | 0 | true | 1.006945 | 1.05 | 通过 |
| 32 | 1 | 1 | false | 1.003273 | 1.05 | 通过 |
| 32 | 1 | 1 | true | 1.005903 | 1.05 | 通过 |
| 32 | 1 | 2 | false | 1.001623 | 1.05 | 通过 |
| 32 | 1 | 2 | true | 1.007888 | 1.05 | 通过 |
| 32 | 2 | 0 | false | 1.005689 | 1.05 | 通过 |
| 32 | 2 | 0 | true | 1.007676 | 1.05 | 通过 |
| 32 | 2 | 1 | false | 1.005639 | 1.05 | 通过 |
| 32 | 2 | 1 | true | 1.006149 | 1.05 | 通过 |
| 32 | 2 | 2 | false | 1.002404 | 1.05 | 通过 |
| 32 | 2 | 2 | true | 1.005464 | 1.05 | 通过 |
| 33 | 0 | 0 | false | 0.925123 | 1.05 | 通过 |
| 33 | 0 | 0 | true | 1.008601 | 1.05 | 通过 |
| 33 | 0 | 1 | false | 0.925972 | 1.05 | 通过 |
| 33 | 0 | 1 | true | 1.000152 | 1.05 | 通过 |
| 33 | 0 | 2 | false | 0.899098 | 1.05 | 通过 |
| 33 | 0 | 2 | true | 1.010013 | 1.05 | 通过 |
| 33 | 1 | 0 | false | 0.905596 | 1.05 | 通过 |
| 33 | 1 | 0 | true | 1.006024 | 1.05 | 通过 |
| 33 | 1 | 1 | false | 0.908190 | 1.05 | 通过 |
| 33 | 1 | 1 | true | 1.006636 | 1.05 | 通过 |
| 33 | 1 | 2 | false | 0.906814 | 1.05 | 通过 |
| 33 | 1 | 2 | true | 1.006552 | 1.05 | 通过 |
| 33 | 2 | 0 | false | 0.909475 | 1.05 | 通过 |
| 33 | 2 | 0 | true | 1.008619 | 1.05 | 通过 |
| 33 | 2 | 1 | false | 0.910473 | 1.05 | 通过 |
| 33 | 2 | 1 | true | 1.005471 | 1.05 | 通过 |
| 33 | 2 | 2 | false | 0.903211 | 1.05 | 通过 |
| 33 | 2 | 2 | true | 1.009039 | 1.05 | 通过 |
| 64 | 0 | 0 | false | 0.833985 | 0.98 | 通过 |
| 64 | 0 | 0 | true | 1.005981 | 1.05 | 通过 |
| 64 | 0 | 1 | false | 0.829701 | 0.98 | 通过 |
| 64 | 0 | 1 | true | 1.003867 | 1.05 | 通过 |
| 64 | 0 | 2 | false | 0.808310 | 0.98 | 通过 |
| 64 | 0 | 2 | true | 1.004384 | 1.05 | 通过 |
| 64 | 1 | 0 | false | 0.804581 | 0.98 | 通过 |
| 64 | 1 | 0 | true | 1.006774 | 1.05 | 通过 |
| 64 | 1 | 1 | false | 0.808354 | 0.98 | 通过 |
| 64 | 1 | 1 | true | 1.001016 | 1.05 | 通过 |
| 64 | 1 | 2 | false | 0.798652 | 0.98 | 通过 |
| 64 | 1 | 2 | true | 1.008280 | 1.05 | 通过 |
| 64 | 2 | 0 | false | 0.870121 | 0.98 | 通过 |
| 64 | 2 | 0 | true | 1.007956 | 1.05 | 通过 |
| 64 | 2 | 1 | false | 0.866536 | 0.98 | 通过 |
| 64 | 2 | 1 | true | 1.009307 | 1.05 | 通过 |
| 64 | 2 | 2 | false | 0.862424 | 0.98 | 通过 |
| 64 | 2 | 2 | true | 1.007642 | 1.05 | 通过 |
| 128 | 0 | 0 | false | 0.982014 | 0.98 | 失败 |
| 128 | 0 | 0 | true | 1.002639 | 1.05 | 通过 |
| 128 | 0 | 1 | false | 0.983783 | 0.98 | 失败 |
| 128 | 0 | 1 | true | 1.005455 | 1.05 | 通过 |
| 128 | 0 | 2 | false | 0.934289 | 0.98 | 通过 |
| 128 | 0 | 2 | true | 1.004893 | 1.05 | 通过 |
| 128 | 1 | 0 | false | 0.935578 | 0.98 | 通过 |
| 128 | 1 | 0 | true | 1.006043 | 1.05 | 通过 |
| 128 | 1 | 1 | false | 0.934267 | 0.98 | 通过 |
| 128 | 1 | 1 | true | 1.005885 | 1.05 | 通过 |
| 128 | 1 | 2 | false | 0.922624 | 0.98 | 通过 |
| 128 | 1 | 2 | true | 1.007202 | 1.05 | 通过 |
| 128 | 2 | 0 | false | 0.851921 | 0.98 | 通过 |
| 128 | 2 | 0 | true | 1.009950 | 1.05 | 通过 |
| 128 | 2 | 1 | false | 0.850021 | 0.98 | 通过 |
| 128 | 2 | 1 | true | 1.010284 | 1.05 | 通过 |
| 128 | 2 | 2 | false | 0.844409 | 0.98 | 通过 |
| 128 | 2 | 2 | true | 1.008130 | 1.05 | 通过 |
| 512 | 0 | 0 | false | 1.027168 | 0.98 | 失败 |
| 512 | 0 | 0 | true | 1.022423 | 1.05 | 通过 |
| 512 | 0 | 1 | false | 1.028926 | 0.98 | 失败 |
| 512 | 0 | 1 | true | 1.023751 | 1.05 | 通过 |
| 512 | 0 | 2 | false | 0.985427 | 0.98 | 失败 |
| 512 | 0 | 2 | true | 1.023381 | 1.05 | 通过 |
| 512 | 1 | 0 | false | 0.994944 | 0.98 | 失败 |
| 512 | 1 | 0 | true | 1.024675 | 1.05 | 通过 |
| 512 | 1 | 1 | false | 0.994000 | 0.98 | 失败 |
| 512 | 1 | 1 | true | 1.023429 | 1.05 | 通过 |
| 512 | 1 | 2 | false | 0.979902 | 0.98 | 通过 |
| 512 | 1 | 2 | true | 1.021122 | 1.05 | 通过 |
| 512 | 2 | 0 | false | 0.850468 | 0.98 | 通过 |
| 512 | 2 | 0 | true | 1.003738 | 1.05 | 通过 |
| 512 | 2 | 1 | false | 0.848612 | 0.98 | 通过 |
| 512 | 2 | 1 | true | 1.001615 | 1.05 | 通过 |
| 512 | 2 | 2 | false | 0.838002 | 0.98 | 通过 |
| 512 | 2 | 2 | true | 1.002451 | 1.05 | 通过 |
| 2048 | 0 | 0 | false | 0.958407 | 0.98 | 通过 |
| 2048 | 0 | 0 | true | 1.016668 | 1.05 | 通过 |
| 2048 | 0 | 1 | false | 0.962447 | 0.98 | 通过 |
| 2048 | 0 | 1 | true | 1.014851 | 1.05 | 通过 |
| 2048 | 0 | 2 | false | 0.930722 | 0.98 | 通过 |
| 2048 | 0 | 2 | true | 1.013403 | 1.05 | 通过 |
| 2048 | 1 | 0 | false | 1.008334 | 0.98 | 失败 |
| 2048 | 1 | 0 | true | 1.015548 | 1.05 | 通过 |
| 2048 | 1 | 1 | false | 1.011035 | 0.98 | 失败 |
| 2048 | 1 | 1 | true | 1.016947 | 1.05 | 通过 |
| 2048 | 1 | 2 | false | 1.000336 | 0.98 | 失败 |
| 2048 | 1 | 2 | true | 1.018655 | 1.05 | 通过 |
| 2048 | 2 | 0 | false | 0.846760 | 0.98 | 通过 |
| 2048 | 2 | 0 | true | 1.001348 | 1.05 | 通过 |
| 2048 | 2 | 1 | false | 0.846905 | 0.98 | 通过 |
| 2048 | 2 | 1 | true | 1.005017 | 1.05 | 通过 |
| 2048 | 2 | 2 | false | 0.839003 | 0.98 | 通过 |
| 2048 | 2 | 2 | true | 0.992847 | 1.05 | 通过 |

重复求键确实消除，但没有给所有目标带来完整流水线收益。目标比值范围0.798652–1.028926；512条混合全唯一的普通九键/十四键分别为1.027168/1.028926。新方案增加map值大小、从哈希迭代收集赢家及长度n的逆排列，不能把省下求键次数直接当成收益；本次没有做局部耗时拆分，不能断言是哪项成本单独导致失败。

## Correctness and storage

旧完整流水线的debug红测试：128条输入求键256次，预算每输入最多一次，唯一一项失败，其余3项通过、1项手动忽略。接入后6项通过、1项手动忽略，包括实际生产逆置换helper对n=0..8所有完整排列的穷举，以及160行实际交错/调频与原始128截断线之外的简拼词可见断言。

独立审查发现最初“各排名维度”样本先增加覆盖长度，后续来源/模糊/读法变体全部被遮住。修正为各维独立同词序列，断言该维实际改变键并检查冻结旧版实际赢家字段后，重新运行6项均通过。正式计时二进制在这项正确性测试修正前构建；计时入口、fixture、生产算法与冻结对照没有变化，未在普通分配器二进制运行分配断言。独立审查核实冻结push/rank正文与基线机械一致，旧refresh只适配到冻结旧push，未随新leading类型改变。

以下是完整复制、push、排名与析构的计数分配器测量：空结果无申请，1/8/32条分配次数与旧版相同，但1条起map请求峰值增加2048字节；并非短表没有成本。此处是逻辑请求字节，不是进程RSS。

| 输入数 | 模式 | 旧申请次数 | 新申请次数 | 旧峰值字节 | 新峰值字节 |
| --- | --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 0 | 0 |
| 0 | 1 | 0 | 0 | 0 | 0 |
| 0 | 2 | 0 | 0 | 0 | 0 |
| 1 | 0 | 9 | 9 | 13162 | 15210 |
| 1 | 1 | 9 | 9 | 13157 | 15205 |
| 1 | 2 | 9 | 9 | 13157 | 15205 |
| 8 | 0 | 58 | 58 | 14158 | 16206 |
| 8 | 1 | 59 | 59 | 14128 | 16176 |
| 8 | 2 | 59 | 59 | 14128 | 16176 |
| 32 | 0 | 224 | 224 | 25551 | 27599 |
| 32 | 1 | 230 | 230 | 25472 | 27520 |
| 32 | 2 | 215 | 215 | 25342 | 27390 |
| 33 | 0 | 232 | 233 | 24503 | 26815 |
| 33 | 1 | 238 | 239 | 24417 | 26729 |
| 33 | 2 | 222 | 223 | 24279 | 26079 |
| 64 | 0 | 443 | 444 | 27595 | 30155 |
| 64 | 1 | 455 | 456 | 27424 | 29984 |
| 64 | 2 | 408 | 409 | 27018 | 28074 |
| 128 | 0 | 884 | 884 | 43057 | 45865 |
| 128 | 1 | 905 | 905 | 42712 | 45520 |
| 128 | 2 | 800 | 794 | 42723 | 40955 |
| 512 | 0 | 3500 | 3500 | 176630 | 188918 |
| 512 | 1 | 3597 | 3597 | 175453 | 187741 |
| 512 | 2 | 3108 | 3100 | 133508 | 118684 |
| 2048 | 0 | 13952 | 13952 | 710789 | 759941 |
| 2048 | 1 | 14353 | 14353 | 706049 | 755201 |
| 2048 | 2 | 12328 | 12318 | 498730 | 431682 |

2,048条全唯一的峰值请求增加49,152字节；连续改善的同词列表则减少67,048字节。候选向量容量和全部WordItem字段由冻结对照逐形状核对。

## Reproduction

测量机器为Apple M1 Max、arm64，macOS 27.0.1（26A434），Rust 1.97.1（8bab26f4f68e0e26f0bb7960be334d5b520ea452，LLVM 22.1.6），host/target均为 `aarch64-apple-darwin`。正式计时采用仓库默认release profile（速度优化，未另设RUSTFLAGS或CARGO_TARGET_DIR），CARGO_BUILD_JOBS=4只用于先行构建。

固定基线 `17db04aa0a86a7d3df6991c64e7e5eebe1e85df4`，在仓库外新建worktree；应用下面完整补丁并创建下面测试文件。普通debug运行 `cargo test --locked -p msime-engine --lib ranked_leaders_tests -- --nocapture`。要重现重复求键的红测试，先只加入测试模块声明及测试文件，将测试production臂末尾的filtering分支替换成单次原 `rank_candidates` 调用；同时给 `inverse_order_matches_every_small_permutation` 加 `#[cfg(any())]`，禁用此时基线尚不存在的 `apply_inverse_rank_order` helper 测试。保持生产源码其余部分为基线。其余新增行为用例在首次红测试后补入，因此该方法复现256次求键的预算失败，不复现首次红阶段的测试项数。不能把不认识新类型或helper的编译错误当红测试。

正式计时要暂时移除 `crates/engine/src/ime/personal_rerank.rs` 中唯一 `#[global_allocator]` 属性，构建 `cargo test --release --locked -p msime-engine --lib --no-run --message-format=json`，从compiler-artifact的executable字段取得测试二进制并复制到worktree外，finally逐字恢复allocator源码。仅运行复制二进制的 `--exact nine_key::ranked_leaders_tests::full_pipeline_paired_budget --ignored --nocapture --test-threads=1`；不在该二进制运行分配断言。构建、测试、采样都终态后才开始配对计时。

普通分配器计时二进制SHA-256为 `f377f83097a1003c29e2d2efc3d44694b69fa8d7eb52236ac82fbb266df7c60e`。为了后续可能的真实资源比较，曾在改生产前构建当前基线的rerank_latency/convert_eval，但门禁失败后未运行改版整模型评测、质量JSON比较、ABBA或平台装机，不据此声明16ms目标有进展。

## Frozen patch

```diff
diff --git a/crates/engine/src/nine_key.rs b/crates/engine/src/nine_key.rs
index cedb3f18da..04cee84651 100644
--- a/crates/engine/src/nine_key.rs
+++ b/crates/engine/src/nine_key.rs
@@ -1209,7 +1209,7 @@ impl NineKeySession {
         candidates.clear();
         let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
         // 各条切分的前缀组彼此大量重复，一次刷新会推入上万行，见 `push_ranked`。
-        let mut leading: HashMap<String, RankKey> = HashMap::new();
+        let mut leading: HashMap<String, RankLeader> = HashMap::new();
         // Only a split the user typed says where a syllable ends; without one, `3` must keep 的 (a completion of d) ahead of the rarer 额 (e).
         let prefer_exact = !self.splits.is_empty();
         // 每个数字都能当一个音节的首字母时也按简拼查（`68` 是 m't：明天、每天）。用户在每个数字之间都打了切分（`6'8`），说的就是简拼，简拼行排在前面；没打切分时数字也可能是完整音节（`68` 是 mu），简拼行排在同样覆盖的音节行之后。
@@ -1264,7 +1264,7 @@ impl NineKeySession {
                             return false;
                         };
                         // 只查已接纳的前页结果；本页内的同词仍由 `push_ranked` 处理。
-                        if *best
+                        if best.key
                             <= rank_key_for_reading(
                                 candidate,
                                 covered,
@@ -1406,13 +1406,24 @@ impl NineKeySession {
             let strokes = (!self.strokes.is_empty()).then_some(&self.stroke_texts);
             candidates.retain(|item| passes_filter(&item.word, self.single_character, strokes));
         }
-        rank_candidates(
-            &mut candidates,
-            prefer_exact,
-            initials_lead,
-            boost.as_ref(),
-            grid,
-        );
+        if filtering {
+            rank_candidates(
+                &mut candidates,
+                prefer_exact,
+                initials_lead,
+                boost.as_ref(),
+                grid,
+            );
+        } else {
+            rank_leading_candidates(
+                &mut candidates,
+                &leading,
+                prefer_exact,
+                initials_lead,
+                boost.as_ref(),
+                grid,
+            );
+        }
         self.rerank_sentences(&mut candidates);
         let mut seeds = std::mem::take(&mut self.seed_buffer);
         next_seeds(
@@ -2335,6 +2346,12 @@ fn query_key_is_new(
 /// `rank_candidates` 的排序键，小的在前。
 type RankKey = (Reverse<usize>, bool, bool, bool, bool, Reverse<i64>);
-
+
+#[derive(Clone, Copy)]
+struct RankLeader {
+    key: RankKey,
+    index: usize,
+}
+
 /// More digits covered first. Synthesised rows (whole-sentence Generated, Fallback) score on a different scale from dictionary weights, so within one coverage bucket dictionary rows lead; then exact before fuzzy, then weight.
 /// With `prefer_exact` (the user typed a split), a row the typed digits spell to its end then leads one that has to be completed past them: over `94'26` 西安 (xi'an) comes before 自从 (zi'cong), however common the longer word. Without a split the digits do not say where a syllable ends, so `3` keeps 的 (de) ahead of the rarer 额 (e) by weight.
 /// 简拼行（`is_initials_row`）在同样覆盖的词典行里的位置由 `initials_lead` 决定：用户在每个数字之间都打了切分时排在前面，否则先排在音节行之后、整句行之前，去重后再由 `interleave_initials` 按权重插进音节行里。
@@ -2522,10 +2539,10 @@ fn deduplicate_english_words(words: &mut Vec<WordItem>) {
     words.truncate(write);
 }
-
+
-/// 推入一行，除非同一个词已有一行排得不比它靠后。`leading` 记着每个词目前排得最靠前的那一行的排序键。被跳过的行在稳定排序后必然落在那一行之后（键更大，或键相同而推入更晚），会被 `retain_unique_words` 删掉；它也不会让别的行多删或少删，因为它能挡住的行那一行同样挡得住。所以跳过与全部推入再 `rank_candidates`，结果完全相同。前提是两边用同一个 `prefer_exact`。
+/// 推入一行，除非同一个词已有一行排得不比它靠后。`leading` 记着每个词目前排得最靠前的那一行的排序键与入列下标。被跳过的行在稳定排序后必然落在那一行之后（键更大，或键相同而推入更晚），会被 `retain_unique_words` 删掉；它也不会让别的行多删或少删，因为它能挡住的行那一行同样挡得住。所以跳过与全部推入再 `rank_candidates`，结果完全相同。前提是两边用同一个 `prefer_exact`。
 fn push_ranked(
     candidates: &mut Vec<WordItem>,
-    leading: &mut HashMap<String, RankKey>,
+    leading: &mut HashMap<String, RankLeader>,
     candidate: WordItem,
     prefer_exact: bool,
     initials_lead: bool,
@@ -2533,18 +2550,85 @@ fn push_ranked(
 ) {
     let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
     match leading.get_mut(candidate.word.as_str()) {
-        Some(best) if *best <= key => return,
-        Some(best) => *best = key,
+        Some(best) if best.key <= key => return,
+        Some(best) => {
+            *best = RankLeader {
+                key,
+                index: candidates.len(),
+            }
+        }
         None => {
             if leading.is_empty() {
                 leading.reserve(CANDIDATE_LIMIT);
             }
-            leading.insert(candidate.word.clone(), key);
+            leading.insert(
+                candidate.word.clone(),
+                RankLeader {
+                    key,
+                    index: candidates.len(),
+                },
+            );
         }
     }
     candidates.push(candidate);
 }
-
+
+/// 领先表须由相同排序参数构建，且入列后没有过滤、重排或改行；短列表沿用原排序。
+fn rank_leading_candidates(
+    candidates: &mut Vec<WordItem>,
+    leading: &HashMap<String, RankLeader>,
+    prefer_exact: bool,
+    initials_lead: bool,
+    boost: Option<&InitialsBoost>,
+    grid: KeyGrid,
+) {
+    if candidates.len() <= SMALL_CANDIDATE_SORT {
+        rank_candidates(candidates, prefer_exact, initials_lead, boost, grid);
+        return;
+    }
+    let mut ranked: Vec<_> = leading
+        .values()
+        .map(|best| (best.key, best.index))
+        .collect();
+    // 原下标打破同键平局，恢复稳定排序，不依赖哈希表的迭代顺序。
+    ranked.sort_unstable();
+    let mut inverse = vec![usize::MAX; candidates.len()];
+    for (target, &(_, original)) in ranked.iter().enumerate() {
+        inverse[original] = target;
+    }
+    let unique = ranked.len();
+    let mut tail = unique;
+    for target in &mut inverse {
+        if *target == usize::MAX {
+            *target = tail;
+            tail += 1;
+        }
+    }
+    apply_inverse_rank_order(candidates, &mut inverse);
+    candidates.truncate(unique);
+    drop(inverse);
+    drop(ranked);
+    if !initials_lead {
+        interleave_initials(candidates);
+    }
+    if let Some(boost) = boost {
+        boost_used_initials(candidates, initials_lead, boost);
+    }
+    candidates.truncate(CANDIDATE_LIMIT);
+}
+
+/// 完整旧座位到新座位排列：每次交换固定目标座位，累计最多 n-1 次。
+fn apply_inverse_rank_order<T>(items: &mut [T], inverse: &mut [usize]) {
+    debug_assert_eq!(items.len(), inverse.len());
+    for index in 0..inverse.len() {
+        while inverse[index] != index {
+            let target = inverse[index];
+            items.swap(index, target);
+            inverse.swap(index, target);
+        }
+    }
+}
+
 /// 按 `rank_key` 稳定排序、按词去重，简拼交错和个人调频后再截断（NK:283-307）。
 fn rank_candidates(
     candidates: &mut Vec<WordItem>,
@@ -3102,6 +3186,8 @@ mod isolated_test;
-
+
 #[cfg(test)]
 mod cached_rank_tests;
+#[cfg(test)]
+mod ranked_leaders_tests;
-
+
 #[cfg(test)]
 mod initials_code_tests;
diff --git a/crates/engine/src/nine_key/dominated_projection_tests.rs b/crates/engine/src/nine_key/dominated_projection_tests.rs
index 5ce469eefd..c2d08f7f6b 100644
--- a/crates/engine/src/nine_key/dominated_projection_tests.rs
+++ b/crates/engine/src/nine_key/dominated_projection_tests.rs
@@ -1,4 +1,6 @@
 use super::isolated_test as isolation;
+// 冻结刷新继续调用旧入列，避免参考结果跟随生产领先表改变。
+use super::ranked_leaders_tests::reference_push as push_ranked;
 use super::*;
 use crate::ime::personal_rerank::allocations::count;
 use crate::quanpin::fixture::Fixture;
diff --git a/crates/engine/src/nine_key/filter_projection_tests.rs b/crates/engine/src/nine_key/filter_projection_tests.rs
index 56bd4474e6..ff5fceb10f 100644
--- a/crates/engine/src/nine_key/filter_projection_tests.rs
+++ b/crates/engine/src/nine_key/filter_projection_tests.rs
@@ -1,4 +1,6 @@
 use super::isolated_test as isolation;
+// 冻结刷新继续调用旧入列，避免参考结果跟随生产领先表改变。
+use super::ranked_leaders_tests::reference_push as push_ranked;
 use super::*;
 use crate::ime::personal_rerank::allocations::count;
 use crate::quanpin::fixture::Fixture;
```

## Complete test file

`crates/engine/src/nine_key/ranked_leaders_tests.rs`：

```rust
use super::*;
use std::hint::black_box;
use std::time::Instant;

// 固定 17db04aa0a 的完整入列和排名；共用未修改的键与后处理。
pub(super) fn reference_push(
    candidates: &mut Vec<WordItem>,
    leading: &mut HashMap<String, RankKey>,
    candidate: WordItem,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) {
    let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
    match leading.get_mut(candidate.word.as_str()) {
        Some(best) if *best <= key => return,
        Some(best) => *best = key,
        None => {
            if leading.is_empty() {
                leading.reserve(CANDIDATE_LIMIT);
            }
            leading.insert(candidate.word.clone(), key);
        }
    }
    candidates.push(candidate);
}

/// 按 `rank_key` 稳定排序、按词去重，简拼交错和个人调频后再截断（NK:283-307）。
fn reference_rank(
    candidates: &mut Vec<WordItem>,
    prefer_exact: bool,
    initials_lead: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
) {
    if candidates.len() <= SMALL_CANDIDATE_SORT {
        candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
    } else {
        candidates.sort_by_cached_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
    }
    retain_unique_words(candidates);
    if !initials_lead {
        interleave_initials(candidates);
    }
    if let Some(boost) = boost {
        boost_used_initials(candidates, initials_lead, boost);
    }
    candidates.truncate(CANDIDATE_LIMIT);
}

fn grids() -> [KeyGrid; 3] {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .expect("内置九键双拼表");
    [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(
            crate::nine_key_shuangpin::nine_key_shuangpin(&table).expect("合法表"),
        ),
    ]
}

fn fixture(length: usize, mode: usize) -> Vec<WordItem> {
    (0..length)
        .map(|index| {
            let mixed = (index * 73 + 17) % 101;
            let word = if mode == 2 { index % 17 } else { index };
            let mut row = WordItem::new(
                if mode == 0 {
                    &"64426646"[..1 + mixed % 8]
                } else {
                    "626"
                },
                format!("样例{word}"),
                match mode {
                    0 => mixed as i64 * 31 - 1000,
                    1 => 100,
                    _ => index as i64,
                },
                if mode == 0 && index % 7 == 0 {
                    CandidateSource::Generated
                } else {
                    CandidateSource::Database
                },
                if mode == 0 {
                    ["ni'hao", "a", "ming'tian", "", "lü'e"][mixed % 5]
                } else {
                    "man"
                },
            );
            row.fixed_position = (index % 6) as i32;
            row.fuzzy = mode == 0 && index % 7 == 0;
            row.corrected_from = format!("synthetic-{index}");
            row.sentence_association = index % 2 == 0;
            row.sentence_words = vec![format!("part-{index}")];
            row
        })
        .collect()
}

// 按词面过滤，刻意删除前部下标；同词各行的去留保持一致。
fn filter(rows: &mut Vec<WordItem>) {
    rows.retain(|row| row.word.as_bytes().last().is_some_and(|byte| byte % 2 != 0));
}

type Pipeline = fn(&[WordItem], bool, bool, Option<&InitialsBoost>, KeyGrid, bool) -> Vec<WordItem>;

#[inline(never)]
fn reference(
    input: &[WordItem],
    exact: bool,
    initials: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
    filtering: bool,
) -> Vec<WordItem> {
    let mut candidates = Vec::new();
    let mut leading = HashMap::new();
    for row in input {
        reference_push(
            &mut candidates,
            &mut leading,
            row.clone(),
            exact,
            initials,
            grid,
        );
    }
    if filtering {
        filter(&mut candidates);
    }
    reference_rank(&mut candidates, exact, initials, boost, grid);
    candidates
}

#[inline(never)]
fn production(
    input: &[WordItem],
    exact: bool,
    initials: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
    filtering: bool,
) -> Vec<WordItem> {
    let mut candidates = Vec::new();
    let mut leading = HashMap::new();
    for row in input {
        push_ranked(
            &mut candidates,
            &mut leading,
            row.clone(),
            exact,
            initials,
            grid,
        );
    }
    if filtering {
        filter(&mut candidates);
    }
    if filtering {
        rank_candidates(&mut candidates, exact, initials, boost, grid);
    } else {
        rank_leading_candidates(&mut candidates, &leading, exact, initials, boost, grid);
    }
    candidates
}

#[test]
#[cfg(debug_assertions)]
fn full_pipeline_does_not_evaluate_accepted_keys_again() {
    let input = fixture(128, 0);
    cached_rank_tests::KEY_EVALUATIONS.with(|count| count.set(0));
    let rows = production(&input, true, false, None, KeyGrid::NineKey, false);
    assert!(!rows.is_empty());
    let count = cached_rank_tests::KEY_EVALUATIONS.with(std::cell::Cell::get);
    assert!(
        count <= input.len(),
        "完整流水线重复求键：{count} 次，{} 条输入",
        input.len()
    );
}

#[test]
fn full_pipeline_preserves_fields_capacity_ties_improvements_and_filters() {
    for length in [0, 1, 8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            for grid in grids() {
                for exact in [false, true] {
                    for initials in [false, true] {
                        for filtering in [false, true] {
                            let expected =
                                reference(&input, exact, initials, None, grid, filtering);
                            let actual = production(&input, exact, initials, None, grid, filtering);
                            assert_eq!(actual, expected, "length={length}, mode={mode}, grid={grid:?}, exact={exact}, initials={initials}, filtering={filtering}");
                            assert_eq!(actual.capacity(), expected.capacity());
                        }
                    }
                }
            }
        }
    }
    // 同词同键拒绝、晚到改善与别词同分的稳定顺序。
    let mut input = fixture(64, 1);
    let mut duplicate = input[0].clone();
    duplicate.corrected_from = "same-key-late".into();
    input.push(duplicate);
    let mut improved = input[1].clone();
    improved.weight += 1;
    improved.corrected_from = "improved-late".into();
    input.push(improved);
    let mut tied = input[2].clone();
    tied.weight += 1;
    tied.word = "并列词".into();
    input.push(tied);
    let expected = reference(&input, true, true, None, KeyGrid::NineKey, false);
    let actual = production(&input, true, true, None, KeyGrid::NineKey, false);
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
}

#[test]
fn full_pipeline_preserves_active_interleaving_and_boost() {
    let input: Vec<_> = (0..160)
        .map(|index| {
            let initials = index >= 136;
            WordItem::new(
                "626",
                if initials {
                    format!("简拼{}", index - 136)
                } else {
                    format!("音节{index}")
                },
                if initials { 2000 + index } else { 1000 - index },
                CandidateSource::Database,
                if initials { "ni'hao'ma" } else { "man" },
            )
        })
        .collect();
    let boost = InitialsBoost {
        used: [("简拼0".into(), 100)].into(),
        frequency: FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Linear,
            trigger_count: 1,
            linear_step: 2,
        },
    };
    for grid in grids() {
        for exact in [false, true] {
            for initials in [false, true] {
                let raw = reference(&input, exact, initials, None, grid, false);
                let boosted = reference(&input, exact, initials, Some(&boost), grid, false);
                assert_ne!(raw, boosted, "样本必须触发调频");
                if !initials {
                    assert!(
                        raw.iter().any(|row| row.word.starts_with("简拼")),
                        "低于原始截断线的简拼必须实际交错到可见范围"
                    );
                }
                for (boost, expected) in [(None, &raw), (Some(&boost), &boosted)] {
                    let actual = production(&input, exact, initials, boost, grid, false);
                    assert_eq!(&actual, expected);
                    assert_eq!(actual.capacity(), expected.capacity());
                }
            }
        }
    }
}

#[test]
fn full_pipeline_records_heap_and_preserves_short_allocation_count() {
    for length in [0, 1, 8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            let measure = |pipeline: Pipeline| {
                crate::ime::personal_rerank::allocations::measure(|| {
                    let rows = pipeline(&input, true, false, None, KeyGrid::NineKey, false);
                    black_box(rows);
                })
                .1
            };
            let old = measure(reference);
            let new = measure(production);
            assert_eq!(old.remaining_bytes, 0);
            assert_eq!(new.remaining_bytes, 0);
            if length <= 32 {
                assert_eq!(old.allocations, new.allocations);
            }
            println!(
                "heap {length} {mode} {} {} {} {}",
                old.allocations, new.allocations, old.peak_bytes, new.peak_bytes
            );
        }
    }
}

fn elapsed(
    pipeline: Pipeline,
    input: &[WordItem],
    grid: KeyGrid,
    filtering: bool,
    repeats: usize,
) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        // 整个入列与排名：输入复制、map/工作区申请、后处理及全部析构均在区间内。
        black_box(pipeline(
            black_box(input),
            true,
            false,
            None,
            grid,
            filtering,
        ));
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动完整入列加排名126形状配对；仅在普通分配器构建中运行"]
fn full_pipeline_paired_budget() {
    let mut failures = Vec::new();
    println!("pair,length,mode,grid,filtering,repeats,pair,old_seconds,new_seconds");
    for length in [8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            for (grid_index, grid) in grids().into_iter().enumerate() {
                for filtering in [false, true] {
                    assert_eq!(
                        production(&input, true, false, None, grid, filtering),
                        reference(&input, true, false, None, grid, filtering)
                    );
                    let mut repeats = 1;
                    while elapsed(reference, &input, grid, filtering, repeats) < 0.010 {
                        repeats *= 2;
                    }
                    let mut ratios = Vec::new();
                    for pair in 0..21 {
                        let (old, new) = if pair % 2 == 0 {
                            let old = elapsed(reference, &input, grid, filtering, repeats);
                            (old, elapsed(production, &input, grid, filtering, repeats))
                        } else {
                            let new = elapsed(production, &input, grid, filtering, repeats);
                            (elapsed(reference, &input, grid, filtering, repeats), new)
                        };
                        println!("pair,{length},{mode},{grid_index},{filtering},{repeats},{pair},{old:.9},{new:.9}");
                        ratios.push(new / old);
                    }
                    ratios.sort_by(f64::total_cmp);
                    let ratio = ratios[10];
                    let budget = if length >= 64 && !filtering {
                        0.98
                    } else {
                        1.05
                    };
                    println!(
                        "result,{length},{mode},{grid_index},{filtering},{ratio:.6},{budget:.2}"
                    );
                    if ratio >= budget {
                        failures.push((length, mode, grid_index, filtering, ratio, budget));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "完整流水线配对预算失败：{failures:?}");
}

#[test]
fn inverse_order_matches_every_small_permutation() {
    fn visit(inverse: &mut [usize], start: usize) {
        if start == inverse.len() {
            let mut values: Vec<_> = (0..inverse.len()).collect();
            let mut expected = vec![0; inverse.len()];
            for (old, &new) in inverse.iter().enumerate() {
                expected[new] = old;
            }
            let mut actual_inverse = inverse.to_vec();
            apply_inverse_rank_order(&mut values, &mut actual_inverse);
            assert_eq!(values, expected);
            assert!(actual_inverse
                .iter()
                .enumerate()
                .all(|(seat, &target)| seat == target));
            return;
        }
        for next in start..inverse.len() {
            inverse.swap(start, next);
            visit(inverse, start + 1);
            inverse.swap(start, next);
        }
    }
    for count in 0..=8 {
        visit(&mut (0..count).collect::<Vec<_>>(), 0);
    }
}

#[test]
fn full_pipeline_preserves_winner_changes_across_rank_dimensions() {
    for grid in grids() {
        for exact in [false, true] {
            for initials in [false, true] {
                for dimension in 0..6 {
                    // 每个维度独立建同词序列，避免更长覆盖遮住后续维度。
                    let mut first =
                        WordItem::new("6266", "目标维度", 100, CandidateSource::Database, "man");
                    let mut second = first.clone();
                    match dimension {
                        0 => second.pinyin.push('6'),
                        1 => first.source = CandidateSource::Generated,
                        2 => {
                            first.pinyin = "626".into();
                            second.pinyin = "626".into();
                            first.canonical_pinyin = "ni'hao'ma".into();
                        }
                        3 => first.canonical_pinyin = "ni'hao'ma".into(),
                        4 => first.fuzzy = true,
                        _ => second.weight += 1,
                    }
                    let first_key = rank_key(&first, exact, initials, grid);
                    let second_key = rank_key(&second, exact, initials, grid);
                    if dimension != 3 || exact {
                        assert_ne!(
                            first_key, second_key,
                            "维度必须实际改变键：{dimension}, {grid:?}"
                        );
                    }
                    let (mut worse, mut better) = if first_key < second_key {
                        (second, first)
                    } else {
                        (first, second)
                    };
                    worse.corrected_from = "first-worse".into();
                    better.corrected_from = "late-winner".into();
                    let mut same = better.clone();
                    same.corrected_from = "later-same-key".into();
                    let mut input = fixture(64, 0);
                    input.extend([worse, better.clone(), same]);
                    for filtering in [false, true] {
                        let expected = reference(&input, exact, initials, None, grid, filtering);
                        if !filtering && first_key != second_key {
                            assert_eq!(
                                expected.iter().find(|row| row.word == "目标维度"),
                                Some(&better)
                            );
                        }
                        let actual = production(&input, exact, initials, None, grid, filtering);
                        assert_eq!(actual, expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                    }
                }
            }
        }
    }
}
```

## Restoration verification

被否后，`crates/engine/src/nine_key.rs`、两份旧refresh测试、计数分配器均与固定基线逐字相同；新增测试文件已删除，完整测试与patch只保留于本笔记。恢复后固定模型资源目录运行 `cargo test --locked -p msime-engine --no-fail-fast --quiet`：2,242项单测、31项golden通过，38项保持忽略，无失败；Rustfmt与diff空白检查通过。没有用改版质量或真实逐键延迟结果替代失败的合成门禁。

本地 `bash scripts/verify-local.sh --quick` 通过：仓库契约检查、笔记检查、Rust工作区与共享Apple bridge编译通过；macOS未构建产品bundle，Tauri外壳跳过，各平台无关阶段按最终仅md/csv的范围跳过，没有平台GUI验收。`pnpm run verify-notes` 通过1,202篇活跃笔记及4份归档封印。独立审查逐行复算126个统计、核对全部2,646对原始记录、27行内存数据和源码恢复；修正红测试复现缺口后无阻断项。
