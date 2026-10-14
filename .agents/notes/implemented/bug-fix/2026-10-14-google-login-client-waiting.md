# Agent Note: Google 登录等待状态随客户端更换清理

Status: implemented

## Problem

`useAccountAction` 在账号客户端更换时解除旧操作的忙碌状态并作废结果，但 `useAccountGoogleLogin` 的等待状态仍属于上一次客户端。旧请求的 `finally` 因代次失效不会清理等待。账号页自行补了重置 effect，`LoginSheet` 没有，因而保留「正在等待浏览器」与取消按钮；随后进行验证码操作时，过期等待标记还让本应禁止关闭的面板可以关闭。

## Decision

由共享 Google 登录 hook 在客户端变更的 effect 中清理等待状态和同步标记，两个消费者共用这条边界。取消旧客户端等待仍由原有消费者的 effect 清理执行；不在 hook 的旧 effect 清理中提前改同步标记。账号页移除重复重置，旧请求结果及收尾继续使用已存在的客户端代次保护。

## Alternatives considered

只在 `LoginSheet` 增加与账号页相同的重置 effect，改动最少，也能修复当前面板。但两个消费者都必须知道 hook 的内部等待生命周期，以后新入口仍容易漏清理，因此把重置归到状态拥有者。

换客户端时给面板加 key，强制卸载旧实例，可以连同所有表单状态一起清空。但会丢弃与本问题无关的输入和焦点，并改变整个面板的生命周期，因此保留原有实例，只清理客户端拥有的 Google 等待。

在旧请求 `finally` 中无条件清理，容易恢复表面状态。但旧请求可能在新客户端已经开始等待后才结束，会覆盖新操作，继续保留代次守卫。

## Consequences

更换客户端后界面不再显示已经取消的 Google 等待，验证码请求仍遵守忙碌期间不能关闭的规则。等待中的关闭及取消按钮、旧客户端取消一次、卸载时清理等契约保持原语义。这里只清理 Google 等待，不重置验证码表单；原生监听和授权流程由各宿主继续管理。

## Testing

`apps/desktop/tests/account/login-sheet-client-generation.test.tsx` 的四条新增回归在旧实现上全部失败，旧取消按钮及等待文案在客户端更换后仍存在；共享 hook 版本全部通过。

真实 `LoginSheet` 行为测试在普通及 StrictMode 中更换客户端，检查等待文案和取消按钮消失、验证码请求期间不能关闭，以及旧 Google 请求迟到成功或失败不会结束新请求的等待或提交用户。已有账号页 Google 登录、取消、卸载及登录面板用例，类型检查、前端构建、笔记和 quick 门禁通过。

## Related decisions

已按 `LoginSheet`、Google 等待、`useAccountGoogleLogin` 和客户端代次检索活跃笔记。[鸿蒙 Google 登录](../feature/2026-10-09-harmony-google-sign-in.md)继续持有系统浏览器、回环监听及关闭取消流程；本篇只补共享 UI 的客户端更换边界。[面板焦点恢复](../bug-fix/2026-10-14-account-sheet-restore-focus.md)和[组词期间 Escape](../bug-fix/2026-10-14-account-sheet-ime-escape.md)的决定不变。
