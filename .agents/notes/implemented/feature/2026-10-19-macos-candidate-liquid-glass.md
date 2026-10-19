# Agent Note: macOS 系统候选卡片使用原生液态玻璃

Status: implemented

## Problem

macOS 候选卡片由 `CandidateChrome.h` 自绘底色，即使用 macOS 26 SDK 构建，也不会自动获得液态玻璃。用户希望新系统上的候选窗使用系统材质，同时仍能在 macOS 13 起运行。

## Decision

此处最初采用的兄弟背景层方案已被[玻璃内容归属修正](../bug-fix/2026-10-19-macos-glass-content-containment.md)取代：当前实际控件进入玻璃的 `contentView`，回退时恢复普通容器。下述初始结构取舍保留用于说明错误来源；主题启用、兼容与宿主归属边界仍然成立，最新运行验证见修正笔记。

在实际 IMK 入口 `InputController.mm` 的普通候选与展开网格中，为未覆盖皮肤的 `system` 主题启用 `NSGlassEffectView`。玻璃放在卡片的背景层，候选按钮仍属于原来的内容视图，保留点击校验、无障碍、选中动画和定位契约。玻璃只覆盖卡片，不覆盖上方装饰带。

运行时用 `@available(macOS 26.0, *)` 守住新 API；旧系统、系统开启「降低透明度」、自定义主题与皮肤包都使用原有绘制。候选透明度只作用于背景，不淡化文字与高亮。系统辅助功能设置变化时刷新效果，不要求重启输入法。

### 自动明暗与玻璃文字对比度

macOS 26 起，`system` 主题、全局 `system` 与候选 `follow` 同时成立时，普通候选和展开网格利用既有 `attributesForCharacterIndex:lineHeightRectangle:` 返回的背景色判断输入区明暗；没有有效背景时只参考中性前景色，彩色语法高亮、透明色、无效色均不作依据。没有可靠颜色就清除上一个客户端的覆盖，回到系统外观。手动明暗与自定义主题仍优先，不按应用名称猜测背景，不额外读取文档、不截屏、不增加每次渲染的客户端往返，也不把颜色保存到配置。

原生玻璃启用时单独覆盖序号、释义与页码的颜色；浅色使用不透明深灰，深色使用浅灰。深色亮绿选中底上改用深字，浅色深绿底保留白字，避免白字落在亮绿上对比度不足。基础 token、皮肤包、减少透明度与旧系统保留原配色；共享预览同步覆盖。

该机制依赖客户端提供的颜色信息，不能宣称覆盖所有自绘终端。只提供光标矩形的宿主仍回退系统明暗；该限制必须在验收中报告，不能用合成颜色测试冒充该终端的真实支持。

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

- 本轮自动明暗改动：`candidate-skin`、`shortcut`、`candidate-row-layout`、`candidate-hover` 四项通过，相关前端 58 项测试、类型检查、生产构建与 quick 门禁通过；Developer ID 签名安装原生宿主及设置应用，嵌入宿主同步替换，内外 bundle 的严格签名校验通过。设置页已恢复 `candidate_theme=follow`，保留系统主题与皮肤槽位。
- 生产候选渲染器的深浅配色截图已检查，来源为合成客户端属性。真实 AppKit 编辑器经系统选中五笔／中文源后，自动化按键仍直出拉丁字母、未唤起候选，不能证明真实 IMK 配色传递或用户终端自动明暗已通过；没有操作用户终端内容。
- 自动明暗回归覆盖深→浅→深、只有前景、无颜色后清除旧覆盖、背景优先、透明／无效／彩色前景、手动全局及候选明暗优先，以及普通候选和展开网格；每次渲染仍只有一次既有客户端属性读取。玻璃选中行和次要文字增加 4.5:1 对比度断言。

## Consequences

旧系统只能通过 availability 编译检查，当前开发机无法覆盖其实际显示。自定义主题保持原貌；共享 React 设置页由独立窗口宿主接入，见[设置窗口玻璃](2026-10-19-macos-settings-liquid-glass.md)。
