# Agent Note: macOS 系统候选卡片使用原生液态玻璃

Status: implemented

## Problem

macOS 候选卡片由 `CandidateChrome.h` 自绘底色，即使用 macOS 26 SDK 构建，也不会自动获得液态玻璃。用户希望新系统上的候选窗使用系统材质，同时仍能在 macOS 13 起运行。

## Decision

在实际 IMK 入口 `InputController.mm` 的普通候选与展开网格中，为未覆盖皮肤的 `system` 主题启用 `NSGlassEffectView`。玻璃放在卡片的背景层，候选按钮仍属于原来的内容视图，保留点击校验、无障碍、选中动画和定位契约。玻璃只覆盖卡片，不覆盖上方装饰带。

运行时用 `@available(macOS 26.0, *)` 守住新 API；旧系统、系统开启「降低透明度」、自定义主题与皮肤包都使用原有绘制。候选透明度只作用于背景，不淡化文字与高亮。系统辅助功能设置变化时刷新效果，不要求重启输入法。

## Historical audit

- [跨平台候选渲染边界](../../implemented/architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：部分重叠，保留原生宿主职责；只修改产品使用的 `MSIMECandidateChromeView`，不改测试专用 `CandidatePanel.mm`。
- [候选显隐动效](../../implemented/feature/2026-10-16-macos-candidate-declared-motion.md)：部分重叠，背景跟随现有卡片图层，不额外增加动画时间线。
- [macOS 构建工具链](../../implemented/process/2026-10-09-macos-ci-and-release-on-xcode-26.md)：部分重叠，新 API 已在所用 SDK 中，最低部署版本仍为 13.0。
- 搜索命中的 iOS 原生皮肤与热力图笔记属于其他平台，无关。

## Alternatives considered

- **给整张候选卡片换成玻璃父视图**：所有内容能直接进入玻璃的 `contentView`，层级最直观。但产品多处依赖按钮直接属于 `panel.contentView` 来拒绝旧页面点击，并据此遍历高亮、读屏和动画。保留原有结构，把玻璃作为背景子视图更小，也不改变输入路径。
- **用半透明填色或 `NSVisualEffectView` 模拟**：兼容旧系统且容易复用已有颜色，但不能提供用户要求的原生液态玻璃；只保留已有绘制作为回退。
- **对所有皮肤启用玻璃**：统一观感最容易，但会改变皮肤作者指定的底色、图片与边框，因此只启用未覆盖的系统主题。

## Verification

- `bash scripts/verify-local.sh --quick` 通过，包含完整 macOS 原生目标编译、Rust workspace、Android 目标与 Linux 设置壳编译。
- `candidate-hover`、`candidate-row-layout`、`candidate-skin`、`external-skin` 四项通过。覆盖原生背景位于候选下方、避开装饰带、跟随窗口尺寸／圆角／背景透明度、按钮归属和鼠标命中不变、关闭材质后移除效果视图，以及实际产品入口中的系统／自定义主题切换和展开网格策略。
- `nm -m` 确认 `NSGlassEffectView` 为弱链接符号，macOS 13 的部署目标不变。
- 本机用 Developer ID 签名安装，`codesign --verify --deep --strict` 和输入源检查通过；五笔、中文、日文模式可选。按用户选择经 Host API 切换系统主题，比较返回的偏好快照，确认仅 `global_theme` 改变，自定义皮肤槽位保留。
- 真实编辑器的视觉与输入提交验收未完成：EveryAPI 的窗口自动化对临时编辑器及 TextEdit 返回 `window_not_found`，没有可复核的候选窗截图。原生渲染集成测试不能替代这项检查。

## Consequences

旧系统只能通过 availability 编译检查，当前开发机无法覆盖其实际显示。自定义主题保持原貌；共享 React 设置页由独立窗口宿主接入，见[设置窗口玻璃](2026-10-19-macos-settings-liquid-glass.md)。
