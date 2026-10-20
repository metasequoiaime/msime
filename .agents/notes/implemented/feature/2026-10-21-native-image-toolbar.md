# Agent Note: 原生宿主图片工具栏

Status: implemented

## Problem

[图片工具栏共享契约](../../implemented/feature/2026-10-21-image-toolbar-compatibility.md) 已保留清单和资源，但 macOS、Windows 与 HarmonyOS 2in1 的悬浮窗仍按普通条形工具栏绘制。同一皮肤的按钮坐标、状态图和背景外按钮因此没有落实到可操作的系统界面。

## Decision

三个已有桌面悬浮窗消费共享布局桥，保留原有按钮动作与用户开关。图片画布与普通条形工具栏分开，切回普通模式完整恢复原生条形界面；图片模式不绘制普通条的底色、阴影、边框、把手和分隔线。缓存图片与布局，状态变化只选图和重新布局，不扫描整个包。按钮矩形优先命中，背景不透明区域用于拖动，透明区域穿透；背景命中使用所有动画帧 alpha 的并集，读不出时按背景矩形回退。

macOS 在独立画布上复用原有按钮的 target/action、标签和系统图标，避免破坏普通条的自动布局。窗口保持不激活输入客户端，图片动画遵守系统减弱动态效果。Windows 的绘制、点击、提示、读屏和菜单锚点必须使用相同的按钮矩形。HarmonyOS 只在现有 2in1 桌面工具窗绘制图片，触屏键盘入口保持自身所有权。Android、iOS 保留键盘内工具入口，Linux 保留框架菜单，不创建新的桌面悬浮窗。

## Related notes audit

- [图片工具栏共享契约](../../implemented/feature/2026-10-21-image-toolbar-compatibility.md)：部分重叠，沿用字段校验、资源预算和状态选择，本篇只持有原生窗口与输入边界。
- [Windows 工具栏对齐](../../implemented/feature/2026-10-10-windows-floating-toolbar-mac-parity.md)：普通条的开关、动作和位置持久化仍有效，图片模式只替换呈现和命中。

## Alternatives considered

直接重排普通条的按钮能少建一层视图，但普通模式依赖自动布局、拖动把手和主题材质，图片模式频繁切换会同时改动这些约束。独立画布保留两种模式的结构，并复用既有动作。用 React 或 WebView 承载图片工具栏可以沿用预览，但会改变原生输入法宿主、焦点与窗口命中边界，因此不采用。

## Verification

原生测试覆盖状态图、越背景按钮、隐藏、用户开关、缩放、透明命中和普通模式恢复；macOS、Windows 交叉构建与 HarmonyOS HAP 编译均完成。Windows Wine 窗口测试在无头环境中未完成，HarmonyOS 2in1 HAP 已编译但设备安装验收未完成；透明区的系统级穿透仍需在可用桌面/设备上继续验证。模拟器、跨目标构建与真实编辑器证据分开记录，不把共享预览算作原生验收。

## Consequences

透明窗口的系统穿透与 UI 内部 hit-test 不等价，必须检查底层窗口收到点击。各平台解码器对 GIF、WebP 和 APNG 的支持不同，无法解码要可读回退，不能留下看不见的按钮。原生验证未结束前保持 proposed。
