# Agent Note: 拼音按键分组空结果延后哈希表预留

Status: implemented

## Problem

`query_exact_keys_per_key` 在连接、空键批次及每键限额的保护检查之前，按输入键数创建结果 `HashMap`。非空输入全部未命中、无有效键或每键限额为零时仍分配空结果表。纠错边和滑行权重查询都使用这个入口。

## Decision

结果表保留默认 `RandomState`，从 `HashMap::new` 开始，在首个单音节非空页或首张多音节非空页入表之前，按原 `keys.len()` 一次 `reserve`。保留查询规划、64/65 键去重边界、SQL、每键行向量预留和行数限制；命中保留原容量提示与分配次数，全空批次返回零容量。

## 既有笔记审计

活跃笔记检索函数名、空 `HashMap`、分组预留和结果哈希表预留，没有相同决定。[精确键去重](../../implemented/testing/2026-10-08-pinyin-exact-key-dedup.md)约束查询规划的 `HashSet`，结果表是独立存储；[滑行输入](../../implemented/feature/2026-10-07-glide-typing-engine.md)约束调用方权重接口，不改其内容。[底层空页](../../implemented/testing/2026-10-10-pinyin-empty-row-storage.md)和[聚合空页](../../implemented/testing/2026-10-10-pinyin-empty-aggregate-storage.md)部分重叠于延后预留与冷计量边界，保留并互链，测试两侧都使用已合并的底层 helper，仅计结果表收益。

## Alternatives considered

- 保留立即预留：命中直接插入，但保护检查返回和全空批次仍持有空表存储。
- 仅把预留放到保护检查之后：能修正限额零或关闭数据库的空表，但有效批次未命中仍分配。
- 完全自然增长：空表不分配，密集命中可能多次重新哈希；延后原键数提示保持命中存储策略。

## Verification

旧生产上的真实红测试失败：96个有效键全部未命中时，结果表容量112，期望0。新增 `dictionary/pinyin/empty_key_map_tests.rs` 的8项测试通过，固定基线完整查询正文已机械对照一致，仅接收者转接、注释翻译和格式不同。覆盖单/多音节首个命中、先空后命中、跨表、重复/非法键、零限额/空输入/关闭连接、1/64/65/96/129键批次、多键密集表与1026行单键、单音节无界限额和真实首步溢出。非空映射容量、逐键 `Vec`的完整字段/顺序/容量及总分配次数严格相等；`HashMap` 迭代顺序未定义，仅按键比较。正键数的全空结果或保护检查返回少一次，空键列表两侧均无旧表预留。

冷边界在区间外显式预热不可变音节表与线程级随机哈希状态，数据库打开、PRAGMA缓存、查询规划、SQL缓存与显式析构全部在区间内；只返回结果表，输入键及fixture路径只借用。独立新进程单跑冷测试也通过，不依赖用例顺序。当前arm64 debug，96键未命中旧/新分配220/219次，Rust逻辑请求峰值9090/2810字节，返回存储6280/0字节，两侧最低signed差值均为零。同类型 `HashMap::with_capacity`独立计量的返回桶表也为6280字节，用它验证旧返回存储，不以容量乘元素大小猜桶和控制字节布局。两侧底层均使用已合并的新 `rows`，收益不重复计入底层或聚合向量。

完整引擎1534项单测通过（11项忽略）、golden31项通过；Clippy `--all-targets -- -D warnings`、格式与差异检查通过。独立审查核对冻结正文、默认哈希器、规划/限额、字段顺序和测量所有权，无阻断。笔记校验与完整 quick 通过：共享 Rust、Android target/Java、Linux 外壳和原生宿主（73 项测试）、macOS 原生宿主与 Apple bridge；Windows 本机 102 项通过。完整 Windows 交叉编译、wasm 与 HarmonyOS 真打包因工具链/资源缺失跳过。同步到包含日语引擎改动的 `dbcee3c04ad5ffabd08ae63da9d6ea8c3f7ed4e9` 后，拼音相关代码与 Cargo 配置不变；组合状态完整引擎 1537 项通过（13 项忽略）、golden 31 项通过，Clippy 再次通过。未做发布构建计时或真实宿主延迟验收。

## Consequences

空结果省去一次桶表分配，命中按原提示预留一次；首个结果前增加容量分支，没有命中延迟计时。原多音节每键 `Vec` 大提示风险保持原状。[HashMap::reserve官方契约](https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.reserve)允许超额预留，不保证表的物理大小；Rust 请求字节不代表 SQLite C 堆、系统分配器内部暂存或 RSS。
