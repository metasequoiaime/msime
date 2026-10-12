# Agent Note: 触屏皮肤网格共用异步请求代次

Status: implemented

## Problem

触屏皮肤网格分别加载本地皮肤库和社区首屏，两处都手写 `let current = true` 和 effect cleanup。仓库已有 `useAsyncGeneration`，这些局部守卫重复了客户端变化、视图关闭和组件卸载时的请求隔离逻辑。

## Decision

两项加载各自使用一个 `useAsyncGeneration`，所有者包含网格是否可见和对应客户端。成功与失败回调只在所属代次仍有效时更新列表。保留两项加载的独立生命周期，以及原有失败回退。

## Alternatives considered

- 保留两个局部布尔守卫：行为可行，但继续复制生命周期管理。
- 两项加载共用一个代次：任一客户端变化会取消另一项仍有效的结果，扩大行为变化。

## Consequences

本地设计和社区皮肤首屏分别复用共享请求代次机制；隐藏网格或切换客户端后，旧结果不会写入列表。现有皮肤网格集成测试覆盖列表顺序和点按行为。
