# Agent Note: 设置首次加载与重载共用请求

Status: implemented

## Problem

`useSettingsPersistence` 的首次 `client.load()` 不占用 `reloadInFlightRef`。移动设置页在首次加载尚未返回时恢复前台，或用户触发手动重载，会再次调用同一宿主的 `load()`。代次令牌会阻止旧结果覆盖新状态，但两个请求仍会同时读文件或远端快照，并各自执行收尾，增加 I/O 和状态竞态面。

## Decision

首次加载把自己的 Promise 放入 `reloadInFlightRef`，并在结束时只清理仍由自己持有的槽位。`reload()` 继续复用该 Promise；宿主切换时已有的代次和槽位清理仍使旧请求不能阻止新宿主加载。

## Alternatives considered

- **继续依赖代次令牌并允许两次加载**：实现改动最小，旧结果不会覆盖新结果；但重复读取仍然发生，且每次前台恢复都可能放大 I/O，因此不满足同一宿主一次加载的生命周期约束。
- **取消首次请求后重新加载**：可以只保留一个请求，但 `SettingsClient` 没有取消契约，取消只能丢弃结果，无法停止宿主已经开始的读操作；还会让首次错误和重载错误的显示时序分叉。

## Consequences

首次加载进行中触发重载时只保留一个宿主请求，重载调用者等待首次请求完成，成功、失败和 `busy` 收尾沿用首次加载路径。独立重载仍然可以在首次加载完成后发起；宿主切换时旧 Promise 的 `finally` 不会清理新宿主的槽位。测试覆盖了复用、成功/失败的旧宿主隔离和独立重载互斥。

## Verification

新增回归测试先在旧实现上确认两次 `client.load()`，修复后通过。定向 `settings-persistence-reload-owner.test.tsx` 共 5 项通过。
