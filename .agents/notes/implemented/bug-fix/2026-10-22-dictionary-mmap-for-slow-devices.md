# Agent Note: 词库连接打开内存映射，降低慢设备上九键每键的 SQLite 开销

Status: implemented

## Problem

用户在 iPhone XR（A12，iOS 18.7）上用九键打字明显卡，iPhone 17 上不卡。在 XR 上给键盘按键路径加 `os_signpost` 区段，用 Instruments 录 90 秒的 Time Profiler 与区段数据（iPhone Use 在备忘录里按「你好、我们、今天天气、很好」逐键点按），九键每个数字在输入线程上的引擎耗时为 15–72 ms，中位数 39 ms，随组字变长而增长；写宿主组字（`setMarkedText`）与宿主回调合计不到 1 ms，主线程重画 2–17 ms。引擎这一段的采样里，`msime_client_character` 有 44% 落在 SQLite 逐行读取（`rusqlite` `Rows::advance` → 系统 `sqlite3_step`），其中一半多的栈顶在内核系统调用里、线程状态为 Running，另有 SQLite 申请内存页和清零新页的样本：页缓存在被反复重建，同样的页被反复拷入。

同一份代码在 Mac 上 SQLite 只占九键每键耗时的约 15%。按查询类型统计，九键每按一个数字平均发出约 107 条 SQL（长词前缀探测 54.5 条、不限行数的范围查询 28.8 条、精确键查询 19.9 条、`IN` 批量 3.2 条）。主词库 `msime-pinyin.db` 约 74 MB，而连接没有设置任何 PRAGMA，SQLite 默认页缓存约 2 MiB；Mac 的系统文件缓存让每次 `pread` 很便宜，XR 上则很贵。

## Decision

`PinyinDatabase` 的连接在 `open_connection` 里设置 `PRAGMA mmap_size = DICTIONARY_MMAP_SIZE`（128 MiB）。查询直接读映射的文件页，不再经 `pread` 拷进页缓存。设置失败时忽略，连接照常打开，退回 `pread`：不支持映射的 SQLite 也必须能用词库。

安全前提：

- 运行时从不收缩词库文件。VACUUM 只出现在离线的 `dict-builder`；学习写入只让文件变长，SQLite 自己扩展映射。
- 词库更新按内容哈希装进新目录（`ResourceStore`），运行时唯一会删的目录是中途放弃的 `incoming-` 暂存目录，正被映射的词库文件不会被删除或覆盖。Windows 上被映射的文件不能删改，这一条因此也成立。
- 映射的是干净的文件页，iOS 不计入键盘扩展的 `phys_footprint` 上限。

`crates/engine/examples/ninekey_latency.rs` 按上面那串数字逐键计时（`character` 加 `snapshot`），按数字在组字里的位置给出分布，供以后回归测量。

## Measurements

iPhone XR，同一安装方式、同一按键序列，打点版本与 develop 同步，只差这一行设置：

| | 每个数字引擎耗时 p50 | p90 | 平均 | 样本 |
| --- | --- | --- | --- | --- |
| 未映射 | 39.2 ms | 62.4 ms | 40.6 ms | 20 |
| 映射 128 MiB | 30.6 ms | 53.4 ms | 31.7 ms | 34 |

映射后 SQLite 在九键耗时里的占比从 44.0% 降到 38.4%，其中内核部分从 23.4% 降到 14.9%；剩下的内核时间主要是映射页的缺页。Studio（M 系列）上 `ninekey_latency` 的 p50 在 2.8–3.0 ms 之间，与未映射时差别落在噪声内，符合预期：Mac 上 SQLite 本来就只占一小部分。

两次 XR 录制的样本数不同（自动点按的节奏不同），按数字归并的组字长度分布不完全相同；结论只取中位数和平均值的方向和幅度，不把 22% 当成精确值。

## Alternatives considered

- **调大 SQLite 页缓存（`cache_size`）**：同样能留住常用页，而且不依赖映射。但页缓存是匿名脏内存，计入键盘扩展的内存上限；74 MB 词库要装下常用部分得要几十 MB，会把扩展推向被系统结束。映射页是干净的文件页，不占这份额度。
- **减少每键查询数**（按音节前缀记住「这张表里没有以此开头的键」、在慢设备上收紧九键候选路径数、增量刷新而不是每键整段重查）：能动到剩下的大头，但牵涉九键刷新结构和候选质量，本次不做，留作后续单独评估。仓库里已有多轮九键微优化被否决，见 [冷页投影](../../rejected/bug-fix/2026-10-19-nine-key-cold-ranked-projection.md)，改动刷新结构必须先有真机数据。
- **把词库整体读进内存**：查询最快，但 74 MB 远超键盘扩展可用内存，不可行。

## Consequences

收益：慢设备上九键每个数字的引擎耗时中位数降约 22%。桌面、Android、HarmonyOS 共用同一连接设置，也不再经 `pread` 拷页，但这些平台没有单独测量。

代价与边界：

- 每个词库连接多占最多 128 MiB 虚拟地址空间，实际驻留取决于读到的页。
- 剩下的耗时主要在整句合并与词网格解码（`merge_sentences_with_typos` 约 24%、`decode_graph` 约 14%）和每键约 107 条查询本身，XR 上每个数字仍要 30 ms 左右，这次没有解决「卡」，只是去掉了与查询数量无关的那部分页拷贝。
- 主线程上每 2 秒一次的个人词库维护（`dictionaryMaintenance`，XR 上 7–22 ms）与键盘出现时一次 45 ms 的同步等待属于 iOS 宿主，不在本篇范围。

相关：iOS 宿主侧的耗时决定见 [主线程绘制预算](2026-10-18-ios-keystroke-render-budget.md) 与 [九键会话线程](2026-10-19-ios-nine-key-owned-thread.md)；同样映射只读大文件的先例见 [八元语法模型](../feature/2026-10-21-octagram-grammar-model.md)。

## Testing

`dictionary_connections_map_the_file` 核对连接上的 `mmap_size` 等于 `DICTIONARY_MMAP_SIZE`，并确认映射后查询结果不变。`cargo test -p msime-engine --lib dictionary::pinyin` 138 项通过，`cargo clippy -p msime-engine --lib --examples -- -D warnings` 通过。真机数据来自上表两次 XR 录制；没有在 Android 低端机上单独测量。
