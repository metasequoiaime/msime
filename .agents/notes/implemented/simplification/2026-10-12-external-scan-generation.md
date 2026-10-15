# Agent Note: 外部皮肤扫描复用异步请求代次

Status: implemented

## Problem

外部皮肤预览扫描目录时使用局部 `let current = true` 和 effect cleanup 保护异步结果。同一文件的主题解析已经使用共享 `useAsyncGeneration`，扫描请求仍保留另一套生命周期守卫。

## Decision

扫描请求使用 `useAsyncGeneration(scan, key, active, id)`，成功和失败回调只接受当前请求代次。扫描触发条件、刷新按钮、失败状态和预览结果保持不变。

## Alternatives considered

- 保留局部布尔值：行为可行，但重复异步隔离逻辑。
- 与主题解析合并成一个代次：两个请求的生命周期和刷新条件不同，合并会让一个请求的变化取消另一个请求。

## Consequences

外部皮肤扫描与主题解析共享相同的代次守卫模式，旧目录扫描结果不能覆盖新预览。源码契约测试覆盖扫描和主题两处调用。

图片读取后的浏览器解码错误由[图片解码代次](../bug-fix/2026-10-16-skin-preview-decode-generation.md)单独按读取结果归属，不由扫描响应代次清空。
