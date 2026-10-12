# Agent Note: 社区资源共用评分与删除的 JSON 契约

Status: implemented

## Problem

社区皮肤、候选皮肤和插件 API 各自声明相同的评分请求、评分回包及删除回包结构。三套回包都启用 `deny_unknown_fields`，但复制的定义以后可能在字段或反序列化规则上漂移。

## Decision

`crates/client-core/src/community.rs` 持有 `CommunityRatingRequest`、`CommunityRatingResponse` 和 `CommunityDeletionResponse`。三个资源 API 继续使用自己的 URL、鉴权、入参检查和错误映射，只复用 JSON 字段形状及严格反序列化规则。评分回包仍须与请求星数一致，删除回包仍须为 `deleted: true`。

## Alternatives considered

- 保留每个 API 的局部结构，可以在各资源将来分化时直接修改；目前三者字段与严格解析规则完全相同，继续复制会增加无意分化的机会。
- 连 HTTP 评分和删除流程也抽成泛型方法，可进一步减少重复代码；但各 API 的路径和接口职责独立，本次只合并确定等价的 wire 契约，避免把传输流程绑在一起。

## Consequences

三类社区资源的评分和删除回包共用一份字段定义。新增的类型只在 `client-core` 内部可见，公开 API 与网络格式不变；各资源现有测试继续覆盖请求和回包行为。

后续的[社区写操作共用流程](2026-10-12-community-write-actions.md)接管路径拼接、入参和回包核对；本篇关于 JSON 字段与严格反序列化的决定仍成立。
