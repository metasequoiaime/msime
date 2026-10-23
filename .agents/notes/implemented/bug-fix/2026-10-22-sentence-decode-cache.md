# Agent Note: 全拼词典按音节缓存整句解码结果

Status: implemented

## Problem

iPhone XR 上九键每个数字仍要约 30 ms（打开词库映射之后，见 [词库映射](2026-10-22-dictionary-mmap-for-slow-devices.md)）。用 `ninekey_latency` 在 Studio 上统计一轮输入（你好、我们、今天天气、很好、今天天气很好）里每次建图的音节串：每个数字要建图解码 19.3 次，平均 4.1 个音节；其中 32.4% 与这一轮里更早解过的音节串完全相同，65.6% 只比解过的音节串多一个音节。

完全相同的那部分来自 `merge_alternatives`：九键每条路径查询时，这串拼音的其他备选切分也各查一遍并组句，同一条备选切分挂在好几条路径下；`series_cache` 按顶层查询键缓存最终答案，管不到这些内部组句，所以同一串音节被反复建图、解码。

## Decision

`QuanpinDictionary` 增加 `sentence_decode_cache`（`SENTENCE_DECODE_CACHE_CAPACITY` 条，FIFO），键由 `sentence_decode_key` 组成：音节串、这次查询的纠错类型、是否规划纠错整句。值是 `DecodedSentences`（未截断的整句路径与纠错整句）以及解码时的个人模型版本。

命中后由[输出投影](../testing/2026-10-23-sentence-decode-cache-projection.md)借用解码路径，只复制最终入选的候选；冷解码仍保留本篇的一份缓存副本和原拥有路径输出。

`lattice::merge` 把原来的建图、解码、合并拆成两步：`decode_lattice` / `decode_lattice_borrowed` 只依赖音节、词典行和解码参数，`merge_decoded` 负责 `emit` 截断、神经重排、去重与插入位置。`merge_lattice_candidates` 的签名不变，双拼等其他调用方照旧；原来只供全拼使用的 `merge_lattice_candidates_borrowed` 由这两步替代。

缓存失效规则（must）：

- 词典行变化：随 `clear_row_caches` 清空（数据版本变化、`reset_cache` 都经过这里，`set_sentence_association` 走 `reset_cache`）。
- 个人模型：条目记下解码时 `PersonalNgramStore::version()`，取用时版本不同即视为未命中。版本在 `personal.model()` 刷新之后读。
- `set_sentence_alternatives` 改变 `nbest`/路径集合：清空。
- 纠错资料换代（`typo_generation` 变化）：清空。
- 纠错类型、是否规划纠错整句：进键，不清空。
- 神经重排只读解码路径、不改变它们，重排上下文不进键；搭配模型随 `set_sentence_association` 清空。

## Alternatives considered

- **增量解码**（缓存束搜索每一列的状态，前缀解过的列直接接着算）：能覆盖那 65.6% 的「多一个音节」，但要改解码器的列状态与末列宽度处理，正确性需要逐路径等价证明；本次先拿完全重复这一块。
- **调大 `series_cache`**：同样能挡住一部分重复，但它的条目是整页 `WordItem`，键盘扩展内存受限；而且内部组句并不经过它的槽。解码缓存每条只存几条句子。
- **英文九键候选按数字串缓存**：采样里占约 9%，但真实输入每个数字都是新的数字串，只有退格重打才会命中；根本解法是给英文词库加预先算好的九键码列并建索引，要改 `dict-builder` 与词库发版，单独评估。
- **长词前缀的空探测剪枝**（每个数字约 54 条几乎全空的 `LIMIT 3` 探测）：路径数减半后探测随之减半，剩余收益估计不足 5%，暂不做。

## Consequences

收益：每个数字的建图解码从 19.3 次降到 13.1 次，完全重复降到 0；`ninekey_latency` 整轮执行的指令数从约 606 亿条降到约 579 亿条（-4.5%，含启动时加载词库和模型的固定开销，按数字计约 -7% 到 -8%）。比预想小。采样显示剩下的耗时主要在候选整页的复制与释放（采样约 30%–40%）、余下解码（约 20%）和 SQLite（约 18%）。

代价：多一个最多 512 条的 FIFO；每条克隆一次解码结果。失效点分散在五处 setter 与缓存清理路径上，以后新增影响解码的状态（新的 n 元表、新的打分项）必须在这里接上清空或进键。

关联：[替代合并直接消费热主候选行](../testing/2026-10-16-quanpin-alternative-row-stream.md) 处理同一个 `merge_alternatives` 里的整页克隆，两者互不替代。

## Testing

`sentence_decode_cache_tests` 覆盖：去掉 `series_cache` 后同一读法从解码缓存重新合并、结果逐行相同且不新增条目；个人模型版本变化后重新解码并记下新版本；`set_sentence_alternatives`、`set_sentence_association` 清空缓存。`cargo test -p msime-engine --lib` 2472 项通过。`convert_eval` 的九键、九键长句、14 键、14 键长句四组都在基线上；全键盘 `words`、`sentences` 两组在本改动前的 develop 上就已与提交的基线不一致，改动前后两次的完整评测输出逐字相同（只有日志里的镜像目录不同），神经重排开着。

## Related experiments

[跨解码静态列评分缓存](../../rejected/testing/2026-10-23-lattice-column-score-cache.md)尝试复用不同图中的原始二元分，未取得可采用的性能证据，候选撤回；本篇的完整音节解码缓存及其失效规则保持。
