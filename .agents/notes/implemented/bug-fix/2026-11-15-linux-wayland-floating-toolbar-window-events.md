# Agent Note: Wayland 会话不再创建 Xwayland 悬浮工具栏

Status: implemented

## Problem

#11474 报告 Arch Linux / Hyprland 的 Wayland 会话在切换中英文时反复出现和消失 `MSIMEFloatingToolbar`。Linux 的悬浮工具栏只有 X11 后端，原实现看到 `DISPLAY` 就通过 Xwayland 创建 `override-redirect` 窗口；在不提供 root overlay 语义的 Wayland 合成器里，这类窗口仍会进入顶层窗口事件和布局，导致最大化窗口被窗口规则恢复。

此前 [Linux X11 浮层决定](../feature/2026-10-21-linux-x11-floating-panels.md) 把“Wayland 有 Xwayland 就复用 X11 后端”作为兼容前提；[satellite 浮层回退](2026-10-25-linux-satellite-overlay-fallback.md) 只针对已确认缺少 root overlay 的 `xwayland-satellite`，不能覆盖本 issue 的普通 Wayland 合成器。

## Decision

`FloatingToolbar::default_surfaces` 只在没有 `WAYLAND_DISPLAY` 且存在 `DISPLAY` 的纯 X11 会话返回 X11 后端。Wayland 会话直接让宿主沿已有的状态区“工具栏”菜单回退，不创建 `FloatingToolbarX11Surface`，因此不会产生 `MSIMEFloatingToolbar` 的映射、解除映射或顶层窗口事件。共享工具栏偏好和状态区动作保持不变；后续若实现原生 Wayland 工具栏，应通过 `FloatingToolbarSurface` 接入独立的 layer-shell 或其他不会进入应用窗口列表的后端。

纯函数 `floating_toolbar_should_use_x11` 集中表达会话判定，IBus 与 Fcitx5 共用同一工厂，因此两个宿主不会对 Wayland 走不同路径。

## Alternatives considered

- **新增 Wayland layer-shell 工具栏后端**：能保留悬浮工具栏的按钮、拖动和菜单，也能避免窗口管理器事件；但需要单独实现指针事件、菜单、缓冲区尺寸和输出生命周期，且 GNOME 默认没有 layer-shell，超出本次报告的安全修复范围。
- **继续使用 Xwayland，只修改窗口类型或 `InputHint`**：改动较小，但现有窗口已经是 `override-redirect` 且不接受键盘焦点；报告现象说明这些属性不足以阻止合成器把它当作新窗口，不能把未经验证的属性组合当作修复。
- **只对 Hyprland 或某个窗口管理器做黑名单**：能缩小功能回退范围，但无法覆盖同样缺少 root overlay 语义的其他 Wayland 合成器，也会把桌面名称当成不稳定的协议判断。按会话协议统一回退更安全。

## Consequences

纯 X11 用户保留原有悬浮工具栏、菜单、拖动和位置记忆。Wayland 用户不再因输入模式切换触发普通顶层窗口事件，状态区仍可访问同一组工具栏动作和设置；Wayland 的悬浮工具栏视觉与交互暂时缺失，待独立的原生 Wayland 后端实现后再恢复。该修复不改变输入模式提示的 layer-shell 徽章，也不改变 `floating_toolbar.enabled` 的共享偏好值。

策略行为由 `floating_panels_model` 新增断言覆盖。当前环境没有 nlohmann JSON 头文件，无法在本工作树直接编译完整 Linux runtime target；独立编译运行了不依赖第三方库的会话判定测试，并通过 `git diff --check`。真实 Hyprland、Wayland 合成器和报告者环境尚未验收，保留为 PR 验证缺口。
