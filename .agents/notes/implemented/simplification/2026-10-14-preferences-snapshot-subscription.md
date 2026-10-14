# Agent Note: 偏好快照监听复用公共订阅清理

Status: implemented

## Problem

`usePreferencesSnapshot` 手写取消句柄保存、过期注册释放和卸载清理。公共 `trackAsyncSubscription` 已持有这些规则，但快照 hook 还把注册与首次加载写在同一个 async 函数里。

## Decision

注册 Promise 交给公共工具，首次加载作为同一 Promise 的成功和失败续接。保留立即调用宿主方法及接收者、注册结束后才加载、注册失败时继续加载、revision 单调采纳，以及 `preferences` 和 `resetOnClientChange` 的 owner 代次。

取消句柄只等待注册，不等待快照加载。失效注册先释放，失效续接不发起加载，旧事件和旧加载结果仍由代次守卫隔离。

## Alternatives considered

- 保留局部清理：注册、加载与取消在同一函数中直观可读；但重复维护公共工具已有的释放规则。
- 让整个 async 函数在加载结束后返回取消句柄：改动少且仍能直接使用公共工具；但未完成的加载会延迟卸载时释放监听，不能保持原有生命周期。

## Verification

真实 hook、既有 owner 契约和公共订阅工具的 3 个文件、15 项测试在抽取前通过，连同桌面键盘集成的 4 个文件、21 项测试在抽取后通过。生命周期覆盖订阅顺序、加载未完成时卸载、晚到注册、宿主或重置选项变化、StrictMode、同步与异步注册失败、缺少能力、加载失败和 revision 去重。同步注册抛错的时序断言在旧实现通过，曾在迁移中发现微任务延迟，修正后通过。类型检查、构建、lint、格式、quick、笔记门禁和独立审查通过，审查者另运行 3 个文件、15 项测试通过。

## Consequences

快照 hook 删除局部取消句柄保存与释放逻辑，注册资源交给公共工具维护。首次加载仍在注册结束后启动，注册失败继续加载；同步注册抛错仍立即加载，异步拒绝和缺少能力沿用 Promise 续接时机。取消句柄的持有独立于加载，避免未完成加载延迟卸载清理；注册与加载续接使用相同代次守卫。

## Existing notes audit

按 `usePreferencesSnapshot`、`use-preferences-snapshot` 和偏好快照检索活跃笔记，没有同消费者归属。[设置持久化](2026-10-14-settings-persistence-subscription.md)与[窗口监听](2026-10-14-window-state-subscription.md)各自持有不同消费者的注册顺序，本篇记录先注册后加载的快照边界。
