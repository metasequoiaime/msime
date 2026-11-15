# Agent Note: Fcitx5 模式提示避开输入事件重入

Status: implemented

## Problem

#11438 报告 KDE Wayland 的 WPS 文档编辑区在开启 `input_mode_hud` 后失去键盘输入，关闭该偏好后恢复；候选仍能用鼠标选择。Fcitx5 的新模式提示 API 会在 `toggleInputMode()` 的按键处理栈内同步写入 overlay message 并刷新 InputPanel。Qt 输入法模块可能在这次同步刷新中重入客户端输入事件，导致报告中的兼容路径失效。现有代码没有把这条 UI 更新与按键事件隔开。

## Decision

新版本 Fcitx5 的 `showCustomInputMethodInformation` 调用经 `ModeHudDeferredUpdate` 排到当前 Fcitx5 输入事件返回后的下一轮事件循环。连续切换只保留最新提示，失焦、关闭会话、清空面板和插件销毁都会取消待执行回调；回调再次确认上下文仍有焦点且未受限，再更新面板。旧版 Fcitx5 没有该 API，已有辅助行回退保持原来的同步路径和生命周期。

## Alternatives considered

- **关闭所有 Wayland 的输入模式提示**：可以绕开客户端兼容性，但会让正常的 Wayland 应用失去用户已开启的反馈，也无法覆盖 XWayland 或其他 Qt 前端，因此不采用。
- **按 WPS 进程名添加特判**：不稳定，无法区分 WPS 自带 Qt 输入法模块、原生 Wayland 与 XWayland 前端，也没有报告者提供的 `fcitx5-diagnose` 字段作为边界，因此不采用。
- **把 `input_mode_hud` 默认值改为关闭**：会改变所有 Linux 用户的现有行为，且不属于本 issue 的兼容修复，因此不采用。

## Consequences

模式提示最多延后一轮 Fcitx5 事件循环，避免在按键或焦点事件栈中同步触发客户端 InputPanel 更新；旧版开发头和不具备新 API 的发行版行为不变。待执行提示由代次和焦点检查保护，不会在上下文关闭后访问已失效的会话。WPS/KDE Wayland 报告者环境的真实复验仍需在 PR 验证段完成，不能由隔离的 Fcitx5 测试代替。

## Verification

`fcitx5-mode-hud-deferred-update` 行为测试覆盖不在 `schedule()` 内同步执行、旧提示代次丢弃和取消后的回调不执行；Fcitx5 合约测试检查生产路径使用该延后边界。完整 Fcitx5 宿主构建和报告者的 WPS/KDE Wayland 复验待 CI 与后续环境验证。
