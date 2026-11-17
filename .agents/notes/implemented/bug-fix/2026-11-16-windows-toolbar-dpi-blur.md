# Agent Note: Windows 悬浮工具栏按每显示器 DPI 创建

Status: implemented

## Problem

Windows 报告实际悬浮工具栏整体模糊，而设置页里的同一工具栏预览清晰。`FloatingToolbarWindow` 在 Server 进程中创建窗口时没有进入每显示器 DPI 上下文，也没有使用透明 DirectComposition 窗口的无重定向位图样式。Server 不带现代 DPI 清单时，DWM 会按进程 DPI 缩放这条窗口，文字、图标和圆角一起变糊。

## Decision

悬浮工具栏创建窗口前复用[Windows DPI 与 DirectComposition 兼容层](../../implemented/feature/2026-10-23-windows-optional-dpi-and-composition.md)里的 `ThreadDpiScope`，让窗口继承 `PerMonitorV2`；创建样式同时复用 `no_redirection_bitmap_style()`，让 DirectComposition 的透明表面直接交给合成器，旧系统由兼容层返回零并继续走已有回退。刷新布局也在同一作用域内计算物理像素并调用 `SetWindowPos`，退出时恢复调用方上下文。只在创建时切换会让不感知 DPI 的 Server 在 125% 系统缩放下把已经换算过的尺寸再次放大：Windows 11 实测从应有的 333×100 变成 416×125。Server 冒烟测试核对窗口与刷新回调的 DPI 上下文、调用方上下文恢复和新系统上的样式位。既有兼容层的决定保持不变，本篇记录悬浮工具栏漏接这条窗口级约定的修补。

这只修窗口创建与合成边界，不改变工具栏大小、字体、布局、颜色、默认位置或用户偏好。

## Alternatives considered

- 只把工具栏的绘制尺寸乘上系统 DPI：能让部分屏幕上的像素尺寸看似正确，但窗口仍由错误的 DPI 上下文管理，跨显示器和 DWM 重定向仍会缩放；因此复用创建窗口时已有的 PMv2 约定。
- 只加入无重定向位图样式：能避免一层 DWM 重定向，但无法修复 DPI 不感知窗口被系统缩放的根因；因此与线程 DPI 作用域一起接入。
- 修改 Server 的全局 DPI 清单：会改变 TSF Server 进程中所有窗口和旧系统回退的边界，影响候选窗、输入提示和 Win7；本次只让悬浮工具栏窗口按现有窗口级模式创建。

## Consequences

悬浮工具栏在 Windows 10/11 的高 DPI 显示器上按自身 DPI 绘制，避免整条工具栏被 DWM 拉伸；跨显示器时沿用窗口已有的 `WM_DPICHANGED` 重建逻辑。旧系统不声明新样式时仍使用原有分层窗口路径。代价是工具栏创建路径多一个可失败的 PMv2 作用域，失败会沿用既有组件失败处理；服务器全局 DPI 行为保持不变。

Windows 11 build 26300 的交互桌面使用真实 `FloatingToolbarWindow`、同一组合状态和缓存的产品渲染库，在 125% 系统缩放下完成前后截图；验收后恢复原来的系统缩放。截图只包含合成状态和纯色背景。
