# Agent Note: 发现词库复用异步请求代次

Status: implemented

## Problem

HarmonyOS 词库页的“发现词库”列表使用局部 `let current = true` 和 effect cleanup 保护异步结果。详情分页和其他设置组件已经使用共享的 `useAsyncGeneration`，继续保留局部模式会让客户端更换时的请求隔离出现两套实现。

## Decision

为发现列表使用 `useAsyncGeneration(community)`，成功和失败回调只有在当前请求代次仍有效时才更新状态。列表上限、错误提示、添加动作和显示行为保持不变。

## Alternatives considered

- 保留局部布尔值：能够处理卸载，但重复实现请求代次管理。
- 只使用挂载 ref：不能在社区客户端变化后阻止旧客户端结果覆盖新列表。

## Consequences

发现词库列表与同页详情加载共用请求代次机制，移除局部 cleanup 样板。现有词库页测试和新增源码契约测试覆盖该抽取。
