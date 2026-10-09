# Agent Note: 拼音按键分组借用输入键

Status: implemented

## Problem

`query_exact_keys_per_key` 已经借用输入键完成校验和去重，但多音节键进入按表分组时仍复制到 `BTreeMap<String, Vec<String>>`。这些副本只用于生成 SQLite `IN (...)` 参数，查询完成前始终可以借用调用方的键字符串。

## Decision

将分组值改为 `Vec<&str>`，让表分组只拥有表名，按键保持借用；`batch_rows` 改为泛型 `AsRef<str>`，在参数迭代时借用键文本。单音节查询仍按原策略复制命中键进入结果表，SQL、表顺序、去重、每键限额、字段、排序、容量和返回所有权保持不变。

私有 `batch_rows` 的字符串转换遵循 [Rust `AsRef<str>`](https://doc.rust-lang.org/std/convert/trait.AsRef.html)，参数迭代使用锁定版本的 [rusqlite `params_from_iter`](https://docs.rs/rusqlite/0.40.2/rusqlite/fn.params_from_iter.html)。同文件简拼查询已有 `Vec<&str>` 分组；该版本 `String` 和 `str` 的 `ToSql` 都返回相同的借用文本。

## 既有笔记审计

[拼音完整键规划借用音节切片](../../implemented/testing/2026-10-10-pinyin-borrowed-key-plan.md)消除了规划阶段的音节向量，本片继续处理规划之后的分组键副本；[按键空结果表延后预留](../../implemented/testing/2026-10-10-pinyin-empty-key-map-storage.md)只约束结果表容量；[去重后拆分](2026-10-08-pinyin-exact-key-dedup.md)处理重复键的音节分配。这三篇均部分重叠并互链，滑行输入的候选选取笔记不涉及分组存储，归为无关；没有完全吸收或过时提案。冻结测试的旧查询继续保留拥有型分组，用独立计量补偿本片新增的键克隆节省。

## Alternatives considered

- 保留 `Vec<String>`：实现最简单，但每个唯一多音节键仍复制一次，无法利用已经借用的输入。
- 让通用 `batch_rows` 接受专门的字符串切片：会为借用和拥有两种容器重复查询实现；`AsRef<str>` 保留一个参数绑定路径。
- 直接让整个结果表借用输入键：结果键来自 SQLite 行或单音节输入，生命周期和返回所有权不同，扩大范围且不必要。

## Verification

新增 `borrowed_key_groups_tests.rs` 的 7 项测试，覆盖唯一/重复/非法键、跨表、七音节与溢出表、首步错误、空连接、零限额、无界单音节限额、密集页和结果容量；另将拥有/借用参数与冻结旧 `batch_rows` 的字段、顺序、容量和次数逐项比较，覆盖内嵌零字节、缺表与全部限额边界，并证明返回行在输入释放后仍有效。96 个唯一双音节键的冷测量为分配 `231 -> 135`、逻辑请求峰值 `13779 -> 12120` 字节，返回存储 `6413` 字节保持不变；单音节与非法键路径保持原分配边界。既有借用规划、去重和空结果表冻结断言已补偿本片的旧分组键克隆，结果、字段、顺序、容量和错误边界逐项相等。拼音模块 81 项通过，完整引擎 1560 项通过、13 项忽略，golden 31 项通过；Clippy `--all-targets -- -D warnings`、格式、差异、笔记和 quick 门禁通过。quick 中 Linux 原生宿主 73 项通过，Windows 本机 102 项通过；完整 Windows 交叉构建、wasm 与 HarmonyOS 真打包因工具链/资源缺失跳过。

## Consequences

多音节分组不再拥有输入键副本，借用生命周期由本次查询的 `keys` 参数保证；`batch_rows` 只在绑定 SQLite 参数时读取这些切片。表名、结果行和单音节命中仍按原策略拥有存储。该片只降低 Rust 临时分配，不宣称 SQLite C 堆、RSS 或真实宿主延迟改善。

当前结果槽通过[复用已有结果键](2026-10-10-pinyin-result-key-reuse.md)消除重复行的键副本；本篇冻结正文与历史测量保留，热次数预算在区间外独立补偿该聚合收益，既有冷场景每键至多一行而无需追加补偿。
