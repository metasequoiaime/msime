# Agent Note: 候选窗装饰图的待机动效与 APNG 帧动画

Status: implemented

## Problem

皮肤的装饰图（`[candidate_window.decoration]` 的 `image`，比如「蓝色大肥鱼」里站在卡片右上角的女仆鲸）一直是静态的。用户要的是候选窗在屏幕上时这个角色自己会动，不是显隐动效：[有限动效契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md)的 `[animation]` 只管候选窗出现、消失和选中那一下，[macOS 只播放清单写出的动效](2026-10-16-macos-candidate-declared-motion.md)也只覆盖这三种。

角色要动，有两种做法，用户两种都要：

- 整张图做程序化的小幅循环动作（浮动、摇摆、呼吸、弹跳）。
- 图本身就是动画，逐帧播放。

约束有四条：

- 候选窗每个键都同步重排，动画不能拖慢打字。
- 打字时装饰图不能每个键都跳回原位。
- 系统的减弱动态效果要生效。
- 皮肤包可以从社区下载，不能让一个包把输入法进程的内存吃满。

## Decision

### 清单格式

只在 schema 2 里可写，写在 `[candidate_window.decoration]` 下：

```toml
[candidate_window.decoration]
image = "whale.png"
top_inset_dip = 48
width_dip = 60
motion = "float"          # none、float、sway、breathe、bounce，缺省 none
motion_period_ms = 3000   # 1200–8000，缺省 3000
```

- 共享加载器 `crates/client-services/src/skin/catalog.rs` 的 `read_decoration_motion` 解析这两个键。版本 1 写了就以 `decoration motion requires schema_version 2` 拒绝，与 `[animation]` 只在 v2 里可写一致。没有装饰的包写 `motion` 也拒绝。
- `SkinSummary` 新增 `decoration_motion`、`decoration_motion_period_ms` 和 `decoration_frames`，序列化为 `decorationMotion`、`decorationMotionPeriodMs`、`decorationFrames`。帧数在没有装饰图时为 0，静态图为 1，APNG 为它 `acTL` 声明的帧数。
- 帧动画不加新键：`image` 指向一张 APNG 就是帧动画。APNG 可以沿用 `.png` 后缀，让不播放帧的宿主读取默认图；服务端逐帧校验并保留动画。新的 `.apng` 后缀和 GIF/WebP 社区传输见[按服务端能力上传](2026-10-19-community-canvas-resources.md)。
- 新规则也写进了共享用例表 `catalog/client_dialect.json`，msime-cloud 和 msime-windows 靠各自的同步脚本跟上。

### 校验与上限

- **帧数**：一张 APNG 最多 120 帧（`MAX_ANIMATION_FRAMES`），超过以 `animated image has too many frames` 拒绝。
- **像素**：每一帧按整张画布计入整包像素合计，整包仍是原有的 800 万像素（`MAX_PACKAGE_PIXELS`）。宿主播放时每帧都解码成画布大小的位图，所以这个合计就是一个包解码后的位图上限，最多约 32 MiB，与静态图同一个量级。
- **两层检查**：
  - `catalog` 扫描时只用 `png` crate 读到第一个 `IDAT` 之前的块，取 `acTL` 帧数，不解压像素，每次扫描都很便宜。
  - 社区发布、安装和 MCP 创建走 `candidate_community::check_image`：照旧完整解码默认图，再用 `png` 逐帧完整解码每个 `fdAT`，分配受 64 MiB 限制。只有某一帧损坏的 APNG 也会以 `candidate_skin_image_invalid` 拒绝。
- **新依赖**：`png` 0.18.1 加为 `msime-client-services` 的直接依赖。它本来就是 `image` 的依赖，不引入新 crate；`image` 不公开 `acTL` 的帧数，所以直接用它底下的这一层。

### macOS 播放

两条渲染路径共用 `MSIMECandidateMotion`（`platforms/macos/src/candidate/CandidateMotion.mm`），也就是 IMK 产品用的 `InputController` 和测试用的 `CandidatePanel`。

- **装饰视图**：装饰图从 `NSImageView` 换成 `MSIMECandidateDecorationView`。它把图片放进自己图层的 `contents`，不接收鼠标。`NSImageView` 默认 `animates = YES`，会自己用主线程定时器播 APNG，每次重排都从第 0 帧开始，也不理减弱动态效果。
- **待机动效**：`transform` 上的 `CAKeyframeAnimation`，以图片底边中点为轴，无限重复，从原位开始、回到原位。
  - `float`：上下 ±3 点。
  - `sway`：至多 ±3°。
  - `breathe`：放大至多 1.03 倍。
  - `bounce`：大半个周期站着，然后跳起 3.5 点，上升减速、下落加速。
  - 幅度还按图片大小收小，保证最大位移不超过 `kDecorationMotionMargin`（4 点）。
- **APNG 帧**：
  - `MSIMEDecorationFrames` 用 ImageIO 读帧、帧时长（`kCGImagePropertyAPNGDelayTime`）和循环次数，一次解码好。
  - `InputController` 按 `_appearance` 读好的那张装饰图缓存它，打字时不再读文件。
  - 播放是 `contents` 上的离散 `CAKeyframeAnimation`。有限循环播完后停在最后一帧，与浏览器相同。
- **不被裁掉**：候选窗内容视图 `masksToBounds`，窗口正好包住卡片和装饰带。有待机动效时 `DecorationPlacement` 让装饰图顶边和左右两边离窗口边缘至少 4 点，放不下就整张等比缩小，底边仍压在卡片顶边的同一位置。没有动效时摆法不变。
- **相位连续**：两种动画的 `beginTime` 都换算自同一个时刻 `decorationAnchor`。
  - 取值：候选窗停下来之后第一次出现时取当时。收起后 400ms 内又出现算同一段连续输入，沿用原来的时刻，阈值与进场动效相同。
  - `InputController` 每次排版都换新的内容视图和装饰视图，`showCandidatePanel` 把动画重新挂到新视图上、撤掉旧视图上的，时间线不变，所以装饰图不会每个键都跳回原位。
- **什么时候不动**：
  - 收起：`hidePanel:` 撤掉装饰图的动画；有退场动效时装饰图跟着卡片进退场窗淡出，退场结束后整张卡片随退场窗内容一起放掉。失焦时 `stop` 撤掉。
  - 减弱动态效果：`NSWorkspace.accessibilityDisplayShouldReduceMotion` 及其变更通知。打开时撤掉，图层模型值就是第一帧；关掉时如果候选窗还在屏幕上就接着动。
  - 屏幕睡眠、锁屏、切走用户：`NSWorkspaceScreensDidSleepNotification`、`com.apple.screenIsLocked`、`NSWorkspaceSessionDidResignActiveNotification` 撤掉，醒来时如果还可见就接着动。
- **不拖慢打字**：渲染仍然同步。动画只是加在图层上的对象，由合成器驱动，主线程没有定时器。

### 其他宿主与表面

| 宿主 | 现状 |
|---|---|
| Windows | 原生候选窗读共享 catalog 的 JSON，忽略新键；WIC 解码 PNG 只取默认图，静态绘制 |
| Linux | 发给 Fcitx5 的 host catalog 不带新键，classicui 主题图由 cairo/libpng 解码再重新编码，静态绘制 |
| HarmonyOS | 读 catalog JSON，忽略新键；装饰图交给 ArkUI `Image`，没有在设备上验证它会不会自己播 APNG |
| Android、iOS | 不画候选窗皮肤的装饰图 |
| 网页引擎、共享 React 设置页 | 接受新键但不播待机动效；装饰图是 APNG 时浏览器自己逐帧播放，不理 `prefers-reduced-motion` |
| macOS 设置页预览 | `CandidateSkinPreviewView` 画静态的第一帧 |

MCP 的 `create_candidate_skin` 说明和 `make_skin` prompt 写明 `motion`、`motion_period_ms` 和 APNG 的上限，`list_candidate_skins` 对有装饰图的皮肤报告 `decoration { motion, motion_period_ms, frames, animated }`。

## Alternatives considered

- **GIF 或动画 WebP**
  - 最强理由：作者手里现成的动图多半是 GIF，WebP 体积更小、带完整 alpha。
  - 不采用的原因：
    - GIF 只有 1 位透明，装饰图压在卡片顶边上，锯齿边一眼可见。
    - 两者都要在共享加载器、社区服务端和每个宿主里新开一种格式：服务端只接受 PNG 和 JPEG，`image` crate 也只开了这两种解码器。
    - 不播放的宿主看到的是一个不认识的文件，不像 APNG 那样退回第一帧。
- **雪碧图（一张大图横排各帧，清单写帧数和帧时长）**
  - 最强理由：仍是普通 PNG，任何解码器都读得出，帧时长由清单控制，不依赖 APNG 支持。
  - 不采用的原因：
    - 不播放的宿主会把整条雪碧图当装饰画出来，而不是第一帧。
    - 要新增帧数、帧宽和帧时长的清单键，并在每个宿主上实现裁切。
    - 单边 2048 像素的上限让帧数和帧宽互相挤占。
- **Lottie 或其他矢量动画**
  - 最强理由：体积小、缩放不失真，设计师工具链成熟。
  - 不采用的原因：
    - 要在每个原生宿主里引入一个动画运行时，macOS 没有系统自带的。
    - 它是一份可执行的动画描述，同步包的审计面从「几张图」变成「一个小程序」，与动效契约拒绝任意 CSS 的理由相同。
- **CSS `@keyframes` 或任意曲线**
  - 最强理由：作者想怎么动就怎么动，Web 预览天然支持。
  - 不采用的原因：与[有限动效契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md)否掉它的理由相同。原生宿主无法一致执行，也无法限住幅度，装饰图会被窗口边缘裁掉。
- **让 `NSImageView` 自己播 APNG（`animates = YES`）**
  - 最强理由：不用写一行播放代码，AppKit 已经支持。
  - 不采用的原因：
    - 播放由主线程定时器驱动。
    - `InputController` 每个键换一个新视图，动画每个键都从第 0 帧重来。
    - 不理减弱动态效果，也不能和待机动效共用一条时间线。
- **把装饰带加高几点，而不是缩小装饰图**
  - 最强理由：装饰图保持作者给的大小。
  - 不采用的原因：装饰带高度决定卡片在窗口里的位置，候选窗翻到光标上方时卡片和光标之间会多出这几点空白，按皮肤是否有动效而不同。缩小只影响装饰图自己，最多 4 点。
- **每次重排都从原位重新开始待机动效**
  - 最强理由：不需要共享时刻，实现最简单。
  - 不采用的原因：打字时每个键都重排，装饰图会一直被拽回原位、看起来在抖，用户正是在打字时看着它。

## Consequences

收益：

- 皮肤作者只写一个键就能让角色动起来，或者直接交一张 APNG。不支持的宿主照常画第一帧，同一个包在所有平台上都能加载。
- macOS 上动画只在候选窗可见时由合成器驱动，不占主线程，打字时相位连续，减弱动态效果、睡眠和锁屏时停下。
- 一个包解码后的位图上限没有变，仍是 800 万像素。

代价：

- APNG 的每一帧都计入像素合计，同样尺寸的图，帧数越多，背景图和预览能用的像素越少。
- 社区服务端发布时会重新编码图片（`MAX_PACKAGE_FILE_BYTES` 的注释），而 Go 的 `image/png` 解码时跳过 `acTL`、`fcTL`、`fdAT` 这些辅助块。按推断，重新编码后只剩默认图，所以从社区下载的皮肤目前没有帧动画，本地安装和 MCP 创建的有。服务端的代码不在本仓库，这一点没有核实。
- 网页引擎和共享 React 设置页的 APNG 由浏览器播放，不理减弱动态效果；它们也不播待机动效。
- 新增或调整一种待机动效，要同时改 `catalog.rs` 的枚举和用例表、`SkinManifestBridge.mm` 的映射、`CandidateMotion.mm` 的 `DecorationMotionAnimation`、MCP 文案和本笔记。

### 后续

各平台播放：

- **Windows**：在 Direct2D 合成层上实现待机动效，并用 WIC 之外的解码器（如 `png` 经 host-api 导出帧）播放 APNG。用 `SPI_GETCLIENTAREAANIMATION` 作减弱动态效果开关，与[原生宿主生命周期](../architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)的退场规则一致。
- **Linux**：classicui 只能画一张静态 overlay，要播放必须先有宿主自己拥有的候选面板（见同一篇笔记的 Linux 部分）。
- **HarmonyOS**：在设备上确认 ArkUI `Image` 对 APNG 的行为。如果会自己播，改成解码第一帧的 `PixelMap`，或者接系统的减弱动态效果。再加待机动效。
- **Android、iOS**：先要画候选窗装饰图本身。
- **网页引擎和共享 React 设置页**：用 CSS 关键帧实现同样有界的待机动效，并在 `prefers-reduced-motion` 下把 APNG 换成第一帧。

服务端与其他仓库：

- msime-cloud 已有 APNG 保留与逐帧校验路径；GIF/WebP 和 `.apng` 社区资源扩展见[按服务端能力上传](2026-10-19-community-canvas-resources.md)。
- msime-cloud 和 msime-windows 用同步脚本刷新 `client_dialect.json` 副本，接受新键。

### 验证

- **Rust**：
  - `msime-client-services` 的 `skin::` 单测，包括用例表新增的 17 条、APNG 帧数与逐帧像素上限、社区安装逐帧解码和损坏帧拒绝。
  - `msime-mcp-server` 全部单测，包括列表报告、工具说明和 prompt。
  - 两个 crate 的 clippy `-D warnings`。
- **macOS**：
  - `candidate-panel` 覆盖 `CandidatePanel` 路径：可见时挂上、收起撤掉、重排后起算时刻不变、连续输入沿用相位、停顿后重来、减弱动态效果停在第一帧并在关掉后接着动、屏幕睡眠与醒来、帧数超限的包被共享加载器以帧数为由拒绝。
  - `candidate-row-layout` 覆盖 `InputController` 路径：每次排版换新装饰视图后动画从同一时刻起算、旧视图上的撤掉、摇摆以底边中点为轴且横移在余量内、收起和减弱动态效果时不动。
- **真窗口观感**：本地探针用真实的 `CandidateMotion.mm` 在屏幕右下角开一个光标层级减 100 的无边框面板，`screencapture -l` 连续截图后测装饰图像素。
  - `float`：顶边在 2–14 像素（2 倍屏）之间往复，即 ±3 点，周期约 1.2 秒，不贴出窗口。
  - `sway`：顶部中点横移 ±2.7 点，底边不动。
  - `breathe`：顶边上抬 2 点。
  - `bounce`：顶边在 1–8 像素之间，跳起 3.5 点，最高点离窗口顶边还有 0.5 点。
  - APNG：四种颜色约每 0.25 秒换一次。
  - 减弱动态效果：只有第一帧、位置不变。
  - 每 80ms 整页重排一次：顶边仍在 3–14 像素间按正弦走，没有被拽回原位。
- **没有验证**：HarmonyOS 上 ArkUI 对 APNG 的行为；装了这个皮肤的输入法在真实应用里打字时的观感，探针只用了同一套动画代码，没有经过 IMK。
