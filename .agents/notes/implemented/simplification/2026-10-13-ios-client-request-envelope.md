# Agent Note: iOS 公告与使用统计共用客户端请求封装

Status: implemented

## Problem

公告和匿名使用统计各自把字典编码为 JSON、调用 client-core C 接口、释放返回字符串，再解析 `{ok, value}` 响应。两份实现相同，失败时也都返回 nil。以后修改这条调用边界时容易只更新一处。

## Decision

在 SharedUI 增加 `ClientRequestEnvelope.call`，统一请求编码、C 字符串释放和成功响应取值。公告与使用统计仍各自声明入口函数、组成领域请求，并自行决定 nil 的处理方式。

## Alternatives considered

- 让公告直接调用使用统计里的私有函数：会制造无关领域之间的依赖。
- 把所有 client-core 响应调用一次迁完：其他入口还需要错误码或不同返回形状，范围过大且会改变错误语义。

## Consequences

这两个返回 `{ok, value}` 且失败时忽略的入口共用一处实现。公告和使用统计现有行为不变；合成响应测试核对请求编码、成功值、失败和无效 JSON。
