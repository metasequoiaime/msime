# Agent Note: 外部主题解析复用异步请求代次

Status: implemented

## Problem

外部皮肤预览的主题解析 hook 使用局部 `let current = true` 和 effect cleanup 保护异步结果。仓库已经用 `useAsyncGeneration` 表示请求所有者变化与卸载，局部布尔值让相同的异步隔离规则重复存在。

## Decision

在 `useResolvedTheme` 中使用 `useAsyncGeneration(resolve, key)`，解析成功时只接受当前代次的结果。主题请求、失败回退和预览渲染保持不变；同文件中扫描外部皮肤的另一处请求不在本切片范围内。

## Alternatives considered

- 保留局部布尔值：行为可行，但继续复制异步生命周期守卫。
- 把主题解析和皮肤扫描同时改造：会把两个不同请求的行为变化堆进同一切片，降低可独立验证性。

## Consequences

主题解析与其他异步 UI 共用请求代次机制，旧主题结果不会覆盖新请求。新增源码契约测试验证共享 hook 的使用。
