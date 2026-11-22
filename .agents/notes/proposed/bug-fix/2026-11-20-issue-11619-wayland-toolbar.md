# Agent Note: 调查 Wayland 悬浮工具栏不显示

Status: proposed

## Problem

`#11619` 报告 Deepin 25 DDE（Wayland 会话）、Fcitx5 下从 0.16.1 升级到 0.17.0 后悬浮工具栏不显示；0.16.1 能显示但无法拖动（`#11311`，已关闭）。报告者已提供的只有 issue 模板里的系统信息：v0.17.0、Deepin 25、内核 7.3.0-amd64-desktop-rolling、DDE (wayland)、Fcitx5、x86_64、全拼。

0.17.0 的行为来自 `#11475`（2026-11-16 合入）的有意回退：`platforms/linux/src/overlay/FloatingToolbarPlatform.h` 的 `floating_toolbar_should_use_x11` 只在没有 `WAYLAND_DISPLAY` 且有 `DISPLAY` 时返回 true，`FloatingToolbarFactory.cpp` 据此在 Wayland 会话不创建 XWayland `override-redirect` 工具栏，工具栏动作只留在状态区菜单的「工具栏」子菜单。回退的原因是 `#11474`：在 Hyprland 上这块窗口成了合成器管理的普通顶层窗口，切换中英文时反复新建、销毁 `MSIMEFloatingToolbar`，参与布局和焦点管理，并把其他最大化窗口恢复成原尺寸。

维护者 2026-11-18 在 issue 中说明不回退 `#11475`，并请报告者补充最新 nightly 或 0.17.x 构建、`msime-linux` 包来源、`fcitx5-diagnose` 中的 compositor 与 Wayland 窗口后端、启动日志，以及切换到纯 X11 会话后是否显示。2026-11-22 查看时，issue 下只有这一条维护者评论，报告者还没有补充上述信息；引用 `#11619` 的只有承载本 note 的 `#11698`，没有修复 PR；按「悬浮工具栏 wayland」「工具栏 不显示」等关键词搜索，没有找到同一现象的其他 issue。

## Proposal

本 note 作为 `#11619` 的跟踪记录合入 develop。修复另起 PR（正文写 `Fixes #11619`），前提是拿到报告者的 compositor、协议和窗口后端信息，并确认一种在该合成器上不会进入顶层窗口列表的浮层承载方式（Wayland 原生浮层后端，或经验证安全的窗口协议）。修复 PR 接入最小后端，只在已验证的合成器上启用，并附回归测试。

## Alternatives considered

- **重新按 `DISPLAY` 启用 XWayland 工具栏**：最强理由是可以立即恢复部分用户的显示，但会重新引入 `#11474` 已记录的顶层窗口、焦点和最大化状态问题，因此不采用。
- **把工具栏恢复为普通顶层窗口**：最强理由是多数桌面环境能显示它，但会让输入法浮层抢焦点并改变现有交互边界，因此不采用。

## Acceptance criteria

- 确认报告者的 compositor、协议和窗口后端。
- 在对应环境验证工具栏显示、拖动、焦点切换和全屏、最大化窗口行为。
- 修复 PR 附 Linux 原生回归测试和实际环境验收记录，并保持 `#11474` 的场景不回归。

## Risks

本 note 只是调查与跟踪记录，不宣称恢复了 Wayland 工具栏。未经窗口协议验证的回退会重新引入已修复的焦点回归。`platforms/linux/README.md` 中仍有「GNOME、KDE 和 wlroots 的 Wayland 会话经 Xwayland 显示同一块窗口」「能连上支持根窗口浮层的 X 服务器（含兼容的 Xwayland）时悬浮工具栏照常显示」「普通 X11 和其他兼容的 Xwayland 后端继续显示」等描述，与 `#11475` 之后 Wayland 会话一律不创建工具栏的行为不符，修复 PR 需一并更正。
