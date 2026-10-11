# Agent Note: Android AI 皮肤页视图生命周期

Status: implemented

## Problem

`AiSkinPage` 的“使用此皮肤”保存任务通过 `HostTask` 异步执行。视图销毁后，旧回调会被生命周期检查丢弃，但 Fragment 字段 `saving` 原先保持为真；页面重建后使用按钮永久禁用。

## Decision

在 `onDestroyView()` 清理预览和观察者时同时重置 `saving`。旧任务结果仍绑定旧视图并被丢弃，重建后的页面可以再次保存。

## Alternatives considered

- 依赖旧回调恢复 `saving`：回调已被旧视图检查丢弃，无法可靠恢复。
- 把保存状态放进生成用 ViewModel：保存是页面级一次性动作，跨视图保留会把旧视图的忙碌状态带入新页面。

## Verification

`python3 scripts/test-android-ai-skin-view-lifecycle.py` 在修复前按预期失败，修复后通过；另运行 `git diff --check`。

## Consequences

离开 AI 皮肤页会放弃当前视图的保存 UI 状态；后台保存可能仍完成，返回页面后可重新发起保存。
