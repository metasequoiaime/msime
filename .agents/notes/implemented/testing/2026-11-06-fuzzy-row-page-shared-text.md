# Agent Note: 模糊前缀页共用文字字段，行缓存上限按组合数归一

Status: implemented

## Problem

#10457 的报告者在 vivo V2046A 上开着全部 11 条模糊音打九键。11-05 的 nightly 诊断包显示引擎计算中位 18–24 ms、p95 约 50 ms，整句候选开关差别不明显，大头是九键加模糊音时的引擎计算。[上一篇](2026-11-02-fuzzy-row-cache-path-count.md)让行缓存命中时不再展开路径之后，本机 M4 Pro 上 `ninekey_latency`（模糊音全开、整句关、400 轮）的 macOS `sample` 采样里 `fuzzy_candidates` 仍占刷新的 24%，其中：

- 查库 `query_exact_segmentations_keyed_flat` 约占一半，只在行缓存没命中时走到。
- 把一页 `DictRow` 投影成候选：命中页每行先复制 `key`、`value` 两个字符串，再为读音复制一份，然后给三个文本字段各包一个 `Arc`，每行 6 次申请；冷页给缓存另复制一份两个字符串，同样是每行 6 次。`WordItem::new` 加字符串复制约占 `fuzzy_candidates` 的 17%，这些字符串在候选表、整条候选缓存和系列缓存淘汰时还要再逐个释放。
- 行缓存的键是（规则、上限、读音）。上限是 `min(64, 剩余预算)`，九键一键的二三十条切分各自剩下的预算不同，同一段读音前缀在不同切分里会以不同的上限查询。用探针计数（未入库）看，一轮 `ninekey_latency` 里行缓存没命中 3333 次，其中只是上限不同的键有 598 个，663 次没命中发生在上限小于 64 时。

## Decision

**行缓存页存共享字段。** `CachedFuzzyRows.rows` 是 `Vec<FuzzyRow>`，`FuzzyRow` 的 `key`、`value` 是 `SharedString`；`typed` 也是 `SharedString`，是命中时每个候选的 `pinyin`。`append_fuzzy_items` 把页投影成候选时只克隆这三个字段（加引用计数），冷页查库后先把行换成 `FuzzyRow` 存进缓存，再和命中时走同一条投影。任何一边原地改写共用的字段时，`SharedString` 按写时复制先复制，所以候选与缓存互不影响，与[整页复制共用文本字段](2026-10-29-page-copy-shared-strings.md)是同一个做法。缓存里的读音按长度复制一份（`SharedString::from(&str)`），调用方交来带余量的缓冲区时不让缓存一直占着它。冷页的行向量按行数申请：原地收集会沿用 SQLite 页按上限预留的整块缓冲区。

**行缓存键的上限按组合数归一。** `fuzzy_segmentations` 每层都按字典序截取前 `limit` 条，所以结果恰好是全部组合按字典序的前 `min(limit, P)` 条去掉原切分，`P` 是各音节读法个数之积（`fuzzy_variant_count`，`beam_result_depends_only_on_the_smaller_of_limit_and_combinations` 逐个上限核对）。`fuzzy_candidates` 切音节时顺带算出每个前缀的 `P`（`CountedSyllable`），`fuzzy_row_key_limit` 在 `P <= limit` 时把键里的上限记作 `FUZZY_SEGMENTATION_LIMIT`，否则保留实际上限。键里的数总满足「用它展开会得到同样的路径」，而行只由路径决定，所以命中项与重新查库完全相同。`P <= 1` 的前缀展不开路径，原来也是展开后发现为空就跳过，现在直接跳过，不查缓存也不展开。

只在模糊音规则不为 0 时走得到，关着模糊音的全拼、九键、14 键、双拼不受影响；双拼的模糊合并走同一个 `fuzzy_candidates`。

## Alternatives considered

- **把 `retain_unique_sorted_rows` 的 SipHash 换成 FxHash**：只改成员判断、不改顺序，输出一定不变，采样里约占刷新的 5%。`rustc-hash` 已经以传递依赖出现在 `Cargo.lock` 里，但作为引擎的直接依赖仍是新增依赖，要维护者点头，这次不做。
- **把整页投影成 `Vec<WordItem>` 存进缓存**：命中时直接 `extend(cached.iter().cloned())`，代码更短。不选：`WordItem` 每行比 `FuzzyRow` 多出读音、来源、`sentence_words` 等十来个字段，行缓存最多 512 项、每项上百行，常驻内存会明显变大；`FuzzyRow` 每行 24 字节，读音每页只存一份。
- **键直接用 `min(limit, P)`**：与现在的归一分出同样的等价类。不选：现有测试和[上一篇](2026-11-02-fuzzy-row-cache-path-count.md)的辅助函数大多按上限 64 查缓存，未截断时记作 64 与它们一致，改动面小。
- **按候选子集从更大上限的页里筛出小上限的行**：截断后的路径是完整路径的前缀，看起来能复用更多页。不选：每张表 `ORDER BY weight DESC LIMIT 128` 截断，同权重行的先后由 SQLite 决定，子集查询与超集筛选不保证逐行相同。
- **加大 `ROW_CACHE_CAPACITY`**：一轮里约两成没命中是淘汰后重查。不选：每项最多上百行，iOS 键盘扩展的内存上限紧，容量要另行评估。

## Consequences

收益（本机 M4 Pro，`/Applications/MSIME.app` 的桌面词库，release，`ninekey_latency` 30 轮，模糊音全开，按「旧、新、新、旧」各跑一次，1 分钟负载约 4.5；旧是 develop `35f3a409ee`），所有数字键：

| 整句 | p50 旧 → 新 | p95 旧 → 新 | 第 47 位 p50 旧 → 新 |
|---|---|---|---|
| 标准 | 2.66、2.68 → 2.46、2.50 ms | 6.40、6.50 → 5.97、6.12 ms | 6.07、6.35 → 6.09、6.20 ms |
| 关闭 | 1.57、1.61 → 1.43、1.44 ms | 3.30、3.36 → 3.14、3.15 ms | 4.88、4.94 → 4.86、4.88 ms |

p50 降 7% 到 10%，p95 降 5% 到 7%。同一负载下采样，`fuzzy_candidates` 占刷新的比例从 24.1% 降到 18.3%；投影的 `WordItem::new` 和整条候选缓存淘汰时的释放几乎消失。剩下的大头是没命中时的查库（约占 `fuzzy_candidates` 的一半，归一只让它少了约一成）和展开路径，以及系列缓存淘汰时释放整页候选（约占刷新的 11%，与模糊音无关的通用开销）。

代价：每行常驻内存由 `DictRow` 的 56 字节变成 `FuzzyRow` 的 24 字节加两个 `Arc` 头（各 16 字节计数加 24 字节 `String`），净多约 24 字节；冷页的空页多一次申请（读音包一个 `Arc`）。返回的候选与缓存共用字符串，整条候选缓存和返回的候选里的模糊行不再各持一份副本，所以总占用应当不升反降，但没有量过内存，也没有在真机上量过延迟。

输出不变的证据：同一份资源（本机 `/Applications/MSIME.app` 带的桌面词库，按实际哈希写了一份锁副本，两臂共用）、同一个评测程序，引擎分别取 develop `35f3a409ee` 和本改动，`convert_eval --fuzzy-all` 的 words、sentences、nine-key、nine-key-prefix、nine-key-sentences、fourteen-key、fourteen-key-prefix、fourteen-key-sentences 八组 `--report` 与 `--dump` 逐字节相同。对照：同一程序不带 `--fuzzy-all` 打 nine-key，2791 条用例里有 1953 条的候选与带它时不同，说明模糊音确实开着。

测试：

- `unbounded_prefixes_share_one_row_cache_entry_across_budgets`：zan'lan'fan 规则全开时轮到 zan 只剩 42 的预算，归一以前 zan 记在上限 42 下、单独打 zan 还要再查库；现在是同一项，候选与钉住的旧实现逐行相同。
- `fuzzy_rows_share_text_with_the_row_cache_page`：冷页、热页的每个候选都与缓存页共用三个字段，改写返回的候选不影响缓存。
- `beam_result_depends_only_on_the_smaller_of_limit_and_combinations`：归一依据的性质。
- 按申请次数断言的测试：钉住的旧实现读写缓存页时按旧类型逐行转换（`legacy_dict_rows`），命中页每行与旧实现差 6 次申请，冷页本改动每行省 4 次、少一次读音键，旧实现为了把行存成 `FuzzyRow` 每行多两个 `Arc`、读音键多一个，合起来也是每行 6 次（`SHARED_ROW_SAVINGS`）。`fuzzy_candidate_rows_reuse_the_first_typed_reading_string` 从 13 次降到 7 次。`supplied_owned_typed_keeps_first_row_capacity_and_cold_cache_capacity` 原来断言首行接管调用方 256 字节容量的读音，改为 `supplied_owned_typed_is_copied_exactly_and_shared_by_rows_and_cache`：缓存按长度复制，各行与缓存共用它。

前三项新测试和计数测试在把实现退回（键上限用实际上限、投影改回逐字段复制）后失败，恢复后通过。

这篇部分取代[模糊前缀延迟拥有读法](2026-10-14-quanpin-fuzzy-typed-prefix-borrow.md)里「首行接管读法、其余行复制」和[行缓存投影](2026-10-14-quanpin-row-cache-projection.md)里模糊行缓存的投影方式，那两篇的其余决定不变。
