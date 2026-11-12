# Agent Note: Wayland 会话中的悬浮工具栏拖动抓取指针

Status: implemented

## Problem

Linux 悬浮工具栏经 XWayland 显示时，按住左端握把移动指针会离开 override-redirect 窗口。原实现没有抓取指针，窗口因此收不到后续移动或松开事件；工具栏能显示、握把也会变成移动光标，但位置不变。

## Decision

X11/XWayland 后端在确认按下拖动握把后，用 `XGrabPointer` 把按钮、移动和松开事件交给工具栏，继续用根坐标计算窗口位移。拖动结束、工具栏隐藏或 X 连接关闭时释放抓取。保留现有的 X11/XWayland 浮层后端，不在本 issue 中新增 Wayland 原生浮层协议；该边界与 Linux 当前对不能安全放置的 Wayland 浮层回退菜单的架构约定一致。

## Alternatives considered

直接在每次 `MotionNotify` 中查询全局指针位置可以绕过窗口离开，但仍无法可靠收到按钮松开事件，也不能处理 XWayland 的事件路由，因此不采用。新增 layer-shell/Wayland 原生工具栏可以获得完整的 Wayland 定位能力，但会扩大宿主协议和合成器兼容范围，超出本 issue 的拖动缺口。

## Consequences

拖动过程中指针离开工具栏仍能持续移动并在窗口外松开，X11 和 XWayland 的行为保持一致。抓取失败时本次拖动被忽略，避免进入无法结束的拖动态；隐藏或连接断开会清理抓取状态。真实 Deepin Wayland 验收仍需在报告者环境确认。
