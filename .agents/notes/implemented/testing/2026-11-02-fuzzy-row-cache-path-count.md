# Agent Note: 模糊行缓存命中不再展开路径

Status: implemented

## Problem

#10457 的报告者开着全部 11 条模糊音规则打九键。`ninekey_latency` 在 Mac Studio 上量出来，模糊音全开时每键比关着慢 1.5 到 1.9 倍（数字见 [Android 按键路径拆段计时](../feature/2026-11-02-android-keystroke-split-timing.md)）。macOS `sample` 采样（整句关、九键、400 轮）显示，模糊音多出来的时间主要在 `QuanpinDictionary::query` 的模糊合并里：`fuzzy_candidates` 约占 `query` 的 39%，其中查库 `query_exact_segmentations_keyed_flat` 约 35%，展开模糊路径 `fuzzy_segmentations` 约 29%；合并后按词面去重的 `retain_unique_sorted_rows`（SipHash 的 `HashSet<&str>`）约占 `query` 的 11%。

`fuzzy_candidates` 对每个前缀都先展开模糊路径，再去 `fuzzy_row_cache` 查这一前缀的行。展开只是为了两件事：没命中时拿路径去查库，以及按路径条数扣 `FUZZY_PATH_BUDGET`。九键每键要走二三十条切分，这些切分的前缀大多在前几键已经查过，行缓存命中时展开出来的路径只用来数条数，随即丢掉。

## Decision

`CachedFuzzyRows` 记下这一项查过的路径条数 `path_count`。`fuzzy_candidates` 对每个前缀先查行缓存（`append_cached_fuzzy_rows`）：命中时把行投影进结果，按 `path_count` 扣预算，不再展开路径；没命中时它把读音原样交回，照旧展开路径，路径为空就跳过（不入缓存，与原来相同），否则由 `append_queried_fuzzy_rows` 查库、连同 `paths.len()` 一起入缓存。

这样做输出不变，依据是行缓存的键已经定下了路径：键是规则、上限和这段读音（`typed`），而这段读音按 `'` 切开就是前缀音节，`fuzzy_segmentations` 只由前缀音节、规则和上限决定。所以命中项记下的条数就是这次展开会得到的条数，预算在每个前缀上扣得和原来一样，预算在哪个前缀上用完也一样。哈希碰撞仍由完整键核对挡住，按没命中处理。

只在模糊音规则不为 0 时走得到（`fuzzy_candidates` 一开头就对 `rules == 0` 返回空），所以关着模糊音的全拼、九键、14 键和双拼都不受影响。

## Alternatives considered

- **精确部分改走 `query_exact_projected` 的借用路径，让九键的 `filter_dominated` 在模糊音下也生效**（triage 的原计划）：采样里模糊路径 `query_exact` 的整页复制加缓存淘汰，和关模糊音时借用路径的 `clone_rows_reusing` 按同样的查询量折算差不多，省不下多少；`filter_dominated` 在模糊音下作用于已经拥有的行，只省 `push_ranked` 之前的一次比较。又要守住「模糊合并的去重和站位先完成，被筛掉的词不能让另一条同文行复活」，改动面大、收益小，不做。
- **去重换成更快的哈希**：`retain_unique_sorted_rows` 的 SipHash 约占 `query` 的 11%，换成 FxHash/foldhash 一类哈希只改成员判断、不改顺序，输出一定不变。但引擎目前没有这类依赖，加依赖要维护者决定，这次不做，留作后续。
- **整条模糊候选缓存（`fuzzy_cache`）命中时不再复制结果**：要把 `Vec<WordItem>` 改成共享的，后面的合并还要拿到拥有的行，改动牵涉合并的所有权，这次不做。

## Consequences

收益：Mac Studio（M1 Max，`/Applications/MSIME.app` 的桌面词库，release，`ninekey_latency` 30 轮去掉预热，模糊音全开，按「旧、新、新、旧」各跑一次，1 分钟负载 4.7–6.9）上，所有数字键：

| 整句 | p50 旧 → 新 | p95 旧 → 新 | 第 47 位 p50 旧 → 新 |
|---|---|---|---|
| 标准 | 4.04、4.06 → 3.92、3.90 ms | 9.60、9.49 → 8.91、8.95 ms | 9.48、9.47 → 8.34、8.33 ms |
| 关闭 | 2.41、2.40 → 2.33、2.34 ms | 5.62、5.61 → 4.81、4.85 ms | 7.97、7.98 → 6.90、6.89 ms |

p50 降约 3%，p95 降 6% 到 14%，长组字降约 12%。离模糊音关着的水平（整句标准 p95 约 6.3 ms、关闭约 2.9 ms）还远，剩下的大头是模糊路径本身的查库和合并去重。

代价：行缓存每项多存一个 `usize`。

输出不变的证据：`convert_eval` 加了 `--fuzzy-all`（打开全部 11 条规则，只支持全拼），用同一份资源、同一个评测程序，引擎分别取 develop `7f262baea8` 和本改动，nine-key、nine-key-prefix、nine-key-sentences、fourteen-key、fourteen-key-prefix、fourteen-key-sentences 六组和 26 键的 words、sentences 两组的 `--report` 与 `--dump`（每条用例的全部候选文字和来源）逐字节相同。`typing_sim` 用的是出厂配置，模糊音关着，走不到这条路径，没有另跑。

测试：新增 `row_cache_hits_spend_the_recorded_path_budget`，规则全开、预算在长读音中途用完，先查短读音热起前缀页再查长读音，结果与每个前缀都展开路径的旧实现逐行相同，预算用完后的单音节前缀照样不查。行缓存命中时少了展开路径的申请，几处按申请次数断言的测试改为在旧的预期上加上 `skipped_path_allocations` 算出的那部分（按查询同样的预算走一遍前缀，把行缓存已有的前缀展开路径的申请次数加起来）；`fuzzy_candidate_rows_reuse_the_first_typed_reading_string` 的 `zong'guo` 从 25 次降到 13 次，其中 12 次是两个前缀展开路径。

这篇延伸[全拼模糊行缓存借用查询键](2026-10-11-fuzzy-row-cache-key-borrow.md)：那篇让命中时不申请查询键，这篇让命中时连路径也不展开。
