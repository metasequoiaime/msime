# Agent Note: AI 回复模板复用异步请求代次

Status: implemented

## Problem

AI 回复模板列表在异步读取社区内容时手写 `let current = true` 和 effect cleanup。仓库已有 `useAsyncGeneration` 统一管理异步请求的所有者变化与卸载；这里继续保留局部标记会让请求隔离出现两套写法。

## Decision

复用 `useAsyncGeneration(community)`，异步列表成功和失败回调都通过同一请求代次判断，再更新列表状态。读取接口、提示文字和依赖变化行为保持不变。

## Alternatives considered

- 保留组件内的 `let current` 和 cleanup：行为可行，但会继续复制仓库已有的异步请求隔离模式。
- 只使用 `useMountedRef`：可以防止卸载后的更新，但无法挡住社区客户端变化后旧请求的结果。

## Consequences

AI 回复模板组件与其他异步 UI 共用同一请求代次实现，减少局部 cleanup 样板代码。现有社区图库与回复模板集成测试继续覆盖列表和添加流程。
