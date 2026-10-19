# Agent Note: 玻璃必须承载实际内容

Status: implemented

## Problem

上一轮把 `NSGlassEffectView` 放在控件后面作为兄弟视图，测试只断言材质存在，真实运行的悬浮工具栏截图却没有按钮。[Apple 的 AppKit 新设计说明](https://developer.apple.com/videos/play/wwdc2025/310/)明确要求通过 `contentView` 承载内容，以便系统应用可读性处理，并明确反对背景兄弟视图的用法。

## Decision

工具栏、HUD、候选卡片与设置 WebView 都把实际内容放入玻璃的 `contentView`，回退时恢复普通容器。候选保留独立内容容器与装饰带；点击、键盘选词、读屏和动效都从同一个当前内容容器取按钮，继续拒绝旧页面按钮。工具栏内容只弱引用玻璃父视图，避免内容与材质互相持有。玻璃使用系统材质透明度，不再让整棵内容随候选底板透明度变淡。

## Historical audit

[候选](../../implemented/feature/2026-10-19-macos-candidate-liquid-glass.md)、[设置](../../implemented/feature/2026-10-19-macos-settings-liquid-glass.md)与[悬浮表面](../../implemented/feature/2026-10-19-macos-floating-surfaces-liquid-glass.md)的兄弟背景决定由本次修正取代，主题启用与旧系统回退边界保留。

## Alternatives considered

保留兄弟关系只修图层顺序，改动更小且保留原按钮父视图，但绕过系统内容可读性处理，仍与官方 API 使用方式冲突。采用实际内容容器，并同步所有候选身份校验与遍历路径。

## Verification

- 六组原生测试通过：`shortcut`、`settings-glass`、`candidate-hover`、`candidate-row-layout`、`input-mode-hud-panel`、`floating-toolbar-panel`。覆盖真实内容归属、非空尺寸、按钮命中、旧候选按钮拒绝、回退与重新启用；工具栏覆盖两种材质下的全部 128 种按钮组合。
- 工具栏释放回归测试在强引用版本稳定失败，改为弱引用后通过，确保移除宿主后内容与玻璃均释放。
- 本机安装后的真实工具栏从空白底板恢复为完整按钮；真实五笔输入显示预编辑、候选与翻页。设置窗口截图确认正文、平铺导航与圆角玻璃容器，未恢复导航分类。
- `shortcut-test --glass-visual` 通过生产 `InputController` 偏好入口和中英文切换入口显示真实原生 HUD，两种模式均截图确认标签和玻璃正常。该项使用合成输入客户端，不等同于物理 Shift 键经过完整 IMK 会话的验证。
- 候选外观前端测试 28 项、类型检查和生产构建通过。系统玻璃模式把透明度标为「跟随系统」，自定义主题与材质关闭后保留原有数值及可编辑能力。
- 最终安装的设置窗口已重新截图，并通过真实侧栏点击打开候选设置页，确认 WebView 点击与无障碍树正常。共享设置继续使用 React 控件与原生玻璃内容容器，不宣称已替换为系统原生表单控件，也不扩展冻结的 AppKit 回退。

- 最终 `bash scripts/verify-local.sh --quick`、`pnpm run verify-notes`、前端改动文件格式与 `git diff --check` 均通过。quick 包含 macOS 原生编译、Rust workspace、Android 目标／Java 检查、WebAssembly、Linux 设置壳与原生 90 项测试、Windows 管道双架构交叉编译；完整 Windows 宿主与 HarmonyOS 阶段缺前置条件，未覆盖。
- 修正版输入法、设置应用及内嵌输入法已安装，严格代码签名验证通过；系统主题、HUD 开关、五笔输入源与原有两份自定义候选皮肤槽位保留。

## Consequences

系统玻璃的透明度由系统控制，设置页禁用该模式下的手动滑块并解释原因；自定义主题保留独立设置。不能用整个玻璃视图的 `alphaValue` 模拟仅底板透明，否则候选文字也会淡化。旧系统实机仍不在本机覆盖面内。

设置内部控件与原生背景外观同步由[WebKit 系统控件](../feature/2026-10-19-macos-settings-system-controls.md)补齐；共享 React 表单归属不变。
