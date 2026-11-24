# Agent Note: Wayland 会话使用原生悬浮工具栏

Status: implemented

## Problem

#11619 报告 Deepin 25 DDE 的 Wayland 会话中，Fcitx5 使用水杉 0.17.0 时没有悬浮工具栏。0.16.1 经 Xwayland 创建工具栏，合成器有可能将它列为普通顶层窗口、改变焦点或令最大化窗口恢复。[#11475 的回退](2026-11-15-linux-wayland-floating-toolbar-window-events.md) 避开了窗口副作用，同时只剩状态区入口。纯 X11 的工具栏一直可用。

## Decision

IBus 与 Fcitx5 以 `WAYLAND_DISPLAY` 选择后端。Wayland 会话若能绑定 `zwlr_layer_shell_v1`，以 Top 层承载工具栏，键盘交互设为 None。复用原工具栏的绘制、按钮和宿主动作。指针进入和按下只影响浮层，切换编辑器或输入法时仍由活动上下文管理显示。合成器负责决定浮层与全屏应用的叠放关系。

Wayland 没有 X11 根窗口坐标。以右下边距保存拖动位置，松手才移动表面，使按住过程的指针坐标保持稳定。Wayland 位置文件与 X11 的绝对坐标分开。方案和齿轮菜单在同一表面中展开，输入区域只覆盖工具栏和菜单本体。缓冲经 Wayland 的 release 事件回收，并由宿主主循环推进协议。

## Alternatives considered

- **恢复 Xwayland 窗口**：可以复用已有绘制、拖动、菜单和全屏判断，且有些桌面能显示。#11474 已证明合成器可以将它纳入顶层窗口管理，恢复该做法会重新引入焦点和最大化问题。
- **沿用状态区菜单**：不需要新增协议且对编辑器焦点没有影响。Deepin 用户需要可见的浮层，只有菜单无法恢复这项交互。没有 layer-shell 的桌面仍沿用菜单。

## Consequences

提供 layer-shell 的 Wayland 会话可以使用工具栏按钮、菜单和拖动；纯 X11 继续使用原后端。Wayland 后端以输出的整数缩放作图，悬停提示目前由纯 X11 后端提供。负责叠放和屏幕选择的是当前合成器，设备验收覆盖支持协议的合成器。合成器未提供 layer-shell 时，菜单仍可使用。
