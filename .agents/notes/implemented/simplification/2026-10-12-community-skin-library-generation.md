# Agent Note: 社区皮肤库读取复用异步请求代次

Status: implemented

## Problem

社区皮肤页读取本地皮肤库时同时手写了局部 `let current` cleanup 和手动 `libraryGeneration`。手动代次用于保护“获取”完成后新写入的皮肤不被更早的读取覆盖，但局部布尔值只是重复处理视图切换和卸载。

## Decision

使用 `useAsyncGeneration(harmony, browsing, localSkinLibrary)` 处理加载请求的生命周期，并保留 `libraryGeneration` 处理本地库变更与读取之间的业务顺序。两种代次各自负责独立约束，不合并它们。

## Alternatives considered

- 删除手动 `libraryGeneration`：会让较早的读取覆盖刚完成的获取结果。
- 保留局部布尔值：行为可行，但继续复制共享异步生命周期守卫。

## Consequences

社区皮肤页在关闭网格、切换客户端或卸载时忽略旧读取，同时保留获取结果优先级。社区皮肤相关测试和新增源码契约测试覆盖该抽取。
