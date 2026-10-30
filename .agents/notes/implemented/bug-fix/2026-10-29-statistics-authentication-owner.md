# Agent Note: 统计认证开关保存绑定当前客户端

Status: implemented

## Problem

「我的」页的统计认证开关在初始读取时用局部 `current` 标记隔离旧客户端，但保存动作没有归属检查。旧客户端保存等待期间替换设置客户端，旧回执会把新客户端刚读取的开关状态改回旧值；旧回执的 `finally` 也可能清掉新客户端的忙碌状态。

受控保存 Promise 回归在旧实现中复现了旧回执覆盖新客户端选择。

## Decision

复用共享 `useAsyncActionRunner`，以统计认证客户端作为操作所有者。保存完成后只在当前客户端仍有效时写回开关；客户端替换或卸载会使旧操作的反馈和忙碌收尾失效。初始读取继续使用局部挂载标记，并在客户端替换时清除旧错误。已经发出的宿主保存请求不取消，也不回滚宿主已完成的写入。

## Existing notes audit

- [统计页认证默认值与冷却](../../implemented/feature/2026-10-27-statistics-auth-default-off-cooldown.md)：管理认证策略和平台冷却，本次只修共享设置页保存归属，保留。
- [移动端统计认证规则](../../implemented/bug-fix/2026-10-25-mobile-statistics-auth-parity.md)：管理原生认证入口，本次不改变原生行为，保留。
- [宿主操作按钮复用公共异步 runner](../../implemented/simplification/2026-10-14-host-action-runner.md)：记录公共 runner 的生命周期语义，本次复用其客户端 owner，保留。
- [偏好恢复客户端归属](../../implemented/bug-fix/2026-10-15-preference-recovery-owner.md)：同类旧客户端回执隔离，部分重叠，保留各自业务边界。

## Alternatives considered

只在保存完成后比较组件当前的 `client`：改动最少，但不能可靠阻止旧保存的忙碌收尾覆盖新操作，也会在客户端切换与保存完成同一提交中产生竞态。公共 runner 同时隔离回执、错误和 busy 生命周期，因此采用它。

## Verification

新增回归先在旧实现中失败，修复后认证设置 3 项测试通过。变更范围的桌面类型检查、生产构建、lint、格式、笔记校验、差异检查和本地 quick 通过后提交独立 PR；共享 UI 改动不改变 Android/iOS 发布设置页。

## Consequences

客户端替换或卸载后，旧保存仍可在宿主完成，但不会更新新视图、错误或 busy。当前客户端的保存失败仍显示原有 toast；初始读取失败仍禁用开关并显示原有说明。
