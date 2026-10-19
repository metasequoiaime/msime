# Agent Note: macOS 悬浮工具栏和模式 HUD 使用原生玻璃

Status: implemented

## Problem

候选窗和设置页使用玻璃后，悬浮工具栏与中英文模式 HUD 仍然自绘不透明底色，系统主题在同一输入会话里呈现两套材质。

## Decision

此处最初采用的兄弟背景层方案已被[玻璃内容归属修正](../bug-fix/2026-10-19-macos-glass-content-containment.md)取代：当前实际控件进入玻璃的 `contentView`，回退时恢复普通容器。下述初始结构取舍保留用于说明错误来源；主题启用、兼容与宿主归属边界仍然成立，最新运行验证见修正笔记。

系统主题通过实际 `InputController` 偏好应用入口，同时启用工具栏和 HUD 的原生 `NSGlassEffectView`。共用背景辅助函数，按钮和标签继续位于原内容视图；旧系统、减少透明度模式和自定义主题保留原皮肤。窗口的非激活、拖动、显示时间和鼠标穿透策略不变。

## Historical audit

- [外观精简](../../implemented/simplification/2026-10-16-macos-appearance-settings-prune.md)：部分重叠，继续统一跟随颜色模式，不恢复独立工具栏明暗设置。
- [候选玻璃](../../implemented/feature/2026-10-19-macos-candidate-liquid-glass.md)：部分重叠，复用相同兼容政策，候选的装饰带仍有独立几何处理。
- [设置玻璃](2026-10-19-macos-settings-liquid-glass.md)：部分重叠，窗口所属进程不同，设置仍由共享页面承载。
- 检索命中的停用配对符号、读屏高亮和 Windows 模式提示不涉及本材质变更。

## Alternatives considered

- 替换整个内容视图能直接把按钮放入玻璃的 `contentView`，但会改变现有布局和测试的视图归属；只替换背景可以保持输入与点击路径。
- 半透明颜色支持旧版系统，但不能呈现原生折射，因此只作为回退。

## Verification

工具栏与 HUD 的原生测试通过，覆盖背景创建和移除、尺寸与非激活／鼠标穿透策略。实际宿主已构建并安装到本机，严格签名验证通过。尚未取得系统输入会话中工具栏和 HUD 的可复核截图。

## Consequences

共用背景函数保持两个窗口的材质策略一致，原控件和输入行为继续由各自窗口维护。本机只能验证当前系统；macOS 13 的可运行性通过弱链接和 availability 保证，仍缺旧系统实机。
