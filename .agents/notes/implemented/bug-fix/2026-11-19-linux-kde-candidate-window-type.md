# Agent Note: Linux X11 候选窗使用菜单窗口类型

Status: implemented

## Problem

KDE Plasma 5 的 X11「变形弹出窗口」特效会对 `_NET_WM_WINDOW_TYPE_TOOLTIP` 的每次尺寸或位置变化做淡入淡出。Linux 原生候选窗在连续输入时几乎每个字符都会变化，因此旧候选帧残留并造成发虚、闪烁。Fcitx5 自带输入窗口使用 `_NET_WM_WINDOW_TYPE_POPUP_MENU`，不触发这项特效。

## Decision

Linux X11 原生候选窗声明为 `_NET_WM_WINDOW_TYPE_POPUP_MENU`。候选窗仍然是 override-redirect 窗口，继续使用现有的输入形状、鼠标处理、定位、绘制和退场动画。悬浮工具栏、双拼键位提示等其他窗口保持原有类型，因为它们不属于连续变化的候选列表。

## Alternatives considered

- 要求用户关闭 KDE「变形弹出窗口」特效：能绕过残影，但把产品缺陷转成每台机器的手工配置，无法随包修复。
- 继续使用 `TOOLTIP` 并降低候选窗更新频率：会让候选内容或位置滞后，且不能保证 KWin 的过渡被取消。
- 修改所有 X11 浮层的窗口类型：范围过大，会改变工具栏和双拼提示的窗口管理语义，本问题只涉及原生候选窗。

## Consequences

- KDE Plasma 5 X11 下候选窗更新直接显示新内容，不再触发 tooltip 淡入淡出残影。
- 候选窗仍不接收键盘焦点，鼠标选择、滚轮、窗口定位和透明形状保持原有路径。
- 本机没有 KDE Plasma 5 X11 会话，未能进行报告环境的真实视觉验收；Linux native host 的编译和测试由 PR CI 执行，需报告者复验候选窗连续输入。

## Verification

- `git diff --check`。
- Linux native host 编译和测试由 PR CI 执行。
- 未添加源码文本守卫，窗口类型通过实际 X11 窗口属性在报告环境验收。
