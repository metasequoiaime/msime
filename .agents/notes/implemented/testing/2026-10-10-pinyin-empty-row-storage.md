# Agent Note: 拼音底层空页延后行存储预留

Status: implemented

## Problem

`PinyinDatabase::rows` 在首次 SQLite 步进前按 `Some(capacity)` 预留 `DictRow` 容器。有效表未命中、首步失败或首行解析失败仍申请空容器；这个 helper 服务于首字母、精确、前缀、简拼和批量查询，因此级联中未命中的查询会重复承担分配。

## Decision

底层结果从 `Vec::new` 开始，在第一条 `dict_row` 成功后、push 前且 capacity 为零时，按已有容量提示 `reserve_exact`。命中继续一次预留原提示；`None` 和 `Some(0)` 继续自然增长，实际行数超过提示时保持原有扩容策略。SQL、参数、顺序、字段转换、NULL 读作空字符串及步进/解析错误保留已读结果的行为不变。上层聚合向量的预留仍单独排查，本片只改变共享底层 helper。

仅测试参考冻结 `de6c73b254415f87e6a726335554ab71fed045f1` 的完整原 `rows` 和 `query_initial` 正文。公开首字母查询的空页零容量先在旧生产上真实失败；命中逐字段、顺序、容量与分配次数对照。热查询只用 count 比较次数，冷堆计量包含 `PinyinDatabase::open`（包括 `database_changed` 创建的 PRAGMA 缓存）、查询、显式 drop，让全部 Rust 缓存与路径的创建/析构在区间内；fixture/path 在外借用，结果返回。

准备或参数绑定失败尚未到达旧预留点，次数相等；Some 正提示下正常 miss、首步失败、首行解析失败少一次；保留过有效行后再失败两侧都预留并返回部分页；None/Some(0) 没有旧预留，无次数差。用 SQLite 内置 `abs(-9223372036854775808)` 的整数溢出配合 UNION ALL，显式确认第一步有效、第二步失败，再验证部分结果；不新增特性、依赖或 unsafe。NULL 是合法空字段，不套用英文的跳过规则。溢出条件来自 [SQLite abs 官方文档](https://www.sqlite.org/lang_corefunc.html#abs)；容量预留遵循 [Vec::reserve_exact](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.reserve_exact)，不承诺底层物理大小恰好等于请求。

## 既有笔记审计

活跃笔记检索 `query_capacity`、`PinyinDatabase::rows`、`capacity.map_or_else`、空页/底层拼音和 `DictRow`，无相同决定。[英文前缀空页预留](../../implemented/testing/2026-10-10-english-empty-prefix-storage.md)部分重叠于存储延后和 SQLite 测量边界，但英文跳过 NULL、错误丢弃整页，不能照搬。[混输拼音回退去重](../../implemented/testing/2026-10-08-pinyin-fallback-dedup.md)和[九宫格查询键去重](../../implemented/testing/2026-10-08-nine-key-query-dedup.md)是调用方去重状态，无关；笔画和粤拼目标缓冲预留约束最终 WordItem 缓冲，与本片 DictRow 的 SQLite 查询存储无关。已有[九键 PR #6377](https://github.com/metasequoiaime/msime/pull/6377) 修改简拼入口与聚合限额，不改底层 rows；旧 perf/quanpin-query-result-capacity 分支已无独立未应用提交。

## Alternatives considered

- 保留立即预留：命中路径最短，保持一次容器分配；每个有效表的未命中仍分配空存储。
- 所有行自然增长：空页无存储，稀疏命中少预留；密集页增加 realloc，不满足命中分配次数保持不变的约束。
- 先计行数再查询：可精确预留，需重复执行 SQLite 语句并引入两次查询间的数据库变化边界。
- 直接重构全部上层聚合缓冲：能继续省分配，但会混入排序/合并和多个独立容量契约；共享底层先解决一次 SQL 查询的空页预留，上层按证据另片处理。

## Verification

新增 `dictionary/pinyin/empty_page_tests.rs`。红测试在旧生产上真实失败：`query_initial("nu", 1000)` 未命中返回容量 1000，期望 0。8 项新测试通过；固定旧正文对照包含完整 rows 与 query_initial，覆盖容量提示 1/5/1000、None/Some(0)、提示不足、1025 行密集页、i32::MAX 及 usize::MAX 无界限额、负/大权重、NULL/整数/实数/损坏 UTF-8 blob 转换、prepare/绑定失败、首步失败、首行解析失败和有效行之后的真实步进失败。

第二步故障使用 SQLite 内置 abs 的整数溢出；测试先显式检查第一步返回完整有效 DictRow、第二步 Err，再检查新旧保留同一部分页，次数和容量一致。首步/首行解析失败且提示正容量恰好少一次；prepare/绑定前失败、无连接、非法首字符/limit0、None/Some(0) 的次数相等。非空页逐字段、顺序、次数、容量严格相等。

完整冷边界包含打开、database_changed 的 PRAGMA 缓存、主查询缓存与显式析构。当前 arm64 debug 的 `size_of::<DictRow>() = 56`，limit1000 未命中旧/新分配 27/26 次，Rust 逻辑请求峰值 56640/712 字节，返回存储 56000/0 字节；最低 signed 差值均为零。同位数 limit9999 的新峰值仍为712且返回零存储。最低差值为零是附加检查，所有权由闭包创建/析构边界审查保证；热计量只测次数。

完整引擎单测 1516 项通过（10 项忽略），golden 31 项通过；格式、差异检查及 360 篇笔记校验通过。Clippy 指出的测试断言无谓克隆改为借用切片，8 项测试复验与 Clippy `--all-targets -- -D warnings` 通过；完整 quick 门禁通过：共享 Rust workspace、Android target/Java 宿主、Linux 外壳与原生宿主（73 项测试）、macOS 原生宿主和 Apple bridge；Windows 本机 102 项通过，完整 Windows 交叉构建、wasm 与 HarmonyOS 真打包因工具链/资源缺失跳过。独立审查确认冻结正文、字段/错误行为、容量与冷测量所有权，无阻断问题。未做发布构建计时或真实宿主延迟验收；本片只证明行容器存储和次数收益。

## Consequences

共享底层 helper 在有正容量提示的空页和首行前失败时省去一次行容器分配；命中继续保留原提示预留与提示不足时的自然扩容。每条成功解析的行增加容量分支，尚不保证命中延迟加速。Rust 请求字节计量不包括 SQLite C 内部存储、System 内部暂存或 RSS。reserve_exact 不保证底层物理存储精确大小。上层仍有独立空页容器预留，需要后续各自验证，本片不把一个 helper 的改善当成整个查询管线零分配。
