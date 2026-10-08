# Agent Note: 请求预编辑缓冲复用

Status: implemented

## Problem

`ImeSession::refresh_candidates` 已经构造了包含预编辑显示文本的 `QueryRequest`，随后仍调用 `Scheme::preedit()` 再生成一份相同字符串，按键级刷新会产生重复分配。

## Decision

按方案从请求的 `raw_input_with_cases` 或 `normalized_segmentation` 复制到会话已有的预编辑缓冲，继续保留缓冲容量；刷新路径不再调用统一分派的 `Scheme::preedit()`，该方法仍保留给兼容调用。候选查询、方案切换和提交语义保持不变。

## Alternatives considered

继续调用 `Scheme::preedit()` 会重复构造字符串；为每个方案增加独立的可写预编辑接口会扩大改动面，而请求已经持有完全相同的显示字段。

## Verification

新增请求预编辑缓冲零分配测试；完整 `msime-engine` 单测与 golden、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

连续刷新候选时复用会话预编辑字符串容量，避免为同一显示文本额外分配堆内存。
