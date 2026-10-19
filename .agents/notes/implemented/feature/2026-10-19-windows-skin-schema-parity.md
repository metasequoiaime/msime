# Agent Note: 跨平台候选皮肤清单与 msime-windows 对齐

Status: implemented

## Problem

`msime-windows` 固定提交 `c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的候选皮肤清单增加了 `wechat_scroll`、`canvas`、候选细分颜色、候选窗版式、系统材质和 v2 九宫格资源。共享 Rust 加载器此前只校验其中一部分，未保存的字段会被静默丢弃；Windows 原生宿主因此可能把能安装的皮肤画成默认外观，或者无法识别 `canvas` 这类基底。

## Decision

共享 `crates/client-services` 的 catalog 现在保留现有 v1 与动画契约，并解析 `canvas` / `wechat_scroll` 基底、完整候选颜色、候选字体、版式、材质、九宫格、insets、动画图片和自由挂件字段。扫描阶段统一执行数值范围、字体、资源路径、动画帧和挂件数量校验；`canvas` 通过 `SkinSummary.canvas` 传递，不扩大 `GlobalTheme`，`canvas` 与 `wechat_scroll` 也加入保留 ID 规则。`schema_version = 2` 对任何 base 都保留 [候选窗皮肤 v2 的有限动画契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md) 与 [装饰图的待机动效与 APNG 帧动画](2026-10-16-skin-decoration-idle-motion-and-apng.md)：`[animation]` 预设、装饰 `motion` / `motion_period_ms` 与 APNG 装饰帧照常生效，`SkinSummary.schemaVersion` 报的是清单声明的版本。msime-windows 的 v2 含义叠在这份契约之上：`[candidate_window.border_image]` 九宫格、`[[candidate_window.decorations]]` 自由挂件、content/outer insets 与 `line_gap_dip` 这些 canvas 专属字段只在 `base = "canvas"` 时生效；其他 base 上忽略它们并在 `SkinSummary.notices` 里记一条提示（对应 msime-windows 的 `package.notices`），不会因此关掉动效。`base = "canvas"` 配 `schema_version = 1` 直接拒绝。所有宿主都经过这一个共享扫描器或其 host API；能绘制完整字段的宿主读取 `SkinSummary`，Linux 等只支持公共子集的宿主通过 `host_candidate_catalog` 得到安全子集，不会各自复制一套 manifest 规则。

`SkinSummary` 继续使用 camelCase JSON。Windows、macOS、HarmonyOS、iOS 及共享设置页都从这份解析结果或 host API 结果消费候选颜色、装饰图、背景图、工具栏和固定动画资源；各宿主只绘制自己支持的字段。候选颜色覆盖预编辑文字与光标、选中候选文字、序号、释义、选中竖条和右键菜单；未声明或无法解析的颜色沿用主题默认值。颜色写法与 msime-windows 相同（`#RGB`、`#RGBA`、`#RRGGBB`、`#RRGGBBAA`、`rgb()`、`rgba()`、`transparent`），`SkinSummary` 按原样携带，所以 `theme::normalized_color` 的网页副本（`packages/ui/src/theme/global-theme.ts` 与 `packages/web-engine/src/skin.js` 的 `normalizedColor`）也接受 `#RGBA`，设置页预览和网页 SDK 不会丢掉原生宿主画得出的颜色。几何、材质、九宫格、insets 与自由挂件字段完成安全解析与传递；Web、[macOS 原生候选窗](2026-10-19-macos-candidate-canvas.md)、[Android 原生候选区](2026-10-19-android-candidate-canvas.md)和 [iOS 候选区](2026-10-19-ios-candidate-canvas.md)绘制 canvas 的九宫格、insets 与挂件，其他能力依各宿主的实际实现报告。

旧皮肤缺省字段保持原有主题和布局行为，未知或越界资源继续拒绝；Windows 不执行皮肤提供的脚本或任意动画，只接受共享层声明的固定动画预设。`canvas` 包由共享 catalog 按 msime-windows 的 `canvas` 出厂外观补色（不透明 `#202020` / `#FFFFFF`）；带九宫格面板时，包为面板贴图挑的候选颜色整套换成该外观，只保留强调色和右键菜单。Windows 宿主与其他宿主一样直接用这份结果，不再另走一条不带皮肤包的配色路径。`author`、`description` 写成空串时与 msime-windows 的 `ReadString(..., required = false)` 一样当作没写，共享用例表 `client_dialect.json` 的 `empty author` 改为接受，并新增 `empty description`。

## Web 贴图绘制与按能力回退

Web 候选栏接入 canvas 的九宫格面板、自由挂件、内容内边距、外留白和预编辑行间距。切片按源像素、边宽按 CSS px 解释；`stretchDip` 优先于 `sliceDip`，一、二、四值按 CSS 四边展开，`fill = false` 不画中心。挂件相对卡片的九个锚点定位，按 `zIndex` 稳定排序，小于 1 在面板下，其余在文字上，且一律不接收指针。留白参与视口夹紧，挂件不挤占候选文字布局。

共享 catalog 在 `panelCandidate` 保留补齐 canvas 默认值的贴图配色，`candidate` 继续提供未绘制贴图时的可读回退。支持贴图的宿主必须等面板成功加载后才切到 `panelCandidate`，失败或迟到的加载不得让旧贴图覆盖新皮肤。其他宿主照常消费 `candidate`，不能因 Web 接入贴图而先给它们换成可能不可读的文字色。

Web 面板使用宿主生成的固定 `border-image` 属性，挂件使用 `<img>`，保持浏览器的 APNG/GIF/WebP 帧播放和 `img-src` 限制。URL 和切片数值先校验，再通过 `style.setProperty` 设置；不执行包内 CSS、脚本或跨域抓取。裁切九张完整 `<img>` 也能复用浏览器动画，但切片放大时插值会采到相邻切片的颜色，像素验收发现中心混色，因此改用浏览器自己的九宫格采样。减弱动态效果时改画 canvas 首帧，系统选项变化时立即切换；跨域图片允许画进 canvas，但不读回像素、不导出图片，避免额外要求 CORS。挂件和面板加载使用当前皮肤的节点，换肤和销毁时撤销监听，异步结果不能重排已销毁的候选栏。

这一步只证明 Web 绘制能力；原生宿主和社区服务的缺口保持分别记录，不把 Web 浏览器验收外推为全平台完成。

## Alternatives considered

- **按 msime-windows 把非 canvas 的 v2 整体降级为 v1**：与来源的判定逐字一致，网页、原生宿主和 Windows 对同一个包的 v2 判断最简单。但本仓自 #7543 起 v2 就表示有限动画、装饰动效与 APNG 帧，而且不限 base；已发布的 `base = "system"` / `shuishan` / `night` / `fluent` 等 v2 皮肤，以及 MCP `make_skin` 提示词按 `schema_version = 2` + `base = "system"` 让模型生成的用户和社区皮肤，都会被悄悄去掉动效，因此不采用；只把来源 v2 里 canvas 专属的字段限定在 `canvas` 基底上。
- **只把新 ID 加入 Windows 外观常量**：改动最小，但清单新字段仍会被静默忽略，无法与来源的视觉协议兼容，因此不采用。
- **让 Windows 单独解析 TOML 字段**：可以更快让部分字段生效，但会复制安全校验并使其他宿主的扫描结果与 Windows 不一致，因此不采用。
- **一次性把全部新绘制原语接入 Direct2D**：视觉覆盖最完整，但会把协议迁移和渲染器重构绑在一起，难以独立验证；本次先保证字段安全、可传递并消费已有能力，后续按原语分别接入。

## Consequences

共享 catalog 与 Windows 之间的字段契约扩大，后续修改清单 schema 必须同步 Rust 模型、各平台 JSON 消费和对应测试。候选皮肤可以安全携带来源新增字段，Windows 已有渲染路径会显示更完整的颜色和字体效果，网页、macOS、Linux、HarmonyOS 与 iOS 仍通过同一份扫描规则读取公共字段，旧包不受影响。社区上传对新基底与空可选文字的保护由[社区画布资源按服务端能力上传](2026-10-19-community-canvas-resources.md) 部分取代：本地创建、打包与安装接受这些清单，真正上传前要求同源服务端明确声明扩展能力。MCP 描述与生成提示词同步说明资源规则；云端不支持时仍保留本机使用。

原生宿主的九宫格、材质、版式、insets 和自由挂件仍未全部绘制，仍与来源的完整视觉效果有差距；Windows 已经绘制的版式、外框、阴影档位、列表翻页箭头和收紧宽度见 [Windows 候选窗绘制皮肤的版式、外框、阴影、翻页箭头与收紧宽度](2026-10-19-windows-candidate-skin-layout-keys.md)，canvas 的九宫格、insets 与自由挂件见 [Windows 候选窗绘制 canvas 皮肤](2026-10-19-windows-candidate-canvas.md)；文档明确这一边界，避免把协议支持误报成绘制支持。图片边界现在对 PNG/JPEG/GIF/WebP 统一检查尺寸、动画帧和包内像素预算；网页宿主与原生宿主一样对任何 base 的 v2 清单启用动画，只有九宫格、自由挂件和 insets 依赖 `canvas`；网页 SDK 还与 catalog 一样拒绝没有 `schemaVersion: 2` 的 `base: "canvas"`，`canvas: true` 标记也不会覆盖全局主题或 Windows 外观基底。代价是 v2 在本仓与来源的含义不再逐字相同：同一个非 canvas v2 包在 msime-windows 上不播动画，在本仓播；以后来源若给非 canvas 的 v2 加新字段，需要逐项判断它属于哪一层。新增 JSON 和 C++ 字段也扩大了构建面，因此提交前运行皮肤单测、clippy、笔记校验、差异检查和 Windows x64 交叉构建。

验证结果（本笔记更新时实际运行）：`cargo fmt --all -- --check` 通过；`rbuild cargo test --locked -p msime-client-services skin::` 通过（262 项，含非 canvas v2 保留动效并忽略 canvas 专属字段、v1 + canvas 拒绝、空 `author` / `description` 打包拒绝和共享用例表）；`rbuild cargo test --locked -p msime-mcp-server` 通过（单测 69 项、stdio 11 项；最后一次修改描述后重跑，stdio 有一次因 Studio 上写入锁文件失败（`cannot record the write`）挂掉，单独重跑通过）；`rbuild cargo test --locked -p msime-host-api` 通过（847 项单测加各集成测试）；`rbuild cargo test --locked -p msime-desktop skin` 通过（17 项，需先在 Studio 上建空的 `target/macos/EngineResources` 与 `水杉输入法.app` 占位目录）；`rbuild cargo clippy --locked -p msime-client-services -p msime-mcp-server -p msime-host-api --all-targets -- -D warnings` 无警告；Windows 的 `candidate_skin`、`candidate_palette`、`candidate_skin_reload`、`candidate_theme_reload` 四个可移植单测在 Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过；`node --test 'packages/web-engine/test/*.test.mjs'` 通过（34 项，含 `#RGBA`）；`apps/desktop` 的 vitest `custom-theme-parity`、`external-skins`、`community-candidate-skins` 通过（132 项）；`platforms/harmony/tests/run.sh` 通过（`keyboard-logic` 4395 条断言）；`pnpm run verify-notes` 与 `git diff --check` 通过。用 msime-windows `c0a9d371` 的 `skins/` 实测共享 catalog：13 个外部皮肤全部接受（`green_leaf`、`sakura-night` 为 v2 canvas，带宿主绘制能力提示），8 个内置外观以原 ID 放进皮肤目录按保留 ID 拒绝，改名并补上外部皮肤必需的 `version`、`base`、`supports` 后全部接受。没有运行：`platforms/windows/build-cross.sh x64`（Studio 没有 MinGW，交给 PR 上的 Windows CI）、macOS `candidate-panel` 与 `candidate-row-layout`；也未在真实 Windows 编辑器中目测所有新绘制原语。

Web 绘制切片的后续验证：本机 `node --test 'packages/web-engine/test/*.test.mjs'` 36 项通过；`CHROME_BIN=... node packages/web-engine/test/candidates.browser.mjs` 通过，截图像素断言覆盖九宫格四角/边缘/中心、挂件前后层级、裁切、点击穿透、留白定位、图片失败与过期回调，以及 APNG/GIF 面板和挂件的真实帧变化。减弱动态效果时首帧保持不变，实时切换系统偏好和无 CORS 响应头的跨域图片同样通过。固定上游 `c0a9d371` 的 `green_leaf` 与 `sakura-night` 实际图片在浅/深色与横/竖排的八种组合中显示正常；这些源资源只用于本机验收，没有复制进本仓库。类型声明检查、`cargo fmt --all -- --check`、本机 services 皮肤测试 262 项、MCP 单测 69 项与 stdio 11 项、services/MCP 的 `clippy --all-targets -- -D warnings` 均通过。以上只覆盖 Web 贴图绘制与共享回退契约，不代表其余原生宿主具备完整贴图能力。
