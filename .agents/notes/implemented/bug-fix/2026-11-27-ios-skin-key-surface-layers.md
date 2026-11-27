# Agent Note: iOS 自定义皮肤的按键表面和键盘底色改用图层合成，不再为每个键分配位图

Status: implemented

## Problem

[#11830](https://github.com/metasequoiaime/msime/issues/11830)：iPad Pro 12.9 寸（第 3 代，iPadOS 17.7.1）上全局皮肤设成「自定义」后，键盘扩展几秒内被系统以 `per-process-limit` 结束（JetsamEvent 里 `rpages` 5760 × 16 KB = 90 MiB，进程状态 `frontmost`，存活约 8 秒），回退到系统键盘；改回「跟随系统」就能稳定使用。报告者的皮肤只有纯色，没有图片、花纹和渐变，所以多出来的内存不来自素材。

自定义皮肤和跟随系统在扩展里的差别在绘制方式上。改动前：

- `SkinKeySurfaceView`（每个自定义皮肤按键的表面）重写 `draw(_:)`，`isOpaque = false`、`contentMode = .redraw`。每个键因此各有一块 CPU 位图，大小是 bounds × 屏幕倍率² × 4 字节，跟随系统的按键不走这条路，没有这块位图。
- `KeyboardSkinBackgroundView`（键盘底色）同样重写 `draw(_:)`，纯色皮肤也会分配一整块键盘大小的位图。

量级估算（没有在真机上量过）：iPad Pro 12.9 寸约 60 个键，每个约 100 × 75 pt，2 倍屏下 200 × 150 px × 4 字节 ≈ 120 KB，合计约 7 MB；底色约 2732 × 820 px × 4 字节 ≈ 9 MB。两项合计约 16 MB，占 90 MiB 上限的六分之一左右。这是**很可能的贡献者，不是已确认的根因**：跟随系统和自定义之间的全部差距有多少、其中多少来自这两类位图，要等真机上 Instruments（Allocations / VM Tracker）的数据。

## Decision

- **`SkinKeySurfaceView` 不再重写 `draw(_:)`。** 键面、凸起底座和它的暗色叠层、阴影、玻璃和凸起材质的渐变、玻璃高光、纸质纤维、描边各是一个 `CAShapeLayer` 或 `CAGradientLayer` 子图层，由渲染服务合成，扩展进程里不再为每个键分配 CPU 位图。公开接口不变（`design`、`fillColor`、`scale`、`path(in:shape:radius:)`），SwiftUI 的 `SkinKeySurface` 也不变。图层颜色是 `CGColor`，动态颜色在 `applyDesign()` 里按当前 `traitCollection` 解析，`userInterfaceStyle` 变化时经 `registerForTraitChanges` 重新解析；`applyDesign()` 在 `CATransaction.setDisableActions(true)` 里改图层，不带隐式动画。
- **材质的裁剪用遮罩。** 渐变、高光和纤维挂在 `finishLayer` 下，`finishLayer.mask` 是一个路径与键面相同的 `CAShapeLayer`。平面材质（`flat`）隐藏整个 `finishLayer`，没有遮罩那一遍。
- **键面几何是共享契约 `SkinKeySurfaceView.faceRect(in:design:scale:)`。** 键面先按 `scale` 内缩，凸起材质再从底部让出 `3 * scale` 的厚度。`KeyboardKeyButton` 按下时的阴影层用同一个函数取键面，不再自己抄一份计算；改键面几何只改这一处，按下阴影和键面不会错位。
- **阴影映射是近似。** 原来是 `context.setShadow(offset: (0, scale), blur: 2 * scale, color: black × shadow)`，现在是 `faceLayer` 的 `shadowPath` = 键面路径、`shadowOffset` = `(0, scale)`、`shadowRadius = scale`、`shadowOpacity = shadow × 填充色的 alpha`。Core Graphics 的阴影由填充本身的不透明度投下，半透明的键投出的阴影更淡，所以图层上要乘填充 alpha；`blur` 和 `shadowRadius` 的定义不同，`blur 2 * scale` 对应 `shadowRadius = scale` 是按截图对比挑的值，不是精确换算。
- **`KeyboardSkinBackgroundView` 的纯色只是 `backgroundColor`。** 视图本身不重写 `draw(_:)`；渐变、照片和花纹由私有子视图 `ArtworkView` 画，这个子视图只在皮肤有其中之一时存在，绘制代码原样移进 `drawArtwork()`。纯色皮肤不再有键盘大小的位图；有素材的皮肤照旧有一块。
- **皮肤选择器的缩略图改调新入口。** `KeyboardSkinMiniature` 原来直接调 `surface.draw(_:)` 和 `backdrop.draw(_:)`，现在用 `surface.layer.render(in:)` 和 `backdrop.drawArtwork()`，并把缩略图的 `userInterfaceStyle` 传给按键表面。`layer.render(in:)` 会套用遮罩，新测试里玻璃高光被裁在键形之内的像素断言同时证明了这一点。

## Alternatives considered

- **保留 `draw(_:)`，在键盘收起时释放位图（#10944 的方向）** — 这条最强的理由是改动小、绘制代码一行不动，[#10944](https://github.com/metasequoiaime/msime/issues/10944) 也正是要求收起时释放视图和皮肤位图，[键盘根视图改用普通 UIView](2026-11-05-ios-keyboard-plain-root-view.md) 之后整棵键盘视图（包括这些位图）已经随控制器一起释放。不采用：#11830 的进程是在前台、正在打字时被结束的，位图在键盘显示期间一直需要，收起时释放压不低显示时的峰值。
- **继续用 `draw(_:)`，但把纯色键改成 `backgroundColor` + `layer.cornerRadius`，只对有材质的键画位图（#11830 报告里建议的做法）** — 理由是报告者用的正是纯色键、改法最直观。不采用：`SkinKeySurfaceView` 支持的键形（`ticket` 票券形、`pebble` 卵石形）不是圆角矩形，`cornerRadius` 表达不了；描边、阴影和凸起底座也还要位图，最后会是两套渲染路径并存。图层合成一套代码覆盖所有键形和材质。

## Consequences

- **收益**：自定义皮肤每个键不再有 CPU 位图，纯色底色不再有键盘大小的位图；按估算 iPad Pro 12.9 寸上少约 16 MB（见 Problem，未实测）。键面几何只有 `faceRect` 一处定义。
- **代价**：玻璃、凸起和纸质材质的键各多一次遮罩带来的离屏渲染，由渲染服务承担，不计入扩展的内存上限，但每次重绘多一遍合成；平面材质没有这一遍。阴影是近似映射，3 倍屏截图对比里纸质模板在阴影边缘差 0.07% 像素，其余模板不超过 0.01%。有渐变、照片或花纹的底色仍然有一块键盘大小的位图。
- **未验证 / 重访信号**：真机上的内存效果还没有量。拿到 iPad Pro 12.9 寸上 Instruments 的数据后，如果这次的改动没有让自定义皮肤接近跟随系统的占用，说明主因在别处，要按 Allocations 的结果另找，并更正本篇 Problem 里的估算。如果离屏渲染在低端设备上表现为掉帧，考虑给有材质的键改用不需要遮罩的画法。#11830 里「跟随系统时偶尔闪烁、跳动」不在这次改动范围内。

## Verification

- `MSIMEKeyboardTests/KeyboardSkinTests` 新增 `testSkinKeySurfaceIsLayerBackedAndPaintsItsParts`（用 `layer.render(in:)` 取像素，检查填充、描边、圆角透明、凸起底座是填充色叠 0.28 黑、凸起高光、玻璃高光被裁在键形内、动态填充色随真实 `UIWindow` 的外观切换、类上没有 `draw(_:)` 重写）和 `testSolidSkinBackdropHasNoDrawnBitmap`（`KeyboardSkinBackgroundView` 没有 `draw(_:)` 重写，`ArtworkView` 只在渐变、照片或花纹时存在）。已有的 `testKeyStylesSurviveSavingAndRenderDistinctSurfaces`（16 种键形 × 材质组合仍渲染出互不相同的表面）、`testGradientAndPhotoRenderInKeyboardBackdrop`、`testCommunityPreviewDoesNotChangeActiveDesign` 照旧通过。

## 相关笔记

- [iOS 键盘诊断日志记内存读数](2026-10-10-ios-keyboard-memory-telemetry.md)：那里把 `KeyboardSkinBackgroundView` 的位图缓冲列为真机超限时要减的一段，本篇减掉了纯色皮肤下的这一段；`keyboard_shown` 的 `mem=` 读数是真机验收时对比改动前后的入口。部分重叠，互链。
- [iOS 键盘开着 ONNX 重排时的内存](2026-11-02-ios-keyboard-reranker-memory.md)：同一个扩展内存上限下的另一块（重排器和 ONNX Runtime 内存池），与本篇相互独立。部分重叠，互链。
- [键盘根视图改用普通 UIView](2026-11-05-ios-keyboard-plain-root-view.md)：#10944 反复弹出收起的累积已由它解决，本篇处理的是单次显示期间的占用。部分重叠，互链。
