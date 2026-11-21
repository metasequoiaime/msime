# Agent Note: 调查 Wayland 悬浮工具栏不显示

Status: proposed

## Problem

`#11619` 报告 Deepin 25 DDE、Wayland、Fcitx5 下从 0.16.1 升级到 0.17.0 后悬浮工具栏不显示。当前代码是针对 `#11474` 的有意回退，在 Wayland 会话禁用 XWayland `override-redirect` 浮层，避免部分合成器把工具栏当作普通顶层窗口并触发 focus in/out 或覆盖全屏窗口。

## Proposal

建立独立 Draft PR 跟踪报告环境证据和 Wayland 原生浮层方案。需要最新 nightly 或 0.17.x 构建、软件包来源、`fcitx5-diagnose` 的 compositor 与窗口后端信息、启动日志，以及纯 X11 对照结果。确认具体协议和合成器后，再在本 PR 接入最小后端和回归测试。

## Alternatives considered

- **重新按 `DISPLAY` 启用 XWayland 工具栏**：最强理由是可以立即恢复部分用户的显示，但会重新引入 `#11474` 已记录的窗口焦点和全屏覆盖问题，因此不采用。
- **把工具栏恢复为普通顶层窗口**：最强理由是多数桌面环境能显示它，但会让输入法浮层抢焦点并改变现有交互边界，因此不采用。

## Acceptance criteria

- 确认报告者的 compositor、协议和窗口后端。
- 在对应环境验证工具栏显示、拖动、焦点切换和全屏窗口行为。
- 代码改动附 Linux 原生回归测试和实际环境验收记录。

## Risks

当前 PR 只有调查记录，不宣称恢复了 Wayland 工具栏。未经窗口协议验证的回退会重新引入已修复的焦点回归。
