# Agent Note: 双拼规范切分直接写入请求缓冲

Status: implemented

## Problem

`ShuangpinScheme::build_request_from_raw_into` 先让双拼转换函数构造拥有型规范切分，再把它复制进 `QueryRequest::normalized_segmentation`。逐键刷新已经持有这个请求字段的可复用存储，临时字符串只为一次复制存在。

已检索双拼请求复用、规范切分转换、大小写回填和请求缓冲笔记。[双拼请求规范键复用](2026-10-09-caret-prefix-request-reuse.md)已负责从规范切分生成 `normalized_input`，[双拼分段转换复用解码结果](2026-10-17-shuangpin-segmentation-decoding.md)已负责逐音节转换不产生短字符串；本片只把转换结果的外层存储交给请求字段，两项决定部分重叠但互不替代。

## Decision

双拼转换新增写入已有 `String` 的内部入口，先清空并按输入上限预留，再把每个片段直接追加到目标缓冲。拥有型 `convert_seg_shuangpin_to_seg_complete_pinyin` 保留为包装入口，独立查询和测试继续取得拥有型结果；请求构造改用写入入口后再复制到 `segmentation`，不再创建规范切分临时字符串。无效输入仍清空全部规范字段，音节解码、撇号、大小写和无效片段行为保持不变。

## Alternatives considered

- **保留临时规范切分并 `clone_from`**：实现最直观，但热刷新每次仍申请并释放一份短命字符串。
- **让 `QueryRequest` 借用转换结果**：可以消除最终复制，但请求需要独立拥有字段，且借用会跨越方案输入和后续查询生命周期，扩大公开结构边界。
- **把转换状态缓存到方案对象**：能复用更多工作，但必须处理原文、方案和请求失效；当前只需复用已经存在的请求缓冲。

## Consequences

预热双拼请求刷新少一次规范切分字符串分配；规范切分、无分隔查询键、原始大小写切分和请求字段所有权保持。拥有型转换入口仍会按自身结果容量返回字符串，新增写入入口只在 crate 内使用。

## Verification

TDD 回归先在旧实现上运行：`NiHc` 热刷新申请 3 次，要求 2 次的测试失败；改用写入入口后同一测试通过并申请 2 次。专项测试还核对 `normalized_segmentation == "ni'hao"`；随后运行双拼方案与工具测试、全引擎和 runtime 测试、Clippy、Rustfmt、`git diff --check`、`pnpm run verify-notes` 及 quick 门禁。
