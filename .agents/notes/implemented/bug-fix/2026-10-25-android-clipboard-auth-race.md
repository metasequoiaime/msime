# Agent Note: 剪贴板导出认证避免重复启动

Status: implemented

## Problem

剪贴板导出按钮在系统认证窗口尚未返回时仍保持可点击。原实现只检查导出任务的 `busy` 状态，而认证进行中 `busy` 仍为假；连续点击会重复创建 `BiometricPrompt`，在部分设备上可能取消先前认证或抛出窗口状态异常。

## Decision

在共享 `ClipboardOperationAuthenticationPolicy` 中集中表达“认证窗口已存在时不再启动”的规则，`ClipboardExportActions` 在创建 `BiometricPrompt` 前检查该规则。认证成功、失败或详情页销毁时仍清空当前认证对象，后续操作可以重新认证。

## Alternatives considered

- **只把按钮置灰**：认证开始前没有进入导出任务，现有 `busy` 状态不会更新，无法覆盖认证窗口显示期间的重复点击。
- **在每个页面各自维护标志**：设置页与剪贴板搜索页共用导出动作，分散状态容易再次出现竞态，因此将判定放在共享策略并由动作对象持有当前提示。

## Testing

新增 JVM smoke 检查认证未开始时允许启动、认证已进行时拒绝重复启动。修改前 `platforms/android/check-host.sh` 因缺少策略方法失败，修改后该门禁通过。

## Consequences

认证窗口显示期间的重复点击被忽略；认证结束后导出行为和目录选择流程保持不变。
