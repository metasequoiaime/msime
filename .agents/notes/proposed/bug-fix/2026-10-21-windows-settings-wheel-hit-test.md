# Agent Note: Windows 设置正文空白区域参与滚轮命中

Status: proposed

## Problem

#9161 在真实 WinUI 设置窗口中对照确认：卡片内部可上下滚动，同页卡片间隙不能滚动，且没有位于滚动边界。页面 StackPanel 和外层 ScrollViewer 的背景均未显式设置，空白位置不能沿预期元素树传递指针事件。

## Proposal

为正文页面及视口设置显式透明背景，让 WinUI 按原有滚动规则处理空白区域的滚轮。设置页的颜色、尺寸、控件内滚动与系统滚轮选项由既有实现持有。

复用 settings_ui.ps1 的真实 WinUI 窗口与独立合成状态，先在卡片内部投递滚轮作为阳性对照，再检查分组间隙、左右内边距、上下内边距、滚动边界和展开的下拉框。只用 UIA 设置起点并读取稳定结果，滚动动作使用 SendInput；最后恢复窗口大小与指针位置。先验证未修复版本失败，再补透明背景。

## 历史检索

- [WinUI 模板陷阱](../../implemented/bug-fix/2026-10-17-windows-settings-winui-template-traps.md)记录其他布局和模板问题，不持有正文空白区域的命中规则，原决定保持。
- [Windows 原生设置归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)继续要求使用原生宿主验收。
- 未合并 PR #9083 的滚轮修复处理 Windows App SDK 的非活动窗口路由，与此处卡片内外差异不同。本篇不更新 SDK 或复制其未提交内容。
- 微软的 [Panel.Background](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.panel.background?view=windows-app-sdk-1.7) 和 [PointerWheelChanged](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.uielement.pointerwheelchanged?view=windows-app-sdk-1.7)文档说明默认空画刷及路由事件的命中边界；具体修复仍由真实输入对照证明。

## Alternatives considered

- 在窗口捕获并转发所有滚轮：可以统一所有空白区域，但会接管下拉框和内部滚动控件的原生处理，还需维护动画、边界与系统设置，所以采用显式命中背景。
- 为各个分组单独加背景：可覆盖组内间隙，但页面内边距和视口留白仍可能没有事件源，且每种组结构都要维护，因此使用共用页面与视口容器。

## Acceptance criteria

- 相同合成状态和真实滚轮输入下，卡片内部先能滚动，未修复版本在空白区域失败；修复版本在浅深色及两种宽度下通过全部区域与边界检查。
- 展开下拉框时滚轮不带动外层页面，原有 16 页界面验收继续通过。
- quick、笔记与 Windows 原生构建通过。

## Risks

原生输入验收需要交互桌面；SSH 会话没有登录桌面时只做脚本解析或编译，不声称滚轮已验收。透明画刷依赖 WinUI 的原生命中与模板契约，升级 SDK 后需要保留这项回归。
