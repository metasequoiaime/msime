# Agent Note: 全长切分合并跳过已排序词库行

Status: implemented

## Problem

`QuanpinDictionary::query_series` 在完整切分的同长度词库行不足首页行数时，会先从 `query_single_path` 取得同长度候选，再把这些行按权重重新排序，随后才与更长补全词合并。`query_database` 的两个生产入口已经让这些行按权重降序返回，`segment_row_cache` 保存的也是该结果；`cache` 只由这个入口填充，所以这次排序没有改变顺序，却在每次满足首页补全条件时重复比较行。

## Decision

完整切分合并直接沿用 `query_single_path` 的权重降序顺序，只保留把更长补全词接入后的必要排序。首位精确读音保护、补全词与其余同长度行的竞争、缓存内容和候选顺序保持不变。`query_database` 的单字母 SQL、完整切分级联和行缓存继续是该顺序的唯一生产来源；如果未来向 `query_single_path` 引入未排序来源，必须在该来源处恢复排序或重新评估这里的前提。

## Historical audit

已检索全长系列、前缀追加、词库行投影和长词续接笔记。[系列前缀流式追加](../../implemented/testing/2026-10-16-quanpin-prefix-row-stream.md)只处理较短前缀的结果直接追加；[原始行投影](../../implemented/testing/2026-10-14-quanpin-row-cache-projection.md)只处理 `DictRow` 到候选字段的构造；[长词按页预留](../../implemented/testing/2026-10-10-pinyin-longer-rows-reserve.md)只处理 SQLite 多页聚合。它们都保留各自的容量和所有权决定，本项只删除同长度候选的重复比较。

## Alternatives considered

- **保留重复排序**：代码直观且能防御未来未排序来源，但当前所有生产来源已经排序，每次满足补全门槛都重复付出比较成本。
- **把排序移到缓存写入处**：可以让缓存契约显式记录排序，但会扩大缓存接口改动；现有词典查询已经提供稳定的降序契约，直接沿用更小。
- **连更长补全词也取消排序**：可以再少一次比较，但会改变补全词与非首位同长度词的权重竞争，破坏首页排序规则。

## Verification

新增合成词库回归覆盖 4 条同长度行和 32 条更长补全词，比较冻结旧系列实现与当前结果的全部候选字段、顺序和容量；生产修改前测试因两侧走同一条排序路径而先失败，修改后通过。`cargo test -p msime-engine --lib quanpin::dictionary::prefix_row_stream_tests::complete_rows_preserve_database_weight_order_when_merging --locked` 通过。

忽略的 release 配对计时在同一进程交替执行旧系列实现与当前实现，各 7 轮、每轮每侧 20,000 次热查询；旧/新墙钟比 p10 `0.990`、p50 `1.010`、p90 `1.020`。这项结果只证明局部比较成本下降，不能单独推导九键端到端 16 ms 门槛；完整引擎、golden、Clippy、fmt、quick 和九键延迟仍按合并门禁执行。

## Consequences

同长度行不再重复排序，长词合并仍按原规则从首位保护区之后排序；数据库查询、缓存失效、候选字段和返回容量不变。收益只覆盖同长度行少于首页行数且存在长词补全的查询，列表很短或没有补全时不会改变路径。该实现依赖词库查询保持降序顺序，新增其它行来源时需要同步审计。
