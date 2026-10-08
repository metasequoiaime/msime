# Agent Note: 复用粤拼活动方案查询状态

Status: implemented

## Problem

粤拼会话刷新已经从活动方案构造请求，但 provider 仍按 `request.raw_input` 再把输入写入 registry 自己持有的 `CantoneseScheme`，逐键刷新重复执行输入过滤并保留一份重复的组字状态。

## Decision

让 `Scheme` 暴露只读的活动 `CantoneseScheme`，并为 registry 增加直接接收活动方案的查询入口。会话刷新时直接用活动方案查询候选；保留请求查询入口供原始请求路径使用，并共用候选行写回逻辑。

## Verification

- 粤拼候选的文字、顺序、`pinyin`、`canonical_pinyin`、权重和来源保持不变。
- 逐键刷新继续复用候选行存储，并避免再次规范化活动输入。
- 使用合成词库逐项对照完整输入、前缀补全、分隔符、非法尾部与清空后的完整 `WordItem`；备用方案尚未分配输入存储时，直接查询比请求查询少一次输入分配。短输入预热后的逐键刷新分配数保持 14 次，分段仍由原查询逻辑执行。
- 粤拼相关单测、格式、Clippy、笔记检查和 quick 门禁通过。

## Alternatives considered

继续让 registry 从请求重建粤拼方案会重复过滤输入；只保留 registry 自己的方案状态则无法复用活动方案已经持有的规范化结果。

## Consequences

活动方案和请求的状态必须来自同一次刷新；只有 `request.scheme` 为粤拼时才能走直接入口。候选查询入口共用写回缓冲，通用请求路径仍保留原有行为。活动刷新省去重复输入过滤和复制，不缓存分段；备用方案继续供光标前缀等通用请求使用。
