# Agent Note: 配置恢复结果绑定当前设置客户端

Status: implemented

## Problem

`usePreferenceRecovery` 通过 `useAsyncActionRunner` 执行损坏配置修复，但没有把 `client` 作为 runner 的 owner。恢复请求等待宿主返回期间切换设置客户端，旧请求仍被视为当前请求；它会把旧快照写入新客户端的界面。恢复流程的 `finally` 还无条件调用 `setBusy(false)`，会清掉新客户端自己的忙碌状态。

## Decision

把 `client` 传给 `useAsyncActionRunner`，让客户端替换递增代次并使旧恢复结果失效；`finally` 只有在 `isCurrent()` 仍为真时才释放忙碌状态。已经发出的宿主恢复请求不取消，但迟到结果不再更新新客户端的快照、提示或忙碌状态。

## Alternatives considered

- 只给 `finally` 加代次检查不能阻止旧恢复结果写入新客户端，状态污染仍然存在。
- 在宿主层取消恢复请求需要新增取消协议，且不是所有 `SettingsClient` 都能取消；复用现有 runner 的 owner 代次可以覆盖结果、错误和清理边界。

## Verification

新增回归先在旧实现上运行，迟到恢复会错误调用 `setSnapshot`，测试失败；修复后恢复测试 3 项全部通过。回归覆盖客户端替换、旧快照和草稿不落地，以及旧请求不释放新客户端忙碌状态。改动文件的 lint、格式和 `git diff --check` 通过；完整 quick 和 CI 在提交前继续运行。

## Consequences

设置客户端替换后，旧恢复操作仍可能在宿主中完成，但其结果只被丢弃，不再干扰当前设置窗口。当前客户端的恢复流程和错误提示保持原有行为。
