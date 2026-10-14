# Agent Note: 全拼模糊前缀延迟拥有读法

Status: implemented

## Problem

`fuzzy_candidates` 每个前缀都先 `join_segments` 构造拥有型 `typed`，再生成模糊路径；无路径时字符串直接丢弃，有路径但冷/热行页为空时也没有最终候选消费者。行缓存冷保存自己的读法副本，不能把这些副本与中间查询值混为一份必要申请。

## Decision

输入由 `split_segments` 按撇号原样切分，完整拼接必等于原参数。前缀循环从原参数借用 `&str`，每轮按最后一个撇号缩短，无路径也照常推进，预算和顺序保持不变。行追加接收可转为 `Cow<str>` 的即时值，查找与完整键核对借用其文本；冷缓存按原容量复制自己的读法，只有确实取得首行才 `into_owned` 给最终候选，其余行复制首行的同一读法。已有 `String` 参数直接转移，保留首行可能有的额外容量。

## Existing decisions

按模糊前缀、`typed`、`append_cached_fuzzy_rows`、`fuzzy_candidates` 检索活跃树。已有[模糊行缓存索引](2026-10-11-fuzzy-row-cache-key-borrow.md)持有哈希与完整字段核对，[行投影](2026-10-14-quanpin-row-cache-projection.md)持有惰性行迭代与首行接管，部分重叠并互链，保持各自决定；本篇只收窄无消费者读法的拥有时机。[系列前缀借用](2026-10-14-quanpin-series-prefix-borrow.md)提供逐次缩短字符串视图的局部参考，但它需要规范键匹配与拥有回退，模糊入口没有别名规范化，不能照搬该回退。[模糊路径复用](2026-10-09-fuzzy-path-reuse.md)只改 beam 路径容器，无关并保留。

输入音节的拥有型副本由[借用输入音节](2026-10-14-fuzzy-input-segments-borrow.md)处理；本篇保留前缀视图推进、缓存拥有副本与首行消费者规则。冻结查询矩阵在计量外独立补偿该后续切片的非空音节复制节省。

## Alternatives considered

- 只把拼接移到 `paths.is_empty` 后：改动最少，可省无模糊路径的申请；不选，因为真实空行页仍会构造无人消费的 `typed`。
- 追加辅助函数一律接收 `&str`：借用接口更简单；不选，因为已有拥有型参数的首行转移会变成复制，失去预留容量并多一次申请。
- 借用前把所有音节都改成 `&str`：还能省切分副本；不选，因为 `fuzzy_segmentations` 的拥有型输入契约与 beam 复用是独立决定，本篇只处理读法所有权。

## Verification

- 固定 `81988699f365c76910b4ec7d5e761be2018ec547` 的完整旧查询、行缓存追加和候选追加，只改名、转接冻结入口并移除注释。旧生产的完整查询矩阵在 `x/Z_ZH` 冷查询真实失败（11 次申请，目标 10），两个所有权与状态控制通过；修复后三项专项测试全部通过。每个非空且无候选消费者的前缀恰好省一次，非空页和整条候选缓存命中成本相同。
- 15 输入 × 4 组规则（关闭、Z_ZH、AN_ANG、ALL）× 0/1/129 行页 × 冷/行页热但整条答案清空/整条热共 540 组，按旧循环在计数外独立拼接前缀并计算预算；没有路径或缓存页无行时才计入一次非空拼接。对照全字段、顺序、结果与缓存容量，覆盖空/连续/首尾撇号、Unicode、预算提前终止、128 行上限、FIFO、外部失效与独立结果。断言确实覆盖无路径、空页及预算停止。
- 辅助函数的 `String` 参数独立覆盖 0/1/8 行与冷热，首行保留 256 字节容量，冷缓存读法仍为 3 字节。用两个独立数据库连接对照，排除单连接已预备 SQL 语句只污染一侧的计数。
- 全量 `cargo test -p msime-engine --lib` 通过（2026 个通过、16 个忽略）；原有 `fuzzy_candidate_rows_reuse_the_first_typed_reading_string` 预算从 34 调整为 33，因本切片恰好移除了一个首行未消费的拼接申请，重跑通过。`cargo test -p msime-engine --test golden` 的 31 项全部通过。
- `cargo clippy -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`、`pnpm run verify-notes` 和仓库 quick 门禁均在提交前完成并通过。


## Consequences

前缀缩短必须在每一实际循环轮次执行，不能让无路径的 `continue` 跳过视图推进。冷缓存仍要拥有，所有借用仅限本次调用；`Cow::into_owned` 只放在首行成功的分支。沿用 [Rust `Cow`](https://doc.rust-lang.org/std/borrow/enum.Cow.html#method.into_owned) 和既有 `rsplit_once` 模式，不增加状态、依赖、公开接口或 unsafe。不声称 CPU、设备延迟、SQLite C 堆或 RSS 收益。
