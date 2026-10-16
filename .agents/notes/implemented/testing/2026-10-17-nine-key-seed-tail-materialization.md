# Agent Note: 九键种子尾段只物化实际采用的路径

Status: implemented

## Problem

`seeded_paths` 每条种子只采用 `SEED_TAIL_LIMIT`（3）条尾段，但 `split_paths` 先为最多 48 条最终路径复制向量和音节字符串；其余结果随后立即释放。既有采样把路径构造列为九键刷新热点，本片先用受控分配对照验证浪费，不从旧采样推断局部占比。

## Decision

完整保留逐偏移搜索和非根页的稳定排序、48 条上限；普通调用根页仍为 48，种子尾段根页先选择最小 3 条再排序，只物化这 3 条。根页无前驱消费者，比较完全相等的路径具有相同音节输出，选择不稳定性不改变返回文本序列。空尾段原处理不变。

## Historical audit

[路径链](../../implemented/testing/2026-10-16-nine-key-path-chain.md)部分重叠，仍负责中间页后继，本片收窄最终物化。[整句种子](../../implemented/feature/2026-10-10-nine-key-sentence-seeds.md)定义 3 条采用上限，本片保持该决定。[双拼](../../implemented/feature/2026-10-10-nine-key-shuangpin.md)曾否决增加尾段数量，本片不扩大。无完全吸收或过时提案。

## Alternatives considered

- 降低全部中间页上限到 3：还可减少排序，但会改变获胜路径，无法保持原候选。
- 最终返回借用迭代器：能延后物化，但需要把后继页所有权跨函数携带；显式输出上限足够覆盖当前消费者。

## Verification criteria

冻结 `8ef7f5a4d4` 的完整切分和种子追加；三种网格、默认/非均匀频度、空/不匹配/完整/重复种子及预填路径逐项相等，分配不增加，密集尾段至少少 100 次。普通系统分配器 release 每形状 40 对、交替先后、旧臂校准 10 ms；8 形状×3 网格×2 频度完整保留，长尾目标中位比 <0.98，短或空控制 <1.05。质量全拼 60 与九键 81 例 JSON 相等，保留实际 ABBA 和原 16 ms 预算。

## Consequences

除根页外，搜索和排序仍完整运行；根页选择与物化收益不能证明端到端 16 ms 达标。

## First variant observations

仅限制物化的首轮完整 48 形状有一个空种子控制失败（加权九键 1.079823 >=1.05），24 目标通过；50 次监测均未观察到并行构建。反汇编确认空分支的新旧入口、空检查与返回指令序列相同。原样复测全部 48 形状后空控制全部通过，但九键默认频度形状 5 为 0.981431 >=0.98，仍拒绝接入。两组原始记录均保留于 worktree 旁的 `-paired.log`、`-paired-repeat.log`。不修改门槛，进一步对无消费者的根页使用选择后排序；其余页仍完整稳定排序并保留48条。

两轮完整矩阵（新/旧中位比；形状 0–3 控制 <1.05，4–7 目标 <0.98）：

| 网格 | 加权 | 形状 | 首轮 | 复测 |
| --- | --- | ---: | ---: | ---: |
| NineKey | false | 0 | 1.034259 | 0.999108 |
| NineKey | false | 1 | 0.999646 | 1.002604 |
| NineKey | false | 2 | 1.005329 | 1.005441 |
| NineKey | false | 3 | 0.699671 | 0.701718 |
| NineKey | false | 4 | 0.530388 | 0.530318 |
| NineKey | false | 5 | 0.975976 | 0.981431 |
| NineKey | false | 6 | 0.945101 | 0.954086 |
| NineKey | false | 7 | 0.976729 | 0.976507 |
| NineKey | true | 0 | 1.079823 | 1.000497 |
| NineKey | true | 1 | 1.002764 | 0.997592 |
| NineKey | true | 2 | 1.005490 | 1.004751 |
| NineKey | true | 3 | 0.754700 | 0.753460 |
| NineKey | true | 4 | 0.491235 | 0.494995 |
| NineKey | true | 5 | 0.925891 | 0.942400 |
| NineKey | true | 6 | 0.864829 | 0.868582 |
| NineKey | true | 7 | 0.919222 | 0.938855 |
| FourteenKey | false | 0 | 1.033757 | 1.000765 |
| FourteenKey | false | 1 | 0.999478 | 0.997181 |
| FourteenKey | false | 2 | 1.004852 | 1.004344 |
| FourteenKey | false | 3 | 0.821123 | 0.820889 |
| FourteenKey | false | 4 | 0.921163 | 0.918666 |
| FourteenKey | false | 5 | 0.909438 | 0.910971 |
| FourteenKey | false | 6 | 0.869545 | 0.870919 |
| FourteenKey | false | 7 | 0.909757 | 0.910457 |
| FourteenKey | true | 0 | 1.049004 | 0.993793 |
| FourteenKey | true | 1 | 0.998214 | 0.996026 |
| FourteenKey | true | 2 | 1.004462 | 1.003943 |
| FourteenKey | true | 3 | 0.846615 | 0.845599 |
| FourteenKey | true | 4 | 0.934979 | 0.944093 |
| FourteenKey | true | 5 | 0.745341 | 0.742250 |
| FourteenKey | true | 6 | 0.702876 | 0.705536 |
| FourteenKey | true | 7 | 0.754498 | 0.754936 |
| NineKeyShuangpin | false | 0 | 1.042166 | 0.998418 |
| NineKeyShuangpin | false | 1 | 1.003437 | 1.000248 |
| NineKeyShuangpin | false | 2 | 1.003044 | 1.004945 |
| NineKeyShuangpin | false | 3 | 0.767108 | 0.769240 |
| NineKeyShuangpin | false | 4 | 0.512753 | 0.513101 |
| NineKeyShuangpin | false | 5 | 0.856955 | 0.856468 |
| NineKeyShuangpin | false | 6 | 0.700691 | 0.703518 |
| NineKeyShuangpin | false | 7 | 0.859742 | 0.855138 |
| NineKeyShuangpin | true | 0 | 1.034306 | 1.000664 |
| NineKeyShuangpin | true | 1 | 0.999276 | 1.000428 |
| NineKeyShuangpin | true | 2 | 1.003800 | 1.004365 |
| NineKeyShuangpin | true | 3 | 0.794831 | 0.797234 |
| NineKeyShuangpin | true | 4 | 0.619205 | 0.621858 |
| NineKeyShuangpin | true | 5 | 0.878414 | 0.880400 |
| NineKeyShuangpin | true | 6 | 0.745364 | 0.747436 |
| NineKeyShuangpin | true | 7 | 0.881266 | 0.881910 |

形状按测试 `cases` 顺序：0 空输入无种子，1 不匹配种子，2 完整前缀无尾段，3 单音节尾段，4 双音节尾段，5 长尾段，6 两种种子，7 重复和完整种子混合。所有输入均合成。每格 40 对原始耗时保留，不删除失败格。

正式配对的 release test executable 在构建时仅临时把 `personal_rerank.rs` 的唯一 `#[global_allocator]` 改为 `#[allow(dead_code)]`，运行 `cargo test --release --locked -p msime-engine --lib --no-run --message-format=json`，复制 Cargo 报告的程序后立即逐字恢复；它只运行忽略的计时测试，不运行分配断言。分配断言在恢复后的普通 debug 测试程序运行。模型与资源固定，正式计时不得与构建、采样或其它计时并行。

## Final root selection verification

单纯物化版的性能红门禁驱动根页改为选择后排序。功能测试通过三项（另一个计时手动忽略）；固定夹具旧/新为 290/64 次申请，未物化路径向量与字符串预算 225，根页排序还少一次临时申请，总共少 226。分配矩阵要求所有格不增加并至少省掉未物化路径的全部申请。最终页限额的冻结输出对照覆盖默认/非均匀先验、三网格、显式切分、0/1/3/48/49 限额和确定随机的1/2/4/8/16/32/42位输入；42位仅为 helper 健壮性覆盖，公开输入仍上限32。

正式普通系统分配器完整配对 48 形状、1920 对全部通过，48 次监测未观察到编译。24 目标范围为 0.255390–0.953740，24 控制为 0.716621–1.002181；门槛与原方案相同，没有删除失败格或更改形状。完整比值如下：

| 网格 | 加权 | 形状 | 新/旧中位比 |
| --- | --- | ---: | ---: |
| NineKey | false | 0 | 0.995062 |
| NineKey | false | 1 | 0.999101 |
| NineKey | false | 2 | 0.987669 |
| NineKey | false | 3 | 0.716621 |
| NineKey | false | 4 | 0.406743 |
| NineKey | false | 5 | 0.889777 |
| NineKey | false | 6 | 0.673871 |
| NineKey | false | 7 | 0.888326 |
| NineKey | true | 0 | 1.000019 |
| NineKey | true | 1 | 1.000941 |
| NineKey | true | 2 | 0.986724 |
| NineKey | true | 3 | 0.745964 |
| NineKey | true | 4 | 0.408018 |
| NineKey | true | 5 | 0.875226 |
| NineKey | true | 6 | 0.669680 |
| NineKey | true | 7 | 0.864661 |
| FourteenKey | false | 0 | 1.002181 |
| FourteenKey | false | 1 | 0.998431 |
| FourteenKey | false | 2 | 0.986720 |
| FourteenKey | false | 3 | 0.823265 |
| FourteenKey | false | 4 | 0.924235 |
| FourteenKey | false | 5 | 0.255390 |
| FourteenKey | false | 6 | 0.289075 |
| FourteenKey | false | 7 | 0.262864 |
| FourteenKey | true | 0 | 0.999271 |
| FourteenKey | true | 1 | 1.001783 |
| FourteenKey | true | 2 | 0.986588 |
| FourteenKey | true | 3 | 0.858834 |
| FourteenKey | true | 4 | 0.953740 |
| FourteenKey | true | 5 | 0.438025 |
| FourteenKey | true | 6 | 0.479130 |
| FourteenKey | true | 7 | 0.459102 |
| NineKeyShuangpin | false | 0 | 0.999164 |
| NineKeyShuangpin | false | 1 | 1.001950 |
| NineKeyShuangpin | false | 2 | 0.988207 |
| NineKeyShuangpin | false | 3 | 0.784792 |
| NineKeyShuangpin | false | 4 | 0.753358 |
| NineKeyShuangpin | false | 5 | 0.950748 |
| NineKeyShuangpin | false | 6 | 0.883579 |
| NineKeyShuangpin | false | 7 | 0.952250 |
| NineKeyShuangpin | true | 0 | 0.998257 |
| NineKeyShuangpin | true | 1 | 1.001840 |
| NineKeyShuangpin | true | 2 | 0.990770 |
| NineKeyShuangpin | true | 3 | 0.814171 |
| NineKeyShuangpin | true | 4 | 0.329848 |
| NineKeyShuangpin | true | 5 | 0.699423 |
| NineKeyShuangpin | true | 6 | 0.558798 |
| NineKeyShuangpin | true | 7 | 0.703840 |

该二进制 SHA-256 为 `329776a14ad2d142da6b0d434eee19654c1384a129f134d178c0e2b94af1d87c`，分配器源码恢复哈希为 `4cbb8b21744f1d395a559957e5b21f98b01e988e16f73a3dce2a81d9a28a9552`，与基线逐字一致。原始计时和逐秒构建观察保留在 worktree 外同名前缀 `-root-paired.log`、`-root-paired.monitor.json`，前两轮首版记录同样保留。

根页无前驱消费者、比较相等项具有相同音节序列，经独立只读审查通过。[Rust `select_nth_unstable_by`](https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by)将前缀分区后再排序，保留完整比较器；没有依赖、unsafe、持久化或公开接口改动。

最终全拼60例、九键81例完整质量报告和全部候选JSON逐项一致。共同基线为 `8ef7f5a4d4`，模型 SHA-256 为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`；旧/新质量程序 SHA-256 为 `251d48358018e1a14e39f14451a8cd02c9fd5097fd9790d278098fb03ea1cea4` / `57fcbb0bcb72937a9eafd0aa2c07940544061c36528fe8b133481ecae81218f3`。未更新质量金标。

## End-to-end latency

最终版本按旧、新、新、旧串行测量，81例、每臂1791键、丢弃5个预热case，普通System allocator、同一资源和原16ms预算。四轮均因p95超预算退出1。旧/新程序SHA-256分别为 `b1e26ccb51a8a786e095a35cb4daada842bd8581061c19de55f86c7a3543876e` / `ea6b88ce6515e52370b83250241cd7662e2eec2fc426447d136a8ee6d64d2159`，测前测后哈希一致。逐秒构建监测各19/19/18/19次，均未观察到编译重叠；不能排除调度、温度或其它系统任务影响。

| 轮次 | engine mean/p95 ms | model mean/p95 ms | adds mean/p95 ms | model max ms | 超16ms |
| --- | --- | --- | --- | ---: | ---: |
| 1 old | 3.44 / 6.69 | 6.44 / 17.04 | 3.00 / 11.91 | 31.39 | 115 |
| 2 new | 3.41 / 6.53 | 6.39 / 17.05 | 2.99 / 11.83 | 30.22 | 114 |
| 3 new | 3.43 / 6.58 | 6.40 / 17.12 | 2.97 / 11.95 | 30.28 | 113 |
| 4 old | 3.50 / 6.64 | 6.50 / 17.23 | 3.00 / 11.92 | 30.42 | 119 |

新版p95处于旧版两轮之间，不能据此证明稳定端到端提速，也没有达到16ms；目标保持active。接入证据限定为完整局部矩阵通过、分配下降与质量输出相同。原始日志与构建监测保留于 worktree 外同名前缀 `-abba-*.log`、`-abba-*.monitor.json`。

## Final repository gates

验证脚本顺序结果为：`cargo test --locked -p msime-engine -p msime-input-runtime --no-fail-fast --quiet` 通过（引擎 2250、golden 31、运行时 317，40 项忽略）；两 crate 全 targets Clippy `-D warnings` 通过；`cargo fmt --all -- --check` 通过。`bash scripts/verify-local.sh --quick` 完成其余阶段，但以已有的 HarmonyOS settings bundle 检查失败退出；该失败在本片改动前即存在，日志首段明确为 `the HarmonyOS settings page does not build`，本片没有改动 HarmonyOS 文件。Android host Java、WASM、macOS 配置/编译、共享 Apple bridge 和仓库检查均完成；HarmonyOS 因缺少 `hvigorw`/staged 资源跳过编译，Linux Docker 与 Windows MinGW 阶段按环境缺失跳过。这是本地编译和单测门禁，不是 Android 设备验收。
