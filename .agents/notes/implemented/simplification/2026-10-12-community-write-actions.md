# Agent Note: 社区评分和删除共用写操作流程

Status: implemented

## Problem

社区皮肤、候选皮肤和插件的评分与删除 API 除资源路径外，逐项重复 ID 与星数检查、路径拼接、评分回包匹配和删除成功标志检查。[先前的 wire 类型共用](2026-10-12-community-rating-delete-wire-types.md)只统一了 JSON 形状，这些操作规则仍可能独立漂移。

## Decision

`community::write_actions` 接收每种资源的路径前缀和一个发送请求的闭包，共用评分与删除的入参、路径和回包核对。各资源 API 保留自己的公开方法、令牌传递和 `self.json` 调用；闭包只负责发出相应的 PUT 或 DELETE 请求。评分回包的星数不符、删除回包为 `false` 时仍返回 `AccountError::Unavailable`，无效 ID 或星数仍返回 `AccountError::Invalid`。

## Alternatives considered

- 延续局部实现，每个 API 完全独立，便于以后分化；目前三者的处理步骤相同，重复维护已超过 wire 类型本身。
- 把 `self.json` 和资源路径也封进一个通用传输对象，可进一步压缩调用处；但各 API 的鉴权和请求类型仍由原接口拥有，传入发送闭包使共用范围停留在确定等价的流程。

## Consequences

三类资源的评分和删除规则集中维护，调用处只给出路径前缀和实际请求。公开方法签名与网络格式不变；新增一次闭包调用，现有社区传输测试继续核对 URL、请求、无效参数和回包结果。
