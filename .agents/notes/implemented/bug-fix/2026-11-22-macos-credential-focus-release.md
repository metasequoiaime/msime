# Agent Note: macOS 设置凭据退出时显式释放焦点

Status: implemented

## Problem

[#11782](https://github.com/metasequoiaime/msime/issues/11782) 报告设置窗口里的密码字段在切换窗口、设置页或显示状态时，WKWebView 有时留下安全键盘输入状态，影响系统快捷键以及输入法在其他应用里的字段判定。

## Decision

macOS 设置两种凭据输入共用 React 焦点清理。密码字段退出可见窗口、卸载或切换到明文时，只有当前确实聚焦的字段主动执行 `blur()`。显示按钮先释放焦点再更改 `type`。使用原生密码输入和 WebKit 的安全输入所有权，组件不操作系统的安全输入计数。

已有[凭据回显规则](2026-10-16-cloud-relay-token-field-redaction.md)和[输入法字段判定](../architecture/2026-10-11-desktop-lifecycle-core.md)继续适用，这里只改变设置窗口的焦点生命周期。

## Alternatives considered

- 用普通文本输入和 CSS 圆点可以完全绕开 WKWebView 的密码路径，但会丢掉浏览器的密码语义和输入保护，且读屏行为会改变。
- 在宿主直接调用 `DisableSecureEventInput` 无法区分当前引用是否由设置窗口持有，主动修改会破坏另一个合法的密码输入会话。

## Consequences

返回设置窗口后需要重新聚焦密码字段继续输入，字段值和遮罩状态保留。前端回归覆盖失焦、切页和切换显示的时序，系统安全输入是否随之释放仍由 WKWebView 决定。
