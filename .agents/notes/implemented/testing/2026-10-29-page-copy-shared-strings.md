# Agent Note: 冷页入缓存的整页复制共用文本字段

Status: implemented

## Problem

[淘汰页复用](2026-10-23-quanpin-evicted-page-reuse.md)让冷页全保留时，把整页复制进 FIFO 刚淘汰的那一页：每个字符串只在旧容量正好等于新内容长度时就地覆写，否则新建。它和[候选行共享字符串](../architecture/2026-10-23-shared-candidate-strings.md)是同一天的改动；`WordItem` 的文本字段换成 `SharedString` 之后，普通克隆只加引用计数，而这条复制路径仍逐字节复制，容量对不上的字段（几乎全部）各申请一份。九键打字时 FIFO 一直是满的，每个没命中的切分键都走这里。按 Pixel 那次的按键序列，本机采样里 `clone_rows_reusing` 约占主线程 14%，其中复制字符串的 `clone_shared_string_reusing` 占 8%。

## Decision

`page_copy::clone_row_reusing` 的四个 `SharedString` 字段（`pinyin`、`canonical_pinyin`、`word`、`corrected_from`）直接克隆源行的字段，缓存页与返回页共用同一份字符串；淘汰页里这几个字段的旧缓冲区随旧行释放。外层向量和 `sentence_words`（普通 `String`）照旧复用淘汰页里容量恰好的缓冲区。

两页互不影响：任何一边经 `DerefMut` 原地改写共用的字段时，`SharedString` 按写时复制先复制；`clear` 在共用时只放手自己那一份。

## Alternatives considered

- **保留逐字节覆写**：容量恰好相等时零申请，但真实打字里几乎碰不到相等的容量，绝大多数字段走新建；共用字段在任何情况下都零申请。
- **缓存页直接存返回页本身、不复制**：返回页之后会被调用方筛选和改写，缓存需要独立的一份；克隆字段已经只剩引用计数，没有必要再改所有权。

## Consequences

收益（本机 M4 Pro，测量用的 `ninekey_arms` 经 C ABI 每键调 `msime_client_character` 和 `msime_client_candidate_range(0, 48)`；与[上一个提交](2026-10-29-lattice-beam-prefix-resume.md)的二进制 ABBA 各两次、每次 3 轮计时，21210 个数字键样本）：

| | 平均 | p50 | p95 | p99 |
|---|---|---|---|---|
| 上一个提交 | 3.117 ms | 2.979 ms | 6.150 ms | 8.189 ms |
| 本改动 | 2.808 ms | 2.741 ms | 5.384 ms | 7.025 ms |
| 比值 | 0.901 | 0.920 | 0.875 | |

三个改动（[覆盖数字串](2026-10-29-nine-key-covered-digit-pinyin.md)、[束前缀接续](2026-10-29-lattice-beam-prefix-resume.md)、本篇）合计，对 develop `52a78a2fc9`：

- 本机 M4 Pro ABBA（同上口径）：每键平均 4.381 → 3.022 ms（0.690），p50 0.758，p95 8.975 → 5.824 ms（0.649），一行第 13 位以后 0.636。整个进程的 instructions retired 513.6G → 379.9G（−26%），含启动时加载词库。
- Mac Studio ABBA（1 分钟负载 8–29）：平均 6.276 → 4.159 ms（0.663），p95 13.762 → 8.238 ms（0.599）。
- Android 模拟器（API 35 arm64，M4 Pro 上 4 核，只看比例）ABBA，按键到 JNI 返回：平均 12.00 → 8.92 ms（0.743），p95 29.73 → 20.10 ms。
- Pixel 3 XL（Android 12，APK 装后 `cmd package compile -m speed -f`，整句候选「标准」，九键）ABBA，四臂共同打完的 `nks` 前 8 段 3499/3524 次按键：引擎（JNI 调用）平均 55.63 → 47.39 ms（0.852），p95 114.45 → 95.12 ms（0.831）；按键到候选上屏（`perf.jsonl` 的口径）p50 56.86 → 49.03 ms，p95 120.49 → 101.07 ms。真机上引擎仍占按键到上屏的九成。develop 在真机上的引擎分段（测量用探针，未入库）：每键刷新 57.4 ms，其中按切分查询 54.1 ms，查询里整句解码 12.0 ms、建图 1.8 ms、SQLite 7.0 ms，余下约 33 ms 是行的复制、筛选、入表和释放，比例比 M4 Pro 上（查询 3.4 ms 里约 1.6 ms）高得多，下一步的大头在这里。

输出不变：`convert_eval` 九键、14 键、全键盘共十三组 `--report` 与 `--dump`，`typing_sim` 九键和全键盘两组的 `--report` 与 `--dump`，都与 develop 逐字节相同（行内时间行的文字是墙钟，按位置和来源比较）。

测试：`exact_page_copy_reuses_outer_and_nested_buffers_without_allocating` 原来断言 6 次申请、`word` 指针沿用淘汰页，改为 `exact_page_copy_shares_strings_and_reuses_outer_and_nested_buffers_without_allocating`：0 次申请，四个文本字段与源行共用存储，外层向量和 `sentence_words` 仍沿用淘汰页的缓冲区。`page_copy_preserves_all_fields_and_clone_capacities_across_shapes` 在复制之后清空源行字段，核对复制页不受影响，原样通过。

代价：缓存页与返回页共用字符串，被淘汰的旧页字符串改在复制时释放（原来是留着覆写）；字段容量由源行决定，不再等于内容长度，没有测试以此为契约。
