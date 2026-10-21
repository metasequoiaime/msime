# Agent Note: macOS 设置页的主题卡片预览 canvas 皮肤

Status: implemented

## Problem

`base = "canvas"` 的皮肤包已经在 macOS 原生候选窗里画出九宫格面板、内容与外留白、拼音行距和自由挂件（见 [macOS 候选窗绘制 canvas 皮肤](../../implemented/feature/2026-10-19-macos-candidate-canvas.md)），但共享 React 设置页的主题卡片仍只画可读回退配色。`ExternalSkinCard` 读的是 `candidate` 调色板，`SkinSummary` 已经序列化并经 `msime_client_skin_catalog`（Tauri 侧 `scan_skin_catalog`）送到页面的 canvas 专属字段（`borderImage`、`contentInsetsDip`、`outerInsetsDip`、`lineGapDip`、`decorations`）一个都不画。用户在设置页看不出自己选的九宫格是否生效，只能保存后打开候选窗验证。

## Decision

共享 React 的主题卡片按同一份 catalog 几何画 canvas 预览，不新增宿主接口，也不新增取数路径：

- `packages/ui/src/skin/skin-canvas-preview.ts` 把 `SkinSummary` 的 canvas 字段换算成预览几何：`stretchDip` 盖过 `sliceDip` 当切片、`widthDip` 缺省取切片、浅色用 `lightSource`、四边简写按 1/2/4 值展开、挂件按 `zIndex` 分成面板之下（小于 1）和面板之上两层。取值规则与 Windows `CandidateCanvas.h` 和网页引擎 `candidates.js` 的 `canvasGeometry` 相同。
- `SkinCandidatePreview` 收到几何后画九宫格面板（`.skin-panel` 的 `border-image`，切片按源图像素、边宽按 DIP、四边拉伸）和挂件层（`.skin-layer` 里绝对定位的 `.skin-picture`）。面板图片成功解码前保留可读回退底色；面板画出来时不再叠包的背景图，与 Windows 和网页引擎一致。
- 面板读取一次，先由浏览器图片加载器确认成功，再同时启用横竖预览的 `border-image` 与 `panelCandidate` 贴图配色；待加载、读取失败或解码失败都保留 `candidate` 可读回退。皮肤 ID、明暗图片、读取器或重扫代数变化立即取消旧结果；图片解码与读取都复用 `useAsyncGeneration` 管理代次，过期回调不能覆盖新代数。
- 图片仍走既有的有界读取器：设置页的 `readSkinImage`（macOS Tauri 的 `read_skin_image`，与 `msime_client_skin_resource` 是同一份 client-services 包内资源校验），不读清单里的地址。
- 内容与外留白、拼音行距写成候选预览上的自定义属性，由既有的 `external-skin-decorated` utility 里的选择器读取：内容内边距每边不小于九宫格边宽，外留白是 `.containerParent` 的内边距，`line_gap_dip` 是拼音行的下外边距。
- `ExternalSkinCard` 只在包声明 `canvas = true` 时计算这份几何；其余包（包括只画顶部装饰带的包）与以前逐位相同，不传 canvas 时不改变 DOM 或样式。

## Historical audit

- [macOS 候选窗绘制 canvas 皮肤](../../implemented/feature/2026-10-19-macos-candidate-canvas.md)：部分重叠；候选窗的原生渲染、资源缓存和能力协商保留，本篇只补设置卡片预览，双方互链。
- [iOS 外部候选皮肤选择器绘制真实画布预览](../../implemented/feature/2026-10-20-ios-canvas-settings-preview.md)：部分重叠；同样只补设置页预览、不新增候选窗，但 iOS 复用它的原生画布视图，本篇在共享 React 里按网页引擎的 DOM/CSS 路径画，双方互链。
- [跨平台候选窗的渲染边界与共享契约](../../implemented/architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)：保留；没有新增候选视图或主题 ABI，调色板仍经 `msime_client_resolve_theme` 的共享解析结果。

## Alternatives considered

- **在 React 里用 `<canvas>` 离屏渲染整张面板**：与 Windows `CandidateCanvasPreview.h` 的像素路径最接近，九宫格缩放、越界切片和挂件 contain 可以逐像素对齐。但需要重新实现裁切、DPI 缩放和图片解码，jsdom 也没有 2D 上下文，卡片预览的测试只能靠替身；浏览器已经内置 `border-image` 的九宫格语义，所以改用它。
- **让 macOS 原生设置窗口（`SkinSettingsView.mm`）自己画预览**：能直接复用原生 `CandidateCanvas` 绘制层，像素与候选窗一致。但 macOS 的设置界面是共享 React 页（Tauri 外壳），原生视图只在输入法内，主题卡片不经过它；为它单独维护一份预览会多出一条路径，还修不到别的宿主。
- **新增一个 skin canvas resource 接口，让宿主返回已合成的预览图**：页面最省事，也能让所有平台像素一致。但 catalog 已经把 canvas 字段送到页面，图片也有有界的 `read_skin_image`，新增接口只是把同一份数据换一个形状；因此不新增接口。

## Consequences

- **收益**：macOS 主题卡片能直接看出 canvas 包的九宫格、内容与外留白、拼音行距和自由挂件，画的是与候选窗同一份 catalog 几何；Windows 的原生设置页早就有等价的离屏预览，这次补齐共享页的缺口，其余承载共享 React 页的宿主同时受益。
- **代价与已知上限**：同一款包同时写 `outerInsetsDip` 和装饰带时，装饰图按容器父级高度定位，落点可能有几像素偏差。预览不播放动画，也不替代真实候选窗的定位、材质和缩放验收。挂件定位需要更精确时再改为离屏合成。
- 只覆盖共享 React 的候选皮肤卡片；外观页的 `ExternalAppearancePreview` 与皮肤编辑器仍不画 canvas（编辑器的 `candidate-skin-draft.ts` 明确不提供 canvas 底）。

## Verification

`apps/desktop/tests/candidate/external-skins.test.tsx` 的「canvas card draws the nine-slice panel, its insets and the line gap instead of a placeholder」在实现前失败（`--msime-canvas-content-top` 读到空串），实现后通过：断言内容内边距不小于九宫格边宽、外留白、`data-canvas` / `data-canvas-line-gap`、面板的 `border-image-source` / `border-image-slice` / `border-image-width`、`.container[data-panel]`，以及自由挂件的锚点偏移。同一文件其余 66 项与 `pnpm --filter @msime/desktop test` 全绿，`pnpm format:check` 通过。


审查回归补充：`external-skins.test.tsx` 与图片 URL、代次复用邻接测试合计 74 项通过，覆盖面板待加载、解码失败、浅深配色切换、重扫立即撤下旧面板与过期回调。类型检查、生产构建、笔记校验通过。Chrome 在发布 CSP 下运行实际 `ExternalSkinCard`：有效 PNG 解码后显示两份面板及贴图文字色，坏图和换代恢复可读底色；此项是共享组件浏览器回归，不代表原生候选窗设备验收。

远端复用构建树运行 `bash scripts/verify-local.sh --quick` 通过（2026-10-21，包含仓库契约与共享 Apple 桥构建；按改动范围跳过无关的原生宿主编译）。
