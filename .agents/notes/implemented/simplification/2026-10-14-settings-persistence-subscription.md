# Agent Note: 设置持久化监听复用公共订阅清理

Status: implemented

## Problem

`useSettingsPersistence` 为 `onPreferencesChanged` 手写异步取消函数保存和过期注册释放，与公共 `trackAsyncSubscription` 重复。

## Decision

注册返回的 Promise 和宿主代次判断交给公共工具，effect 直接返回它的清理函数。保留同步调用宿主方法的时机和接收者、过期事件守卫、保存期间保留最大 revision 的队列及注册拒绝静默策略。

## Alternatives considered

- 保留局部清理：生命周期集中在同一个 effect 中，便于单独阅读；代价是继续维护已有公共工具覆盖的取消句柄规则。
- 沿用窗口监听的微任务注册：可以在开始注册前过滤失效宿主；但设置监听原来立即注册，延后会改变事件接入时机，当前抽取不需要它。

## Verification

相关 6 个文件的 16 项测试在抽取前后通过，覆盖正常清理、换宿主或卸载后的晚到注册、旧事件隔离、普通重渲染、StrictMode 和注册拒绝。现有整页跨窗口和保存回声的 4 项用例在抽取前后通过；测试保留真实 hook 与订阅工具，只替换宿主接口。类型检查、生产构建、lint、格式、quick、笔记门禁和独立审查通过。

## Consequences

设置监听减少一份取消句柄保存与释放逻辑，公共工具处理当前清理和晚到注册。业务回调继续用原有代次守卫，自动保存、冲突重试和即时写入 refs 的策略保持原样。这里验证共享前端的订阅生命周期，原生装机不属于本项覆盖面。

## Existing notes audit

按 `useSettingsPersistence`、`onPreferencesChanged` 和持久化订阅检索活跃笔记，没有同消费者归属。[窗口监听记录](2026-10-14-window-state-subscription.md)讨论另一个消费者的微任务注册边界，本项保留设置监听的立即注册。
