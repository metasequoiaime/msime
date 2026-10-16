# Agent Note: 词网格回溯借用图边并延迟复制输出词

Status: implemented

## Problem

`decode_graph` 在确认句子唯一之前复制全部词面。重复句子虽然不再拼读音，仍创建两个临时向量和全部词串；保留的每条路径也各建一次读音指针页。采样中的字符串与分配热点仍包含这类消费者。

## Decision

单次解码内复用一页图边引用，按原前驱顺序回溯并反转。借用词面精确预留并拼句，小末列按原规则查重，保留后才复制拥有词向量和拼读音。读音按边数插入分隔符，包括空字段；输出词向量仍预留音节数。缓冲在首个实际回溯时按音节数预留，小末列零预算不申请。大末列保留原完整物化和哈希回退。搜索、浮点算术、路径顺序、错字计数及公开所有权保持。

## Historical audit

按回溯、`traceback`、路径物化检索活跃笔记。[唯一预算](../../implemented/testing/2026-10-15-lattice-return-budget.md)减少访问的假设，本项减少每个已访问假设的深复制，部分重叠并互链。[终点去重](../../implemented/testing/2026-10-07-lattice-sentence-dedup.md)持有64条边界，保持。[图边所有权](../../implemented/testing/2026-10-14-lattice-graph-row-ownership.md)管理建图，本项只借用已建图。[错字计数实验](../../rejected/testing/2026-10-15-lattice-traceback-typo-count.md)改变假设字段，本项保留累计字段与展开，机制不同。其余命中的布局、列容量、候选移交和其他输入方案笔记无同一决定。

## Alternatives considered

- 跨图用共享词条消除缓存深复制：可以覆盖更多热命中，但需扩大缓存、图边和解码所有权契约，并在冷路径增加引用计数及存储；目前未取得额外收益证据，先消除本函数已确认的无用复制。
- 每条路径临时借用词面：可以延迟复制，但仍反复申请回溯缓冲；图在整个解码中稳定，一页引用足够。

## Verification

冻结 `febb4c85c6b75998764dd8aa4c700d0b5c4c66de` 的 `BeamColumn`、`Hyp`、比较器、束剪枝、列容量、个人评分、完整解码、去重与重排共10个定义。机械对照忽略注释与空白后逐个相等；入口仅扩大测试可见性。旧生产的32条单词路径预算实际减少0次，要求87次，真实失败。

新旧比较完整字段、评分位模式、顺序、路径向量、词向量、词串、整句与读音容量及返回存储。288组字段边界覆盖0/1/4列、1/4/32/64/65/80宽度、空词/空读音/嵌入撇号/Unicode与0/1/6/80预算；7680组搜索与语言模型配置各跑冷/热两轮，覆盖二元、三元、个人模型、备忘与附加图。专项三项通过。完整 `cargo test -p msime-engine -p msime-input-runtime --locked --no-fail-fast --quiet` 退出0：引擎2183、golden31、runtime294项通过，33项既有忽略；`MSIME_NEURAL_MODEL_DIR` 指向固定评测资源，清除 `MSIME_EVAL_RESOURCES`，模型用例实际执行。两crate全targets Clippy（`-D warnings`）通过。独立只读审查核对冻结定义与严格预算，无阻塞或重要问题；其指出的零预算文字范围已收窄为小末列。旧返回预算继续调用当前产品，按独立公式累计本项引用页及延迟复制收益，不改成宽松不等式或改测冻结产品。

首轮边界测试发现65条大末列、零返回预算时峰值多8字节：回溯页与去重HashSet同时存活。生产在批次去重前释放引用页后，全部边界峰值不高于旧版。下表为32条路径、4个唯一句子的Rust逻辑请求；不是RSS或真实按键延迟。

| 列数 | 分配次数：旧→新 | 请求峰值字节：旧→新 | 两侧返回字节 |
|---|---|---|---|
| 1 | 137→50 | 4426→4387 | 2988 |
| 4 | 236→65 | 9052→8896 | 3516 |
| 12 | 500→105 | 21416→20946 | 4948 |

严格节省公式为 `4 - 1 + 28 * (列数 + 2)`：四条输出省各自键指针页，重复项省两个向量及每个非空词串，扣去一页共享回溯引用。生产只在小末列查重，较大末列保留原完整物化再哈希去重。

## Fixed-resource quality

固定旧版重新由共同基线构建，再与本片release程序对照；全拼60例、九键81例完整质量JSON逐项一致。旧版也与上一片完整报告一致。模型SHA256为 `86ac529510cb3b4968a5e6ade83ec8080f5b34a0a681e75c362accbbd387d1c1`；旧/新 `rerank_latency` 校验值为 `333ce47f3c1de35b4fb4596da1b78deb0f8793a0bdda332a2609dd8fac07620f` / `fc0bb6004547948bc7d6a67dc167e6e5efaf7639a9b93019bc4881090a20531b`。没有用主工作区原先来自更旧提交的二进制作本片基线。

## Local gate and latency

共同基线上 `bash scripts/verify-local.sh --quick` 退出0，包含151项本机可运行Windows测试、契约、笔记、Rust workspace、Android目标与Java宿主、WASM、macOS原生宿主及共享Apple bridge。Linux缺Docker、Windows原生/pipe-only缺交叉工具链、HarmonyOS缺完整环境，相应阶段按脚本跳过；不将编译或跳过算作设备验收。

自身所有构建和质量评测结束后，按旧／新／新／旧启动九键16ms配对计划。预检通过，但第一轮旧版运行期间监测到其它会话启动cargo、rustc与clang，整组标为 `invalid_build_interference` 并停止后续轮次，未重测或筛选样本。本片没有有效CPU或端到端延迟结论，16ms目标继续未完成。

## Consequences

保留路径的拥有字段与容量保持，重复小路径不再复制最终丢弃的词串。引用页仅在单次解码内存活，不进入缓存或输出；零预算的小末列不申请，大末列继续原回退。直接拼接按实际边数保留空字段分隔符。共享主机存在其他构建时不采集或宣称CPU/端到端收益，确定性分配下降不代表16ms目标达标。无生产文件访问、权限或Android资源改动，没有设备验收结论。缓冲复用和字符串预留采用[Rust Vec清空保留容量](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.clear)及[String容量接口](https://doc.rust-lang.org/std/string/struct.String.html#method.with_capacity)。
