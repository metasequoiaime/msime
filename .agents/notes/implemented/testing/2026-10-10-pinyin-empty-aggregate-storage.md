# Agent Note: 拼音聚合空页延后行存储预留

Status: implemented

## Problem

`query_longer_phrases` 和 `query_exact_segmentations_keyed_flat` 在各表查询之前按额外音节数或切分数乘 limit 预留聚合行容器。底层空页已不再预留，但全部表未命中、缺表或批次无有效键时，上层仍保留一整个空聚合容器。

## Decision

两个聚合向量从零容量开始，第一张非空页到来后先按原提示 `reserve_exact` 再执行原 `extend`。保留长词续接的首次词值去重、稳定权重排序和截断，以及精确分段的按表顺序、稳定排序和同值行保留。只改变聚合存储的申请时间，不改变底层查询或容量公式。

## 既有笔记审计

[底层空页预留](../../implemented/testing/2026-10-10-pinyin-empty-row-storage.md)与本片部分重叠于延后预留和冷测量边界，本片只计聚合层独立收益，互链保留。[长词去重](../../implemented/testing/2026-10-07-pinyin-longer-phrase-dedup.md)、[切分键去重](../../implemented/testing/2026-10-08-pinyin-segmentation-key-dedup.md)和[表键归并](../../implemented/testing/2026-10-09-pinyin-table-key-buffer.md)分别约束结果去重与查询规划，不持有聚合容器的预留决定。活跃树检索上述函数、聚合空页和延后预留，没有相同决定。简拼入口仍有开放排序 PR，本片不改简拼聚合。

## Alternatives considered

- 保留立即预留：命中路径不需分支，但所有页都为空时仍分配聚合容器。
- 直接接管第一张页的向量：可以减少命中分配，但容量从每层或每表提示变为单页容量，后续页可能扩容，无法保持原容量和分配次数。
- 所有页自然增长：空页不占存储，但密集命中会增加扩容；延后原提示更容易逐字段与次数对照。

## Verification

旧生产上的两个公开查询空页测试真实失败，limit1000 下长词续接返回容量3000，精确分段返回容量1000，均期望零。固定基线 `96fd6b7f9ca636078250e5cc1b8e528b7f4986a9` 的完整聚合正文对照，底层两侧都使用已合并的延后预留 helper。覆盖跨表、重复值/键、等权排序、先空后命中、密集结果和非法输入；命中字段、顺序、容量和次数相等，进入旧聚合预留的全空查询少一次。冷计量在区间外显式预热进程级不可变音节表，再包含数据库打开、查询、显式析构，只返结果并借用外部 fixture/path。7 项新测试通过；完整引擎 1523 项单测通过（10 项忽略）、golden 31 项通过，Clippy `--all-targets -- -D warnings` 通过。独立审查指出未预热全局音节表会让冷测试依赖其他用例先运行；单独运行真实失败，旧返回存储176712字节而非168000字节。修正为测量前显式检查完整音节以预热后，单独冷测试通过且实测数值保持一致。格式、差异和 361 篇笔记校验通过；完整 quick 通过共享 Rust、Android target/Java、Linux 外壳和原生宿主（73 项测试）、macOS 原生宿主与 Apple bridge，Windows 本机102项通过。完整 Windows 交叉编译、wasm、HarmonyOS 真打包因工具链/资源缺失跳过。修正后的冷边界独立复审无阻断。同步到包含日语矩阵优化的 `f9e119e28f5db22a451c6457604a09dbf8913bb8` 后，拼音四个文件与 Cargo 配置不变；组合状态完整引擎1526项通过（11项忽略）、golden31项通过，Clippy再次通过。

当前 arm64 debug 的完整冷边界：长词续接分配38→37次，Rust 逻辑请求峰值169165→1165字节，返回存储168000→0字节；两切分精确查询36→35次，峰值113603→1603字节，返回存储112000→0字节。两侧最低 signed 差值均为零，新 limit9999 与 limit1000 同位数，峰值相等且返回零存储。底层两侧都使用已合并的新 rows helper，只计聚合层的独立收益。未做发布构建计时或真实宿主延迟验收。

## Consequences

全部页为空时省去一次聚合容器分配，返回零容量；命中仍按原容量提示预留一次。每页增加非空及容量分支，尚不保证命中延迟改善。原乘积容量公式和非空时的大提示风险保持原状；不把此片收益当作所有拼音查询零分配，不宣称 SQLite C 堆或 RSS 收益。[Vec::reserve_exact 官方契约](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.reserve_exact)保证至少所需容量，不保证物理分配精确大小。

按键分组查询的空结果 `HashMap` 由[结果表延后预留](../../implemented/testing/2026-10-10-pinyin-empty-key-map-storage.md)单独处理；两侧共用已合并的底层查询函数，结果表收益独立计量。
