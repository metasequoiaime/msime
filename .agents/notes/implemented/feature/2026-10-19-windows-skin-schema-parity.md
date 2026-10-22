# Agent Note: 跨平台候选皮肤清单与 msime-windows 对齐

Status: implemented

## Problem

`msime-windows` 固定提交 `c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的候选皮肤清单增加了 `wechat_scroll`、`canvas`、候选细分颜色、候选窗版式、系统材质和 v2 九宫格资源。共享 Rust 加载器此前只校验其中一部分，未保存的字段会被静默丢弃；Windows 原生宿主因此可能把能安装的皮肤画成默认外观，或者无法识别 `canvas` 这类基底。

## Decision

共享 `crates/client-services` 的 catalog 现在保留现有 v1 与动画契约，并解析 `canvas` / `wechat_scroll` 基底、完整候选颜色、候选字体、版式、材质、九宫格、insets、动画图片和自由挂件字段。扫描阶段统一执行数值范围、字体、资源路径、动画帧和挂件数量校验；`canvas` 通过 `SkinSummary.canvas` 传递，不扩大 `GlobalTheme`，`canvas` 与 `wechat_scroll` 也加入保留 ID 规则。`schema_version = 2` 对任何 base 都保留 [候选窗皮肤 v2 的有限动画契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md) 与 [装饰图的待机动效与 APNG 帧动画](2026-10-16-skin-decoration-idle-motion-and-apng.md)：`[animation]` 预设、装饰 `motion` / `motion_period_ms` 与 APNG 装饰帧照常生效，`SkinSummary.schemaVersion` 报的是清单声明的版本。msime-windows 的 v2 含义叠在这份契约之上：`[candidate_window.border_image]` 九宫格、`[[candidate_window.decorations]]` 自由挂件、content/outer insets 与 `line_gap_dip` 这些 canvas 专属字段只在 `base = "canvas"` 时生效；其他 base 上忽略它们并在 `SkinSummary.notices` 里记一条提示（对应 msime-windows 的 `package.notices`），不会因此关掉动效。`base = "canvas"` 配 `schema_version = 1` 直接拒绝。所有宿主都经过这一个共享扫描器或其 host API；能绘制完整字段的宿主读取 `SkinSummary`，Linux 等只支持公共子集的宿主通过 `host_candidate_catalog` 得到安全子集，不会各自复制一套 manifest 规则。

`SkinSummary` 继续使用 camelCase JSON。Windows、macOS、HarmonyOS、iOS 及共享设置页都从这份解析结果或 host API 结果消费候选颜色、装饰图、背景图、工具栏和固定动画资源；各宿主只绘制自己支持的字段。候选颜色覆盖预编辑文字与光标、选中候选文字、序号、释义、选中竖条和右键菜单；未声明或无法解析的颜色沿用主题默认值。颜色写法与 msime-windows 相同（`#RGB`、`#RGBA`、`#RRGGBB`、`#RRGGBBAA`、`rgb()`、`rgba()`、`transparent`），`SkinSummary` 按原样携带，所以 `theme::normalized_color` 的网页副本（`packages/ui/src/theme/global-theme.ts` 与 `packages/web-engine/src/skin.js` 的 `normalizedColor`）也接受 `#RGBA`，设置页预览和网页 SDK 不会丢掉原生宿主画得出的颜色。几何、材质、九宫格、insets 与自由挂件字段完成安全解析与传递；Web、[macOS 原生候选窗](2026-10-19-macos-candidate-canvas.md)、[Android 原生候选区](2026-10-19-android-candidate-canvas.md)和 [iOS 候选区](2026-10-19-ios-candidate-canvas.md)绘制 canvas 的九宫格、insets 与挂件，其他能力依各宿主的实际实现报告。

旧皮肤缺省字段保持原有主题和布局行为，未知或越界资源继续拒绝；Windows 不执行皮肤提供的脚本或任意动画，只接受共享层声明的固定动画预设。`canvas` 包由共享 catalog 按 msime-windows 的 `canvas` 出厂外观补色（不透明 `#202020` / `#FFFFFF`）；带九宫格面板时，包为面板贴图挑的候选颜色整套换成该外观，只保留强调色和右键菜单。Windows 宿主与其他宿主一样直接用这份结果，不再另走一条不带皮肤包的配色路径。`author`、`description` 写成空串时与 msime-windows 的 `ReadString(..., required = false)` 一样当作没写，共享用例表 `client_dialect.json` 的 `empty author` 改为接受，并新增 `empty description`。

## 装饰图候选词叠加契约

共享解析器参考 `msime-windows` 固定提交 `0978b5c28b2a1806e7ef3acdbc3a1c87e4784823` 的 `ReadDecorationCandidate`，读取 `[candidate_window.decoration]` 下的 `candidate_overlay_*`，v1 与 v2 均可用。`candidate_overlay` 缺省或为 `false` 时完全忽略其余键；启用时必须有可用的装饰图片与正尺寸，保留本仓原有的预览图回退。

`SkinSummary.decorationCandidate` 只在启用时输出，字段使用 camelCase。`index` 默认 1、允许整数 0–100，0 跟随高亮，其余按当前页 1 起编号，越界时不显示。初始／最小字号默认 16／8、范围 `(0,200]`；解析不夹紧两者，绘制时以较小者作为最小字号。最大宽默认 0、范围 0–2000，0 使用装饰框宽；最小字号仍放不下时允许超框且不换行。水平／垂直默认居中，偏移为有限数 ±2000，`stripBadge` 默认 `true`。颜色保留来源的 80 字节 ASCII 白名单声明，同时输出参考来源 `ParseCandidateCssColor` 的八位 `resolvedColor`，统一无 `#` 的十六进制、浮点 RGB 与白色回退，原始 `color` 保留给编辑和导出；不能以现有更窄的 `normalized_color` 拒绝来源接受的清单。

MCP 列表在装饰对象的可选 `candidate_overlay` 中返回同一声明，使用独立 DTO 提供输出 schema，不向共享服务层增加 MCP 的 schema 依赖。旧 `host_candidate_catalog` 条目不增加字段，严格的 `ThemePackage::from_host_catalog_entry` 继续可读；原生新能力应读完整的 `SkinSummary`。本节的共享解析与 MCP 列表支持不代表宿主已经绘制叠加文字，也不代表六平台设备验收。最新来源还新增图片工具栏、按钮状态贴图与自动排列，各原生宿主仍需分别核对绘制缺口，共享预览的支持范围见[canvas 预览](2026-10-21-canvas-skin-card-preview.md)。不能因八份出厂 TOML 没变化就报告上游无变化。

2026-10-21 再次核对远端 `develop@3aa15daf73a029243bbf6ec143c2089c2ccfe544`：相对本篇装饰词协议来源只有安装器修复和布朗熊演示皮肤两次提交，没有变更装饰词解析或绘制协议。新演示使用既有图片工具栏、按钮背景和自动排列，其原生宿主兼容性与共享预览实现分别报告；本地仅审计清单，不把授权未核实的演示图片并入产物。

## Web 贴图绘制与按能力回退

Web 候选栏接入 canvas 的九宫格面板、自由挂件、内容内边距、外留白和预编辑行间距。切片按源像素、边宽按 CSS px 解释；`stretchDip` 优先于 `sliceDip`，一、二、四值按 CSS 四边展开，`fill = false` 不画中心。挂件相对卡片的九个锚点定位，按 `zIndex` 稳定排序，小于 1 在面板下，其余在文字上，且一律不接收指针。留白参与视口夹紧，挂件不挤占候选文字布局。

共享 catalog 在 `panelCandidate` 保留补齐 canvas 默认值的贴图配色，`candidate` 继续提供未绘制贴图时的可读回退。支持贴图的宿主必须等面板成功加载后才切到 `panelCandidate`，失败或迟到的加载不得让旧贴图覆盖新皮肤。其他宿主照常消费 `candidate`，不能因 Web 接入贴图而先给它们换成可能不可读的文字色。

Web 面板使用宿主生成的固定 `border-image` 属性，挂件使用 `<img>`，保持浏览器的 APNG/GIF/WebP 帧播放和 `img-src` 限制。URL 和切片数值先校验，再通过 `style.setProperty` 设置；不执行包内 CSS、脚本或跨域抓取。裁切九张完整 `<img>` 也能复用浏览器动画，但切片放大时插值会采到相邻切片的颜色，像素验收发现中心混色，因此改用浏览器自己的九宫格采样。减弱动态效果时改画 canvas 首帧，系统选项变化时立即切换；跨域图片允许画进 canvas，但不读回像素、不导出图片，避免额外要求 CORS。挂件和面板加载使用当前皮肤的节点，换肤和销毁时撤销监听，异步结果不能重排已销毁的候选栏。

这一步只证明 Web 绘制能力；原生宿主和社区服务的缺口保持分别记录，不把 Web 浏览器验收外推为全平台完成。

## Web 装饰词绘制

网页候选栏按 `decorationCandidate` 从当前页取词，以独立的 `badge` 字段追加可选标注，绝不通过正则删掉原文的括号或后缀。文字通过 `textContent` 绘制，不接收点击，也不重复向辅助技术朗读；用声明的装饰宽和高对齐与偏移，不以图片透明像素或等比缩小后的图像尺寸代替框。初始字号实测超宽后按比例缩小，最小字号仍超宽时保留完整单行。网页尚不播放装饰待机变换；叠加文字与图片都跟随候选栏本身的显隐动效，不宣称新增了待机动画能力。

每次帧更新、换肤、布局和明暗切换重取当前词，隐藏、越界与图片失败立即清理旧词，图片迟到加载只对当前帧重新布局。解析器对网页直接传入的皮肤对象再次检查类型、范围与安全颜色声明，支持来源的无 `#` 十六进制和浮点 RGB 通道，未知安全值回退白色。代价是网页直接对象的颜色校验保留一份独立实现，来源变化时须与 Rust 用例同步。

## macOS 与共享编辑器的装饰词绘制

macOS 从完整 `SkinSummary` 读取声明和 `resolvedColor`，用 `shared/contracts/candidate_decoration_overlay.h` 的有界样式、选词与几何计算绘制原生文字。文字作为既有装饰图视图的子视图，跟随待机和帧动画所在的父变换，不拦截点击、不重复向辅助技术朗读；装饰图视图显式关闭边界裁切，最小字号仍过宽时保留完整单行。图片等比缩小后，文字仍按声明框定位，候选缩放同时作用于字号、宽度和偏移。

普通候选窗、释义更新复用路径和展开网格均刷新文字；与固定来源的 `SetDecorationCandidates` 一样，展开时仍取普通当前页的候选及高亮，不把后续网格光标当作当前页序号。原文与云／AI 标注分别取值，`stripBadge` 只控制独立标注，不删除原文中的括号或后缀。macOS 的装饰词与普通候选共用来源徽标函数，AI 来源遵守当前 robot／whale 样式及未初始化缓存的出厂回退；皮肤关闭来源标注时仍完全隐藏它。固定排位徽标只在普通候选追加，不混入装饰词的来源标注。真实窗口矩阵检查两种 AI 样式、皮肤标注开关与候选身份不变。候选的简繁输出与日文例外继续生效。测试用的独立 `CandidatePanel` 只有已格式化文本，因此原样绘制，不通过正则猜测标注边界。

共享设置页的皮肤卡片和编辑器预览使用固定样例绘制同一声明；导入、暂存、复制和 TOML 导出保留字段与原始颜色，导出不写派生的 `resolvedColor`。旧草稿缺少派生色值时按共享解析器的颜色语义补齐。测量在图片加载和字体变化后重做，错误、越界和换肤会清理文字。带叠加词的图片和文字放进同一个装饰组，编辑器的四种待机动画只作用于该组；减弱动态效果同时禁用整组动画。

HarmonyOS 的 2in1 候选窗从同一份完整 catalog 取 `decorationCandidate`，经有界原生策略从当前页取词，将共享层的 RGBA 规范色转换一次为 ArkUI 的 ARGB。独立的 `CandidateDecorationView` 将图片与文字放在同一个组件；父视图按图片身份重建组件，加载失败时移除文字，旧回调不能更新新皮肤。候选、高亮、字体或声明变化时显式重测，并让绘制节点直接订阅测量结果，避免界面只更新选中条却保留旧装饰词。字号使用明确的 vp，`getMeasureUtils().measureTextSize` 的像素结果转成 vp 后按比例缩小；最小字号仍过宽时不换行、不截断。文字框保留声明尺寸，并沿装饰图的对齐边定位，不把等比缩小后的图像尺寸当成文字框。重复文字不进入辅助朗读、不拦截候选点击。触屏键盘仍按既有规则不绘制这张桌面装饰图，本次不改变它的固定高度布局。

Windows 候选窗读取完整 catalog 的同一声明，复用共享的选词与几何契约；当前页原文已经经过输出模式转换，独立标注按 `stripBadge` 控制。文字只在装饰图片成功解码并绘制时出现，每帧从当前快照取词，越界、坏图和换肤不保留旧词。DirectWrite 使用既有字体回退链，按真实测量宽度缩字；缩字只修改本帧 layout，避免格式缓存积累任意字号。图片和文字使用同一进出场变换及淡入淡出，窗口 DPI 同时缩放声明框、字号和偏移；不新增 Windows 尚未实现的装饰待机变换。文字仍相对声明框、沿缩小后图片的对齐边与底边定位，达到最小字号后保持完整单行且不加裁切。

iOS 的不可变资源快照同时携带传统装饰带与可选装饰词，不再只对 canvas 基底解码。图片仍从共享资源 ABI 校验并在串行后台解码，和九宫格、自由挂件共用八百万像素预算及帧动画生命周期。固定高度的候选条把装饰带算入外留白，沿用[移动画布的统一缩放与文字可读下限](2026-10-19-ios-candidate-canvas.md)，不因开始组字改变键盘高度。声明框、字号、偏移与图片按同一比例缩放；图片等比缩小时，声明框仍沿同一对齐边与底边定位。传统装饰带加载成功不意味着九宫格面板已绘制，仍保留原来的安全配色与首选高亮语义。

iOS 快照严格读取每一行的引擎高亮状态，过滤移动端不显示的云占位行时同步映射；没有高亮时不假定第一项高亮。日语本地转换显式选择时使用实际转换项，展开网格继续取普通当前页，简繁输出及日文例外沿用候选显示转换。独立的云／AI 标注按 `stripBadge` 控制，不包含图钉或候选原文后缀。字体沿用候选的回退链，实测缩字，最小字号仍超框时绘制完整单行；节点不截获触摸、不重复朗读。换肤、隐藏、越界、无高亮与资源失败清掉旧词，设置页原生预览用固定样例绘制同一快照。此接入没有新增 iOS 尚未实现的装饰待机变换。

这些实现覆盖共享协议、MCP、网页 SDK、共享设置页预览、macOS、Windows、iOS 原生候选区域与 HarmonyOS 2in1 的装饰词绘制；Android 的普通／展开候选区域已接入并通过 API 28／35 原生绘制专项，完整设备回归以 Android 笔记为准；Linux X11 接入与验收见对应笔记，Wayland 和 HarmonyOS 触屏仍不能据此声称已绘制该原语，不能以共享层新增字段或预览可见代替六平台完成。

## Alternatives considered

- **按 msime-windows 把非 canvas 的 v2 整体降级为 v1**：与来源的判定逐字一致，网页、原生宿主和 Windows 对同一个包的 v2 判断最简单。但本仓自 #7543 起 v2 就表示有限动画、装饰动效与 APNG 帧，而且不限 base；已发布的 `base = "system"` / `shuishan` / `night` / `fluent` 等 v2 皮肤，以及 MCP `make_skin` 提示词按 `schema_version = 2` + `base = "system"` 让模型生成的用户和社区皮肤，都会被悄悄去掉动效，因此不采用；只把来源 v2 里 canvas 专属的字段限定在 `canvas` 基底上。
- **只把新 ID 加入 Windows 外观常量**：改动最小，但清单新字段仍会被静默忽略，无法与来源的视觉协议兼容，因此不采用。
- **让 Windows 单独解析 TOML 字段**：可以更快让部分字段生效，但会复制安全校验并使其他宿主的扫描结果与 Windows 不一致，因此不采用。
- **一次性把全部新绘制原语接入 Direct2D**：视觉覆盖最完整，但会把协议迁移和渲染器重构绑在一起，难以独立验证；本次先保证字段安全、可传递并消费已有能力，后续按原语分别接入。

## Consequences

Linux X11 的字体与候选布局消费见[原生画布布局](2026-10-20-linux-canvas-layout.md)，不改变本篇共享解析契约。

共享 catalog 与 Windows 之间的字段契约扩大，后续修改清单 schema 必须同步 Rust 模型、各平台 JSON 消费和对应测试。候选皮肤可以安全携带来源新增字段，Windows 已有渲染路径会显示更完整的颜色和字体效果，网页、macOS、Linux、HarmonyOS 与 iOS 仍通过同一份扫描规则读取公共字段，旧包不受影响。社区上传对新基底与空可选文字的保护由[社区画布资源按服务端能力上传](2026-10-19-community-canvas-resources.md) 部分取代：本地创建、打包与安装接受这些清单，真正上传前要求同源服务端明确声明扩展能力。MCP 描述与生成提示词同步说明资源规则；云端不支持时仍保留本机使用。

原生宿主的九宫格、材质、版式、insets 和自由挂件仍未全部绘制，仍与来源的完整视觉效果有差距；Windows 已经绘制的版式、外框、阴影档位、列表翻页箭头和收紧宽度见 [Windows 候选窗绘制皮肤的版式、外框、阴影、翻页箭头与收紧宽度](2026-10-19-windows-candidate-skin-layout-keys.md)，canvas 的九宫格、insets 与自由挂件见 [Windows 候选窗绘制 canvas 皮肤](2026-10-19-windows-candidate-canvas.md)；文档明确这一边界，避免把协议支持误报成绘制支持。图片边界现在对 PNG/JPEG/GIF/WebP 统一检查尺寸、动画帧和包内像素预算；网页宿主与原生宿主一样对任何 base 的 v2 清单启用动画，只有九宫格、自由挂件和 insets 依赖 `canvas`；网页 SDK 还与 catalog 一样拒绝没有 `schemaVersion: 2` 的 `base: "canvas"`，`canvas: true` 标记也不会覆盖全局主题或 Windows 外观基底。代价是 v2 在本仓与来源的含义不再逐字相同：同一个非 canvas v2 包在 msime-windows 上不播动画，在本仓播；以后来源若给非 canvas 的 v2 加新字段，需要逐项判断它属于哪一层。新增 JSON 和 C++ 字段也扩大了构建面，因此提交前运行皮肤单测、clippy、笔记校验、差异检查和 Windows x64 交叉构建。

验证结果（本笔记更新时实际运行）：`cargo fmt --all -- --check` 通过；`rbuild cargo test --locked -p msime-client-services skin::` 通过（262 项，含非 canvas v2 保留动效并忽略 canvas 专属字段、v1 + canvas 拒绝、空 `author` / `description` 打包拒绝和共享用例表）；`rbuild cargo test --locked -p msime-mcp-server` 通过（单测 69 项、stdio 11 项；最后一次修改描述后重跑，stdio 有一次因 Studio 上写入锁文件失败（`cannot record the write`）挂掉，单独重跑通过）；`rbuild cargo test --locked -p msime-host-api` 通过（847 项单测加各集成测试）；`rbuild cargo test --locked -p msime-desktop skin` 通过（17 项，需先在 Studio 上建空的 `target/macos/EngineResources` 与 `水杉输入法.app` 占位目录）；`rbuild cargo clippy --locked -p msime-client-services -p msime-mcp-server -p msime-host-api --all-targets -- -D warnings` 无警告；Windows 的 `candidate_skin`、`candidate_palette`、`candidate_skin_reload`、`candidate_theme_reload` 四个可移植单测在 Studio 上用 `c++ -std=c++17 -Wall -Wextra -Werror` 编译并通过；`node --test 'packages/web-engine/test/*.test.mjs'` 通过（34 项，含 `#RGBA`）；`apps/desktop` 的 vitest `custom-theme-parity`、`external-skins`、`community-candidate-skins` 通过（132 项）；`platforms/harmony/tests/run.sh` 通过（`keyboard-logic` 4395 条断言）；`pnpm run verify-notes` 与 `git diff --check` 通过。用 msime-windows `c0a9d371` 的 `skins/` 实测共享 catalog：13 个外部皮肤全部接受（`green_leaf`、`sakura-night` 为 v2 canvas，带宿主绘制能力提示），8 个内置外观以原 ID 放进皮肤目录按保留 ID 拒绝，改名并补上外部皮肤必需的 `version`、`base`、`supports` 后全部接受。没有运行：`platforms/windows/build-cross.sh x64`（Studio 没有 MinGW，交给 PR 上的 Windows CI）、macOS `candidate-panel` 与 `candidate-row-layout`；也未在真实 Windows 编辑器中目测所有新绘制原语。

Web 绘制切片的后续验证：本机 `node --test 'packages/web-engine/test/*.test.mjs'` 36 项通过；`CHROME_BIN=... node packages/web-engine/test/candidates.browser.mjs` 通过，截图像素断言覆盖九宫格四角/边缘/中心、挂件前后层级、裁切、点击穿透、留白定位、图片失败与过期回调，以及 APNG/GIF 面板和挂件的真实帧变化。减弱动态效果时首帧保持不变，实时切换系统偏好和无 CORS 响应头的跨域图片同样通过。固定上游 `c0a9d371` 的 `green_leaf` 与 `sakura-night` 实际图片在浅/深色与横/竖排的八种组合中显示正常；这些源资源只用于本机验收，没有复制进本仓库。类型声明检查、`cargo fmt --all -- --check`、本机 services 皮肤测试 262 项、MCP 单测 69 项与 stdio 11 项、services/MCP 的 `clippy --all-targets -- -D warnings` 均通过。以上只覆盖 Web 贴图绘制与共享回退契约，不代表其余原生宿主具备完整贴图能力。

装饰词切片的验证：共享服务皮肤测试 333 项、MCP 单测 94 项与 stdio 集成 12 项通过，services/MCP 的 clippy 无警告；网页引擎单测 47 项、真实 Chromium 候选栏回归和 TypeScript 声明检查通过。stdio 合成夹具同时准备独立的 `msime-wubi.db`，避免 macOS 按词库文件判定五笔可用时将夹具的输入方案回退；产品的资源判定不变。共享 React 的真实浏览器验证使用合成图片与固定样例，54 种横竖／图像对齐／文字对齐组合、四种整组待机动画、越界、换肤、图片失败、超框与减弱动态效果均通过；这是组件绘制证据，不是原生宿主系统入口或设备验收。

最终共享设置测试 94 项、类型检查与生产构建通过；macOS `candidate-skin`、`candidate-panel`、`candidate-row-layout`、`candidate-decoration-overlay` 四组测试通过，新增专项覆盖当前页取词、释义更新复用、150% 缩放、展开网格、换肤关闭和越界清理。`scripts/verify-local.sh --quick` 通过，含 Windows 可移植测试 171 项、Linux 原生测试 94 项及共享 Rust、Android、WASM、Java、macOS 和 Apple bridge 编译；完整 Windows／pipe 构建因没有 MinGW 跳过，HarmonyOS HAP 另走配套 DevEco 工具链，不把 quick 的前置不足跳过当作 HAP 验证。

选中条、逐角高亮、悬停与预编辑配色由[原生画布外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)补充，保留本篇其余范围与生命周期规则。

[候选释义分段](2026-10-20-candidate-translation-runs.md)由共享 Rust 层发布 UTF-8 同源范围与规范化颜色；Linux 原生画布消费这些范围，其他宿主可继续忽略新增的可选字段。

Windows 基底的绘制默认由[共享出厂布局](2026-10-20-skin-factory-layout.md)补齐：`resolvedLayout` 与编辑器原始声明分开，供各宿主逐步接入。


HarmonyOS 装饰词切片的验证：`platforms/harmony/tests/run.sh` 通过，其中新增 31 条断言覆盖当前页、高亮、独立标注、缩字下限、九种对齐、颜色顺序与无效声明。配套 DevEco SDK 22 / hvigor 6.22.9 的 `assembleHap --no-daemon` 通过，arm64 的 Rust/C++/NAPI 交叉构建通过。在专用 HarmonyOS 6.0.2(22) 2in1 模拟器中安装、启用、切换、打开共享设置与制作皮肤页均通过。真实编辑框通过硬件事件生成候选，合成装饰图内显示当前高亮词，页内高亮与翻页后文字同步，上屏后装饰词节点消失。

该模拟器没有签名 Profile 的共享沙箱授权，设置应用与输入法扩展的数据隔离；原生绘制验收使用临时包把合成皮肤与偏好放入扩展的私有沙箱，绘制、取词、测量和输入链路仍使用产品代码。临时暂存入口、合成资源与诊断日志不提交，也不作为跨沙箱同步已验证的证据。图片失败、换肤重建和超框边界仍以策略、组件生命周期与编译证据为限，不把未跑过的设备场景写成通过；真机签名与共享沙箱同步未验收。

本轮提交前 quick（构建机 run `20261021-023552`）通过，Linux 原生 94 项与各已配置编译阶段均通过。移除专用模拟器的临时包与私有测试数据后，用无夹具、无临时入口的产品源码重新编译并安装 HAP；包内资源和 ArkTS 源码检查无测试入口。干净包真实编辑框中的 `ni` 候选与空格上屏「你」通过，未选皮肤时没有装饰词节点。最终格式化后的 HarmonyOS 逻辑套件与 HAP 编译也通过，专用模拟器已停止。

Windows 装饰词切片本地验证：catalog 解析与资源门控测试在 macOS 编译运行通过；`CandidateWindow.cpp`、DirectWrite/九宫格预览测试和图库工具在 MinGW x64 与 x86 下以 `-Wall -Wextra -Werror -fsyntax-only` 通过，x64 的 DirectWrite/预览测试程序链接通过。提交前 quick（构建机 run `20261021-025345`）通过，Linux 原生 94 项及各已配置编译阶段通过；构建机没有完整 Windows 交叉工具链，不能把其跳过项当作通过。新增 DirectWrite 实测对齐、缩字与超框断言，以及原生候选窗的高亮、翻页、独立标注、越界、图片失败与关闭声明的像素验收，由 PR 的 Windows runner 执行，本地没有 Windows 桌面，不预写为设备通过。

Windows 后续 CI 在提交 `02796f55f6` 上实际通过：Native Platform CI run `39470951883` 的 Windows GNU 交叉构建及 Windows 原生图库均成功。原生 runner 执行 DirectWrite 实测与四种横竖／明暗组合的装饰词像素验收，`__overlay-*` 产物包含关闭、首项、高亮、翻页、越界、独立标注、坏图和重置九个状态；下载检查了真实窗口截图。该证据覆盖原生绘制与窗口状态，不外推为 TSF 系统编辑器的输入验收。


iOS 装饰词切片的最终验证：构建机 run `20261021-032219` 在 Xcode 27 的 iPhone 18 Pro 模拟器上运行 78 项专项测试，1 项依赖上游图片的用例跳过、0 失败；覆盖真实 UIKit 像素、高亮切换、候选更新、独立标注、九种对齐、缩字下限、固定高度统一缩放、隐藏、零宽、资源失败与生产键盘控制器的普通／展开候选同步。已导出并检查控制器截图。原生 `MSIMEApp` 与内嵌键盘扩展的 Debug simulator 构建通过，iOS Rust simulator 静态库也已真实编译。以上是模拟器中的生产控制器与原生绘制证据，未在系统编辑器启用第三方键盘，也没有真机安装与输入验收。

iOS 同轮 quick 门禁也通过：构建机 run `20261021-032219` 包含 Windows 可移植测试、Linux 原生 94 项、Linux 设置壳、共享 Rust、Android target 与 Java、WASM、macOS 原生及 Apple bridge 编译。没有完整 MinGW 和 pipe 工具链，HarmonyOS HAP 前置未暂存，这些阶段明确跳过；HarmonyOS 独立 HAP 与设备结果以上述专项为准。随后在同一专用 iOS 模拟器安装并成功启动原生 `MSIMEApp`。

Android 装饰词的接入、无显式配色 system 包保留、设备证据与回归边界见[Android 原生画布](2026-10-19-android-candidate-canvas.md)；Linux X11 的动态装饰词与资源戳时序修复见[X11 装饰候选词进入原生候选窗口](../architecture/2026-10-21-linux-decoration-overlay-window.md)。
