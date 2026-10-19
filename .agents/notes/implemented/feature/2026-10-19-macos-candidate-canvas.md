# Agent Note: macOS 候选窗绘制 canvas 皮肤

Status: implemented

## Problem

[跨平台清单兼容](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)使 canvas 包可以被安装，但原生 macOS 候选窗没有绘制九宫格与自由挂件。直接给现有纯色卡片换成贴图配色会使文字不可读；只在测试用的 `CandidatePanel.mm` 画图又覆盖不到产品的 `InputController.mm`。

## Decision

在真实控制器的普通页与展开网格接入 canvas。共享 Rust 保留安全回退与原贴图配色；`msime_client_resolve_theme` 新增缺省为 false 的 `canvas_panel_drawn`，原生资源成功解码后才启用原色，再由同一个共享解析器覆盖用户取色器。细分文字色也由共享解析器处理，新增的 `candidate_details` 是可选输出；不支持这些颜色的宿主继续使用既有字段。

图片在主题重载时解码，逐键排版复用位图。九宫格按源像素裁切，宽度、内外留白和挂件坐标按 DIP 缩放。卡片背景作为独立的绘制层，低于卡片的挂件、面板、候选内容和上层挂件按顺序放置，候选身份校验继续使用原有内容容器。Core Animation 播放图片帧，不引入逐键定时器；减弱动态效果、隐藏和屏幕休眠时撤掉动画。GIF 无循环扩展只播放一轮；ImageIO 已把扩展的重复次数换成总轮数，不能再加一次。帧时长先取整到毫秒，10ms 及以下按上游与浏览器规则改为 100ms；有限循环结束后保留最后一帧。canvas 不叠加原生窗口阴影。

包内 `item_corner_radius_dip` 同时用于候选选中与悬停轮廓，随候选缩放。宿主不透明度只淡化面板，不影响文字或自由挂件。同一个包的明暗切换复用布局与候选按钮，移除旧 canvas 绘制层后挂上对应模式的缓存资源，避免只换文字颜色而继续显示旧贴图。

光标定位以卡片为准，左、下外留白不参与对齐和翻转判断；整个窗口仍按 macOS 原有规则夹进屏幕可见区域。打字特效直接消费同一份 `cardBounds`，不把透明外留白算进卡片轮廓。

## Alternatives considered

- **直接把 `candidate` 换成原贴图颜色**：所有宿主能立刻读到同一套原色，但尚未绘制贴图的宿主会在纯色回退上丢失文字对比，所以保留明确的能力开关。
- **每次按键重新解码并合成整张图片**：实现与静态截图最接近，但会把图片 I/O 和解码带进输入响应路径，并重复分配大图，所以在偏好重载时准备资源，重排只创建绘制图层。
- **另造候选内容容器**：便于整体挪动文字，却会破坏实际按钮所属视图的身份校验；继续使用液态玻璃已经引入的候选内容容器，只调整它的卡片位置。

## Historical audit

- [跨平台清单兼容](2026-10-19-windows-skin-schema-parity.md)：部分重叠，保留共享解析与回退决定，本篇补 macOS 的实际绘制与能力协商；双方互链。
- [玻璃内容归属](../../implemented/bug-fix/2026-10-19-macos-glass-content-containment.md)：部分重叠，保留候选内容容器，新增背景层只负责低于卡片的挂件层级，按钮身份仍按原容器检查。
- [装饰图动效](2026-10-16-skin-decoration-idle-motion-and-apng.md)：部分重叠，已有单张顶部装饰保持原路径；canvas 的九宫格和多个挂件独立缓存资源、播放帧，不替换旧皮肤契约。

## Verification

- `candidate-row-layout` 通过真实 `InputController.mm` 验证明暗、横竖排、100%／150% 缩放、普通与展开候选；检查四边内容与窗口留白、精确的预编辑行间距、隐藏首行不留空行、前后层级、点击穿透、旧按钮身份失效、行圆角和细分文字色。同一个包原地切换明暗时验证资源槽位、实际裁切图片和按钮身份，反复切换不叠加旧绘制层。
- `candidate-placement` 检查左外留白的光标对齐、翻到光标上方时扣除底外留白，以及只有透明留白放不下时不提前翻转；生产控制器验证普通页与展开网格的实际屏幕坐标。
- `candidate-canvas` 栅格像素检查固定边宽、中心不填充、挂件 contain 与资源失败；合成 APNG/GIF/WebP 的帧颜色和时长正确，GIF 无循环、有限重复、10ms 帧归一化均有断言。测试在进程内固定减弱动态效果读数，验证实时切换、睡眠／唤醒与隐藏，不改系统设置。
- 固定上游 `c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的 `green_leaf`、`sakura-night`，以 `candidate-row-layout-test --canvas-upstream <skins-root> <output>` 运行生产控制器。明暗 × 横竖排 × 普通／展开共 16 张原生栅格图完成目测；外部资源只用于验收，没有复制进仓库。
- Rust：services 皮肤 264 项、host-api 单测 881 项通过（3 项忽略），新增能力协商用例单独通过；MCP 单测 69 项与 stdio 11 项通过，相关 fmt 与 clippy 通过。
- 原生全套第一次运行 152／153 项通过，失败为 HTTP 语音夹具在客户端切换后异步重载真实偏好，调用未实现的 `updatePreferencesSnapshot:error:`。语音控制器夹具现与已有 `ensureAppearance` 一样固定 `reloadPreferences`，只消费合成偏好；不改产品重载或识别流程。修正后与候选、玻璃等相关 8 组测试通过。
- `verify-local.sh --quick` 在最新 `develop` 基线上通过，包含 Rust、Android、WebAssembly、Linux 设置壳与 90 项原生测试、macOS 全部目标及 Apple 桥；构建机缺少 Node、Windows MinGW 和 HarmonyOS 原生前置，对应检查跳过。笔记在本机通过 `pnpm run verify-notes` 验证。

## Consequences

新增可选 JSON 字段兼容旧宿主。资源解码在主题重载时进行，候选重排复用资源；代价是每种明暗都缓存图片帧。图片尺寸、帧数和包内总像素预算与共享扫描器保持一致，无法解码的面板继续使用可读回退。

当前验证是原生控制器的合成候选和 AppKit 栅格，不等同于物理键盘进入完整 IMK 会话，也没有旧 macOS 系统实机结果。其余原生平台和社区上传支持仍须继续实现，不据本篇宣称全平台完成。

卡片与候选内容拆层会影响玻璃回退和按钮身份校验，需要实际控制器回归守住层级与点击行为。图片解码成功不代表所有缩放和系统版本的输出一致；旧系统实机覆盖仍不足，不据此宣称所有系统版本或全平台绘制一致。
