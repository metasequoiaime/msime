# Agent Note: macOS 设置使用 WebKit 系统控件

Status: implemented

## Problem

设置 WebView 进入原生玻璃后，开关、复选框和滑块仍由 CSS 绘制固定形状，不能自动获得当前系统的控件外观与交互。系统主题的候选预览也还显示旧实色底板，与实际候选不同。强制深色时 HTML 文字切换，但原生玻璃仍是浅色，造成文字与背景不同步。

## Decision

macOS 26 起的设置宿主独立确认系统控件能力；减少透明度只关闭玻璃，不关闭系统控件。共享页面使用 [WebKit 官方支持的系统开关](https://webkit.org/blog/15054/an-html-switch-control/)： `input[type=checkbox][switch]` 系统开关，并让复选框和滑块恢复系统绘制，沿用当前值、键盘、读屏和偏好回调。其他平台与没有能力标记的浏览器沿用现有控件。系统主题候选预览透出窗口材质，自定义主题保持自身底板；预览仍是固定样例，不声称它是原生候选窗口。

设置页通过 `data-settings-appearance` 发送原始 `system/light/dark` 意图，表面独立覆盖优先于全局偏好。原生主框架观察该属性，并按 [AppKit 的外观继承规则](https://developer.apple.com/documentation/appkit/choosing-a-specific-appearance-for-your-macos-app)设置窗口外观；`system` 清除窗口覆盖。不能只同步解析后的明暗，否则切回系统会把上次明暗固化，系统变化不能继续传播。材质和系统表单控件共享窗口外观；辅助功能回退也保持同步。主题卡片的系统候选与工具栏样例按独立预览明暗叠加薄色层，次要文字采用玻璃模式的高对比颜色。

原生外观消息会重新读取“减少透明度”，因此测试里消息之前的显式启用不能保证消息之后玻璃仍然存在。`settings-glass` 在本测试进程内替换系统读取，分别覆盖开、关两种策略，先检查消息刷新后的玻璃存在性，再显式启用并检查深色继承；不修改测试机器的辅助功能设置，也不绕过产品回退。

真实 WebView 的回归测试先通过 `WKNavigationDelegate` 等到本次 `loadHTMLString` 的导航完成，再观察能力标记和切换外观。能力标记由原生刷新写入，不能证明当前读到的文档已经完成导航、安装了外观观察器；沿用旧文档的标记会让后续外观修改与新文档提交竞争。外观断言同时等待 JavaScript 求值完成，失败时报告合成模式、期望值、窗口实际值和脚本错误，不延长现有超时或改动产品同步逻辑。

## Historical audit

- [玻璃内容归属](../../implemented/bug-fix/2026-10-19-macos-glass-content-containment.md)：部分重叠，原生内容容器已正确，本篇补充内部控件；保持原有输入与层级边界。
- [设置玻璃](../../implemented/feature/2026-10-19-macos-settings-liquid-glass.md)：部分重叠，共享设置宿主归属不变，不扩展冻结的 AppKit 回退。
- [候选预览 token](../../implemented/bug-fix/2026-10-11-macos-candidate-preview-native-tokens.md)：部分重叠，几何与选中行 token 保留，只覆盖原生玻璃模式的系统底板。
- 检索命中的 Windows 布局和 HarmonyOS 输入行是其他平台的控件决定，不适用。

## Alternatives considered

- CSS 绘制玻璃滑块和开关可以控制全部外观并统一跨平台，但仍不能获得系统控件跟随系统版本与辅助功能的处理，因此当前 WebKit 能力下恢复系统绘制。
- 重写 AppKit 设置能获得整套原生表单，但会扩展已冻结的回退并复制共享业务，不符合现有设置归属；使用 WebKit 系统控件保持现有架构。

## Verification

- 5 个前端测试文件 58 项通过，涵盖系统开关能力变化、值与回调、关闭玻璃保留控件、原始外观意图和系统媒体查询后续变化。类型检查与生产构建通过。
- 6 组原生测试通过：`settings-glass`、`shortcut`、`candidate-hover`、`candidate-row-layout`、`input-mode-hud-panel`、`floating-toolbar-panel`。真实 `WKWebView` 验证控件能力与 WebKit 的 `switch` 支持，强制明暗到达原生窗口与玻璃，切回系统和重载均清除覆盖，禁用玻璃仍可同步外观。
- [PR #8768 的 macOS CI](https://github.com/metasequoiaime/msime/actions/runs/39200835396/job/117893341243) 中唯一失败是玻璃深色断言。本机在“减少透明度”开启时复现相同断言，修正前置条件后以 `clang++ -Wall -Wextra -Werror -UNDEBUG` 构建并运行真实 `WKWebView` 测试，两种透明度策略均通过。
- [PR #9690 的 macOS CI](https://github.com/metasequoiaime/msime/actions/runs/40179797297/job/121128277717) 中 184 项仅 `settings-glass` 在文档外观同步断言失败，暴露测试缺少明确的导航完成条件；导航和求值错误现在分别报告，避免把页面尚未准备好的失败都归成原生外观同步错误。
- 同源远端构建中，修复前连续 20 次未复现 CI 的偶发失败；补齐导航与求值完成条件后，专项连续 20 次通过（109.75 秒），覆盖两种减少透明度策略。该结果证明新增等待条件可运行，不将它表述为已经稳定复现过 CI 的原始时序。
- HUD 回归从 `handleEvent:client:` 发送左右 Shift 按下与松开，验证英／中、材质内容归属和关闭 HUD 后不弹窗；这是合成客户端经过生产事件处理器，两种模式均已截图检查标签与玻璃。未覆盖物理键盘进入完整 IMK 会话。
- 本机设置应用完成构建、打包、签名和安装；实际开关、复选框和滑块均操作并确认持久化，随后恢复原值。实际窗口检查明暗玻璃、系统候选／工具栏样例、平铺导航；颜色模式恢复 `system`，自定义皮肤槽位保留。

- `verify-local.sh --quick`、`verify-notes`、改动前端文件格式和 `git diff --check` 通过。quick 包含 macOS 原生、Rust、Android 编译／Java、WebAssembly、Linux 设置壳及原生 90 项测试、Windows 管道双架构交叉编译；完整 Windows 宿主与 HarmonyOS 原生工程缺前置条件，未覆盖。最终设置与输入法均通过严格签名校验，当前五笔输入源保持选中。

## Consequences

WebKit 控件不等同于 AppKit 表单；分段选择器继续使用共享语义控件。系统版本决定其实际外观，当前机器不能代替旧系统实机。真实单独 Shift 的完整系统路径仍未验收，不把合成事件测试写成物理键盘验收。新增文档属性只承载窗口外观意图，不改变偏好存储格式；其他平台不安装该消息桥。
