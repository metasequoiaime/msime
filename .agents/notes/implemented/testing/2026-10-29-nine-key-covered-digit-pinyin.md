# Agent Note: 九键按切分查出的候选行共用覆盖数字串

Status: implemented

## Problem

九键每按一个数字，`NineKeySession::refresh` 要把二三十条切分各查一遍词库，按 Pixel 那次的按键序列（`nks` + 聊天子句，3691 次按键）平均每键往候选表里推 6554 行（测量用探针计数，未入库）。每推一行都把这一行的 `pinyin` 换成它覆盖的那段数字：`SharedString::from(&self.digits[..covered])`，也就是每行申请一份新的 `Arc<String>`，截断到 128 行时再逐行释放。macOS 采样里这次申请（`SharedString::from`）约占主线程 2%，同一批串的释放还有一部分算在 `refresh` 里的析构上。Android 的分配器比 macOS 的慢得多：模拟器上 `nk_query` 里除去解码和 SQLite 之外的部分占 5.8 ms（M4 Pro 本机是 1.6 ms），比解码本身放大得多。

## Decision

`refresh` 开始查询之前建一个 `CoveredDigits`，按覆盖长度懒建并缓存 `self.digits[..len]` 的 `SharedString`。按切分查出的行改为 `candidate.pinyin = covered_digits.prefix(covered)`：同样长度的行共用同一份存储，一次刷新只按不同的覆盖长度各申请一次（最多数字个数加一次）。

简拼行和九键双拼的整词行不改：它们本来就是 `clear()` 加 `push_str`，行是查询新建的、独占自己的缓冲区，原地改写不申请；换成共享串反而会释放掉原缓冲区，`initials_rows_reuse_their_existing_pinyin_capacity` 守着这一点。

`pinyin` 的内容与原来逐字节相同；之后若有路径原地改写某一行的 `pinyin`，`SharedString` 按写时复制先复制，结果仍与原来相同。

## Alternatives considered

- **简拼行和双拼整词行也换成共享串**：试过。它们原来不申请，换了之后每行要释放一份原缓冲区，`initials_rows_reuse_their_existing_pinyin_capacity` 失败，所以只改按切分查出的行。
- **把覆盖长度直接存进 `WordItem`、读取时再切片**：能省掉全部申请，但 `pinyin` 是宿主和学习都读的字段（选词后按它消费数字），改字段语义牵涉每个读它的地方，收益和本篇相同量级，不做。

## Consequences

收益（本机 M4 Pro，ABBA 各两次、每次 3 轮计时加 1 轮预热，`warm + nks + chat` 共 21210 个数字键样本；工具是测量用的 `ninekey_arms`，经 C ABI 调 `msime_client_character` 再调 `msime_client_candidate_range(0, 48)`，和 Android 每键的两次调用相同）：

| | 平均 | p50 | p95 | p99 |
|---|---|---|---|---|
| develop `52a78a2fc9` | 3.930 ms | 3.630 ms | 8.259 ms | 10.469 ms |
| 本改动 | 3.604 ms | 3.352 ms | 7.544 ms | 9.385 ms |
| 比值 | 0.917 | 0.924 | 0.913 | |

一行数字之内第 13 位以后的长串按键比值 0.919，前 6 位 0.920。

同一台机器再按 BAAB 跑一组（这时别的会话把 1 分钟负载推到 7–37）：平均 0.955、p50 0.939、p95 0.980。Mac Studio（M1 Max，1 分钟负载 12–54，同时在跑本篇的评测）上 ABBA：平均 0.991、p50 0.981、p95 1.011，方向一致、幅度小得多；Studio 是 CI 机，负载下的计时只看方向。

输出不变：`convert_eval` 的 nine-key、nine-key-prefix、nine-key-sentences、fourteen-key、fourteen-key-prefix、fourteen-key-sentences 六组 `--report` 与 `--dump`（每条用例的全部候选文字和来源），以及 `typing_sim` 主配置（聊天集、九键）的 `--report` 和 `--dump`，与 develop 逐字节相同；唯一的例外是 nine-key 和 fourteen-key 的 dump 里 4 条用例带行内时间行（来源 8），文字是运行时的墙钟，按位置和来源比较相同。

测试：新增 `rows_with_the_same_covered_digits_share_one_pinyin_buffer`，在旧实现上失败（同样覆盖长度的两行各持一份存储），新实现通过。`dominated_rows_are_rejected_before_field_copies` 记的新旧申请次数差额从 777 改为实测的 846。

代价：每次刷新多一个最多 `DIGIT_LIMIT + 1` 项的小向量。`SharedString::shares_storage_with` 改为 `pub(crate)`，供测试核对共享。
