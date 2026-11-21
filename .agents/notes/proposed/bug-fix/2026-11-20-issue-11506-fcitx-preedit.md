# Agent Note: 跟踪 Fcitx5 Wayland 预编辑复测

Status: proposed

## Problem

`#11506` 报告 KDE Plasma Wayland、Fcitx5 下候选窗口正常显示，但 Qt 应用输入框不显示拼音预编辑。报告环境使用 Fcitx5 5.1.22。代码核查显示水杉仍声明客户端预编辑能力并调用 `updatePreedit`，候选窗口预编辑选项不会清空客户端预编辑。

## Proposal

建立独立 Draft PR 跟踪上游 Fcitx5 5.1.23 或更新版本的复测。该版本发布说明涉及 Wayland text-input v2 的 `done` 刷新，可能覆盖报告现象。复测需要记录 Fcitx5、Fcitx5-Qt、Qt、`QT_IM_MODULE`、`QT_QPA_PLATFORM` 和脱敏诊断输出。上游修复后仍复现时，再在本 PR 加入水杉侧最小改动和 Qt/Fcitx5 回归。

## Alternatives considered

- **回退水杉的客户端预编辑能力**：最强理由是可能绕开 Wayland 客户端预编辑链路，但会破坏候选窗口预编辑关闭选项和其他 Qt 应用，因此不采用。
- **把预编辑文本强制绘制到候选窗口**：最强理由是可以让用户看到拼音，但无法恢复目标应用输入框的行内文本，也会改变既有设置语义，因此不采用。

## Acceptance criteria

- 在同一 KDE Plasma Wayland 环境更新 Fcitx5/Fcitx5-Qt 后复测。
- 若上游更新后仍复现，明确水杉侧责任并加入对应代码和回归测试。

## Risks

当前 PR 只有调查记录，不宣称水杉已经修复 Fcitx5 5.1.22 的预编辑问题。回退宿主能力会影响所有客户端预编辑用户。
