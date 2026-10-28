# Agent Note: 双拼与触屏误触评测及平面表基线

Status: implemented

## Problem

#10018 要让移动端 26 键双拼在盲打时把按错的邻键纠回来，后续几期会改纠错的计价、座位和学习。改之前没有任何可复现的数字：`convert_eval` 和 `rerank_latency` 只打全拼，而且关着纠错；[双拼纠错整句](../feature/2026-10-09-shuangpin-typo-sentence.md)里的召回（573 条误触纠回 38 条）和延迟是一次性测量，没有提交测量代码，造误触的方式也只覆盖「中间音节的韵母键按成邻键」这一种。触屏上按错哪个键取决于键位几何和手抖的幅度，仓库里也没有能合成这种误触的东西。引擎这一期还收不到触点，所以这里只能先量「按到了别的键」之后今天的平面表能做到什么，给以后每一期一个对照。

## Decision

`convert_eval` 加 `--scheme shuangpin`，把现有评测集编成 26 键双拼的按键再打；`--touch-mode` 在参考键位几何上合成误触。产品行为不变：引擎只多了两个只读的公开函数 `host::encode_shuangpin` 和 `host::is_shuangpin_syllable`，全拼路径的输出与 merge-base 逐字节相同。

### 编码

- 用例的输入是不带分隔的全拼。先按 TSV 里的音节数切成音节，只认只有一种切法的。切音节按与方案无关的音节表（`host::is_shuangpin_syllable`，即引擎的 `accepted_syllables`），不按这个方案编不编得出来。首道双拼的 `en` 和 `ia` 同在 `k` 上，`den` 编出来解回 `dia`，它编不出 `den`；要是按方案编不编得出来切，`dena` 在首道上就只剩 de'na 一种切法，看起来没有歧义，首道就会比其余方案多打几条有歧义的用例，各方案比的就不是同一批用例。然后用 `host::encode_shuangpin`（包住引擎解码用的同一个 `shuangpin::utils::encode_pinyin`）逐音节编成两键码，去掉 `'`。纠错整句不接受手打的 `'`，所以按键里不带它，每个音节恰好两键。
- 切法有歧义（`fangan` 两个音节可以是 fang'an 也可以是 fan'gan）、有音节在这个方案里编不出来的用例丢掉，编出来的条数按方案写进报告的 `encoding`。五个内置方案编出的条数相同：整句族都是 1519 / 1580 条，词级族都是 2748 / 2772 条。
- 切音节的递归和九键双拼共用（`grid_keys.rs` 的 `syllable_pieces`），差别只在「认不认这个音节」的判据：九键双拼按它自己的码表，26 键双拼按与方案无关的音节表。

### 触点合成（`touch_noise.rs`）

- 几何文件就是将来宿主下发给引擎的 JSON 形状：`id`（非零代次）、`keys`（a 到 z 的 `[cx, cy, w, h]`）、`semicolon`、`duplicates`（分离键盘的重复键）。读入时校验数值有限、宽高为正、26 个字母齐全，不合格整份拒绝。
- 宿主只在微软双拼下显示 `;`，第二行因此多一个键、键宽也不同，所以每个平台一份不带 `;` 的和一份 `-microsoft`；微软双拼取带 `;` 的那份，其余方案取不带的。
- 落点服从 N(键心 + (0, b), σ²)，σ 和 b 以键为单位，按键宽、键高的中位数归一。落在矩形里就是按到它，落在键距或字母区外归最近的键。
- 引擎收不到触点，所以每个想按的键只需要「按到各键的概率」这一行分布。它用网格积分一次算好：±5σ 的方格按 σ/8 划格，格心密度为权，按格心命中的键归类后归一。抽样只需要均匀随机数。
- 四种模式：
  - `none`：原样打；
  - `clean`：每键都按在本键上；
  - `noisy`：至少一键按错，按条件分布精确抽样，先按 P(第一个按错的是第 j 键) ∝ (∏_{i<j}(1−q_i))·q_j 选位置，不靠反复重抽；
  - `realistic`：不加条件。
- 每条用例的随机数流只由种子、方案、模式、σ、偏置和用例序号经 splitmix64 混合后决定，配置的增减不会挪动其余结果。

### 报告

- 每组配置一项，按音节档（1、2、3–5、6+）和错键数档（0、1、2、3+）分桶。
- 比例指标：`top1`、`top3`、`top9`（第一页）、`found`、`mrr`。
- 计数指标：`top1_changed` 和 `gold_demoted`，拿同一条用例按想按的键原样打出的名次作对照。写计数而不是比例，因为首选被改这类事件要逐条看，千分之一的取整会把它抹掉。
- 一条用例的候选表只取决于上文和送进去的按键，所以同一串按键在各组配置里只打一次（`gold_rank` 的记忆表）。`clean` 和 σ 很小的 `realistic` 因此几乎不花时间。
- 每组配置在 JSON 里占四行，整份报告 125 组，整句族 93 KB、词级族 78 KB。

### 基线与命令

- `resources/eval/baseline-shuangpin-touch-sentences.json`：sentences-v1、sentences-neutral-v1、sentences-v2、sentences-particles-v1 共 1580 条。
- `resources/eval/baseline-shuangpin-touch-words.json`：quanpin-words-v1 的 2 音节子集，`--max-syllables 2 --limit 3000` 抽出 2772 条。词级集生成时已丢掉单字，所以没有 1 音节用例。
- 两份都是 5 个内置方案 × {none, clean, noisy, realistic} × σ{0, 0.15, 0.25, 0.35} × b{0, 0.1}，开 `--autocorrect`，用 `android-portrait` 和 `android-portrait-microsoft` 两份几何，种子 20261027。完整命令写在 `resources/eval/README.md`「双拼与触屏误触」。
- 资源目录只有 `desktop-dictionary.lock.json` 那一组文件加 `sentence-model.safetensors`，没有 settled 模型 `sentence-model-desktop.safetensors`（它由另一份锁 `settled-model.lock.json` 发布）。`convert_eval` 见到那个文件就挂上 settled 重排，排序会变，两份基线就对不上，并不是产品改了。所以复跑时 stderr 必须打出 `no settled model at ...`。10-09 那篇笔记的全拼验证特意加了这个文件，照它准备资源目录会得到 `eval baseline mismatch`。
- 在 Mac Studio 上，整句族一次跑完要一到五小时，看同时跑的 CI 有多少（负载 50 左右时约一小时，负载在 300–780 之间时约五小时），词级族要 5 分钟。所以两份基线不进 `verify-local.sh`，由改双拼纠错的那一期自己跑、自己对照。
- `rerank_latency` 接受同样的 `--scheme shuangpin --shuangpin-profile --autocorrect`，再给 `--key-geometry` 和 `--touch-sigma` 时按 realistic（或 `--touch-mode` 指定的模式）合成误触。
- 延迟工具与召回工具同样拒绝非有限值和负数噪声参数；`noisy` 无法产生错键时跳过该用例，不能退回正确按键污染误触计时。全部用例都被跳过时明确失败，避免把空样本当作通过门禁。

### 参考布局

`resources/eval/key-geometry/` 下有 7 份：Android、iPhone、HarmonyOS 竖屏各两份，`ipad-split` 一份（没有微软双拼版）。它们按各端常量和行规则构造，不是真机导出的，构造规则写在 README。宿主接入触点那一期换成真机导出的几何后，基线要重跑。

### 依赖

`msime-input-runtime` 的 dev-dependencies 加 `rand` 0.10.2（不带默认特性）和 `rand_pcg` 0.10.2，两者已在 Cargo.lock 里，只给这两个 example 用。网格积分只需要均匀随机数，所以批准过的 `rand_distr` 没有加。

## 平面表基线

下面都是今天的平面表（纠错整句）经过 `Runtime` 和重排模型之后的数字，σ=0.25、b=0。五个方案之间差别都在一两个点以内，按小鹤报，其余见基线文件。

| 集合 | none top1 / top9 | noisy top1 / top9 / found | noisy 单错 top9 | realistic 净 top1 | realistic 有错的用例 |
|---|---|---|---|---|---|
| 整句 1519 条 | 0.679 / 0.876 | 0.001 / 0.003 / 0.101 | 0.003（1177 条） | 0.427 | 37.7% |
| 2 音节词 2748 条 | 0.733 / 0.991 | 0.009 / 0.053 / 0.070 | 0.055（2615 条） | 0.644 | 12.5% |

- **整句的纠错整句几乎从不出现在第一页。** 单错的整句 13% 能在候选表里找到原句（3–5 音节 21%，6 个以上 8%），这和 10-09 那次「573 条纠回 38 条」是同一个量级。但第一页召回只有 0.3%。拆开 sentences-v1 加 particles 那次可复现运行的 dump 看，按错之后仍找得到原句的 1105 次里，1086 次来源是 `generated`，965 次排在整张表的最后一位。原因是 `ordering::runner_up_order`：三字以上的读法只把头三条词图读法留在前面，其余同宽的 `generated` 行全挪到表尾，纠错整句也是其中一行。10-09 那次是直接查引擎、没经过 `Runtime`，所以看到的是第 2 位。
- **方案计划里「runtime 不用改」的前提因此不成立。** 只改引擎计价，上不了第一页，PR1a 的召回门槛量不出差别；需要让纠错行不受这条沉底规则影响，否则召回停在这个数。这里只记录，不改，因为这一期不改产品行为。
- **2 音节没有纠错**：纠错整句至少要 3 个音节，所以词级族 noisy 的 5% 第一页召回全是碰巧（按错后的码恰好也能出原词）。这是 PR4 的起点。
- **σ 决定了一切**：σ=0.15 时几乎没有用例按错（整句 realistic 有错的 0.3%），σ=0.35 时整句 89% 有错，净 top1 掉到 0.08。偏置 b=0.1 在 σ=0.25 时把整句有错的用例从 38% 推到 47%，2 音节词从 12.5% 推到 14.4%。
- **clean 与 none 逐项相同**：引擎收不到触点之前，`clean` 送进去的键就是想按的键。两份基线里每组 clean 的总体和音节分桶都与同方案的 none 相同，`top1_changed`、`gold_demoted` 都是 0。
- **延迟**（`rerank_latency`，sentences-v1 60 条，同一时段、负载 50–60）：
  - 全拼：with model p50/p95/p99 = 0.27 / 7.36 / 11.61 ms；
  - 小鹤关纠错：2.06 / 8.15 / 12.85 ms；
  - 小鹤开纠错：2.08 / 8.01 / 12.03 ms；
  - 开纠错 + realistic σ=0.25：2.02 / 9.23 / 14.76 ms，超一帧的按键 0.6%；
  - 开纠错 + noisy σ=0.25：1.16 / 7.19 / 11.36 ms。
  - 机器同时在跑 CI，这组数字只作量级参考。规划预算（96 键）的用量引擎没有对外暴露，这一期量不到。

## Alternatives considered

- **用 `rand_distr` 的正态分布直接撒点、按命中重抽**：最直观，也是方案原本申请 `rand_distr` 的理由。σ=0.15 时单键按错的概率在千分之几以下，`noisy` 要「至少一键按错」只能反复重抽整条用例，短词要抽成千上万次，而且条件分布只是近似。网格积分一次给出每键的按错分布，条件抽样是精确的，还少一个依赖。代价是离散化误差和 ±5σ 截断，在这几个 σ 下可以忽略。
- **把合成好的误触用例提交成 TSV**：评审时能直接看到每条用例按成了什么。但它绑死了几何、σ 和偏置，换真机几何就要整批重生成、重审，方案也定了不提交派生 TSV。现在按种子在进程里合成，`--dump` 能导出实际打出的按键供复核。
- **每组配置一份基线文件（方案里的命令是单个方案、单个 σ）**：每份很小，和现有基线一样一份一组。可是两族 × 125 组就是 250 份文件。逗号分隔的列表让一次运行覆盖全部组合，单值写法仍然可用，形状一样。
- **现在就做真机几何**：最接近真实，但三端宿主还没有导出几何的入口，那是宿主那一期的事。参考布局只用来让流程先跑通、先出数，README 写明是构造的。
- **把双拼基线接进 `verify-local.sh`**：能自动发现回退，但完整组合要一个小时，现有 11 组加起来才二十几分钟。等纠错的哪一期需要时再挑几组小的接进去。

## Consequences

- **收益**：
  - 双拼纠错第一次有可复现的召回、精度和延迟数字。同一种子同一份二进制两次输出逐字节相同，含 `--dump`。
  - 五个内置方案打的是同一批用例（切音节的歧义按与方案无关的音节表判断），none 模式下两族的数字逐项相同：none 送进去的是想按的键，各方案解回的读音相同，这只说明编码和解码互为逆，不说明各方案的纠错能力一样。触屏模式下各方案的按错分布随键位而变，数字才开始分开。
  - 这次就量出了今天平面表在 `Runtime` 里实际上不可见这件事，以后每一期都有对照。
- **代价与已知上限**：
  - 落点模型偏乐观：只有各向同性的高斯加统一偏置，不模拟拇指的系统性偏移、同一音节两键都错、漏键和多键；功能键也不建模，落在大小写、删除、候选栏的点算成最近的字母。
  - 参考几何是构造的。
  - 基线绑定 `dict-v2.0.24` 和当前的 `sentence-model.safetensors`，而且是在没有 settled 模型的条件下生成的；这几样一变就要重跑。
  - 重访信号：宿主导出真机几何之后要重跑；引擎开始消费触点（PR1a）后，`clean` 才会和 `none` 分开，记忆表的键也必须带上触点，因为那时候选表就不只取决于按键了。

## Verification

- `cargo test -p msime-input-runtime --example convert_eval --example rerank_latency`：20 / 15 项通过。新增几类测试：
  - 落点模型：键距归最近的键、σ=0 永不按错、行分布归一且随 σ 变宽、偏置让第一排更容易按到第二排、noisy 每条都有错且 clean 一条都没有、同种子同结果、坏几何被拒；
  - 分桶与对照计数；
  - `shuangpin_keys` 的切音节与歧义丢弃，以及歧义按与方案无关的音节表判断（`dena` 在五个方案上都丢掉，首道双拼的 `den` 整条丢掉而不改打另一种切法；按方案编不编得出来切时这一项失败）。
- `cargo test -p msime-engine --lib host::` 123 项通过，含新增的 `encode_shuangpin_uses_the_named_builtin_profile` 和 `shuangpin_syllables_do_not_depend_on_the_profile`。`cargo clippy -p msime-input-runtime -p msime-engine --all-targets -- -D warnings` 无警告。
- **G0**：分支和 merge-base（`ad2e677c96`）的 `convert_eval` 各跑一遍现有 11 组，输出逐字节相同。其中 sentences、harvested、neutral、words、nine-key 五组与仓库里提交的基线不一致，merge-base 自己也一样，是基线落后于 develop，与这次改动无关，这里不更新它们。
- **可复现**：sentences-v1 加 particles、全部 5 个方案和全部配置，同一种子跑两遍，报告 JSON 和 66 MB 的 dump 都逐字节相同。
- **其余几何**：sentences-v1 加 particles（每个方案 147 条），iPhone、HarmonyOS、ipad-split 和 Android 各跑一组 noisy 和 realistic σ=0.25、b=0。realistic 有错的用例：Android 19.7%–27.2%，iPhone 19.0%–27.9%，HarmonyOS 17.7%–24.5%，ipad-split 23.8%–29.3%，同一量级。ipad-split 最高，原因不是键宽：σ 按键宽、键高归一，键本身多大不影响按错率。它的键约 60 pt 宽、键距只有 6，键距相对键宽更窄：键距归最近的键，所以键心到与邻键分界的距离是半个键心间距，归一之后 ipad-split 横向只有 0.54–0.55 个键宽（Android 0.57–0.62），纵向 0.565 个键高（Android 0.587），算按到本键的那块区域更小。iPhone 的第二行按宿主的规则两端各缩进半个键距、键宽同第一行，HarmonyOS 的键高按 `KeyboardMetrics.ROW_HEIGHT_VP` 取 44。
- **未跑**：`verify-local.sh` 整套、golden 测试、宿主相关的一切。这一期不碰产品代码路径，只多两个公开函数。
