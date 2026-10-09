# Agent Note: 英文前缀空页延后候选存储预留

Status: implemented

## Problem

`EnglishDictionary::query_prefix` 在第一次 SQLite 步进之前按调用方 `limit` 预留全部 `WordItem` 容器。临时与专用英文上限为 1000，未命中也短暂持有整页空存储；为空、被锁定或只有 NULL 行的查询无需候选行容器。

## Decision

结果向量从 `Vec::new` 开始，只在第一条通过 NULL 检查且各列解析成功的行到来后执行 `reserve_exact(limit)`。命中时继续一次预留原上限，不改变 SQL、SQL limit 截断、查询参数、排序、列转换、候选字段、NULL 跳过及步进错误时丢弃整页的行为。空页不预留候选容器；本片不引入持久缓存或逐次扩容策略。

仅测试对照冻结 `4a3cbde0f52d6fc5b63c3e6450468bb8d7e9ea9e` 的完整原查询正文，完整结果逐项相等，有结果时分配次数和容量相等。热查询只用现有 `count` 比较分配次数：首次有效行之前未命中、纯 NULL 或 first step 失败且已到达旧预留点时恰好少一次候选容器分配；prepare/query 前失败无差；已有有效行后再步进失败，两侧都分配且丢弃部分结果。

精确堆峰值计量包含 `EnglishDictionary::open`、冷查询及字典析构，让连接与语句缓存的全部 Rust 存储都在区间内创建和释放，结果容器返回到区间外；最低 signed 差值为零。`rusqlite` 0.40.2 在 `prepare_cached` 命中时仍会替换 `Arc<str>` 缓存键，预热后的单次查询会释放测量前缓存键，不能把该区间的净差直接称为绝对在用峰值。只报告完整冷边界的逻辑请求峰值及返回存储，不用 minimum 为零单独证明区间没有释放旧存储。SQL 错误和缺库路径按实际到达预留点细分验证，NULL 仍占 SQL LIMIT，不能补取跳过行后面的结果。测试数据使用合成词库。

## 既有笔记审计

活跃笔记检索英文/英语空页、未命中、预分配、容量和 query_prefix，无相同决定。[临时英文前缀借用](../../implemented/testing/2026-10-08-temporary-english-prefix-borrow.md)与[专用英文前缀借用](../../implemented/testing/2026-10-08-dedicated-english-prefix-borrow.md)部分重叠于调用边界，继续约束大小写和输入语义，本片只改变词库内部空页存储。语言读音、笔画候选缓冲是不同查询及输出表示，无关。日文滚动矩阵另有工作树修改同一核心，本片保持独立。

## Alternatives considered

- 全部查询继续预留上限：命中路径只有一次容器分配且无需容量判断；未命中时也持有整页存储，热点可直接减少。
- 所有结果自然增长：空页没有候选存储，稀疏命中也少预留；密集前缀会增加多次 realloc，本片守住命中分配次数不变。
- 先执行 COUNT 再查结果：能准确预留行数；额外 SQL 步进和重复查询比这个小容量分支更重，也增加并发数据库变化边界。
- 给英文查询新增复用输出 API：能继续降低命中分配，但需要修改所有调用方及会话存储，本片先取消必定浪费的空页预留。

## Verification

新增 `dictionary/english/empty_page_tests.rs`，原查询完整正文冻结于 `4a3cbde0f52d6fc5b63c3e6450468bb8d7e9ea9e`，只翻译注释。红测试在生产修改前真实失败：未命中结果的容量为 1000，期望为 0。对照覆盖逐字段结果、精确词优先、权重超过 i32 与负权重、长度和词/展示排序、1/2/5/1000 限额、1024 行密集页、NULL 占 SQL LIMIT、非法前缀、缺连接、prepare 失败、锁库首步失败及解锁恢复。

冷测量覆盖字典打开、查询与显式析构；fixture 与路径只借用，结果返回。当前 arm64 debug 计量中 `size_of::<WordItem>() = 136`，limit 1000 未命中时旧/新分配 30/29 次，Rust 逻辑请求峰值 136460/712 字节，返回存储 136000/0 字节，最低 signed 差值均为零。新实现 limit 9999 同样为 712 字节峰值且返回零存储。热 count 不追踪 heap，命中次数/容量相等，正常 miss、纯 NULL、锁库首步失败恰好少一次；无连接、非法前缀、limit 0、prepare 失败次数相等。

删除表但继续用旧连接时，SQLite 的陈旧 schema 能让 prepare 成功、错误延迟至 step，因此 prepare 失败用新建连接并显式检查 prepare 的 Err；锁库用显式检查 rows.next 的 Err。已有有效行后的 step 失败没有在本片新增注入能力：未增加 functions/hooks 特性、依赖或 unsafe，生产 Err 分支仍丢弃部分页；根据正文审查，已有有效行时两侧都完成预留。

完整引擎单测通过 1505 项（8 项忽略），golden 31 项通过；格式与 358 篇笔记的树、格式和归档校验通过。Clippy `--all-targets -- -D warnings` 通过，完整 `scripts/verify-local.sh --quick` 通过：共享 Rust workspace、Android 目标和 Java 宿主、Linux 外壳与原生宿主（73 项测试）、macOS 原生宿主及共享 Apple bridge 均通过。Windows 本机测试 102 项通过；完整 Windows 交叉构建、pipe-only、wasm 和 HarmonyOS 真打包因工具链或资源缺失跳过。独立审查确认生产语义、冻结参考、错误分类与冷测量所有权边界，无阻断问题。未做发布构建计时或真实宿主延迟验收，本片只证明存储与次数收益。

## Consequences

未命中、纯 NULL 和首次步进失败的页省去整页候选预留；有有效候选时继续一次预留原上限。每条有效行增加一次容量零判断，命中延迟没有计时证据，不保证加速。堆计量只记录 Rust 请求字节，不包含 SQLite C 分配器内部存储或进程 RSS。`reserve_exact` 不承诺底层实际物理分配尺寸恰好等于请求；对照以逻辑容量和现有计量为准。候选上限仍可能非常大，命中页继续使用现有整页预留规则，本片不改变公开 limit 契约。冷打开可能主导小 limit 的峰值，不能要求每个空页峰值都严格下降。

[拼音底层空页延后预留](2026-10-10-pinyin-empty-row-storage.md)复用完整冷测量边界，但拼音保留 NULL 行和错误前已读结果，不能套用英文的跳过和整页丢弃规则。
