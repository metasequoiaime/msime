# Agent Note: 设置页候选预览按宿主原生 token 绘制（先做 macOS）

Status: implemented

## Problem

macOS 的「主题」页打开的是共享 React 设置页，卡片里的候选预览是 `SkinCandidatePreview` 画的一段 HTML，样式来自 `packages/ui/src/styles.css` 的 `@utility skin-card-preview`。这套样式是从 MSIME-Windows 04a8df56 搬来的旧 Windows 候选窗样式，对所有平台一样。`system` 主题没有调色板（`theme-catalog.json` 里 `candidate: null`），主题和皮肤包没定的槽位也都落到它写死的默认值上：浅色选中底 `#e8e8e8`、选中行黑字、无条件画一根强调色竖条、卡片圆角 6px、行圆角 4px、边框 1.5px、内边距 1–2px。

macOS 原生候选窗画的是 `NativeCandidateTokens`（`platforms/macos/src/candidate/CandidateSkin.cpp`）：强调色实心选中、白字、82% 白序号、不画竖条，卡片 10pt、行 6pt、1pt 细边框、内边距 4pt（`CandidateSkin.h` 的 `SkinTokens` 默认值），行上下各留 6pt（`CandidateChrome.h` 的 `MSIMECandidateRowPadding`）。于是 #7099 里「跟随系统」的预览是灰底、绿竖条、黑字、小圆角，实际窗口却是绿色实心、白字、大圆角。

另外三处让预览不反映实际：`candidatePaletteStyle` 只输出 bg/border/text/num/accent/selected/hover，主题的 `selected_text`、`selected_number`、`show_selected_bar` 到不了预览；「主题」页的卡片不套用用户在「候选窗口 → 窗口样式」设的圆角和不透明度，而 macOS 原生回退页 `CandidateSkinPreviewView` 是套用的；圆角滑块在「跟随皮肤」时写死显示 6，macOS 实际跟随的是 10。

## Decision

**预览的默认值按宿主平台给。** 设置页根节点已经带 `data-platform`（macOS 上是 `mac`，见 `platform-tokens.ts` 的 `settingsPlatformOf`）。`skin-card-preview` 里新增三块 `:is([data-platform="mac"]) &[data-preview-theme…]`：浅色、深色各一块写颜色 token，共用一块写选中规则和几何。几何不再写死，改读 `--msime-host-card-radius`、`--msime-host-row-radius`、`--msime-host-border-width`、`--msime-host-pad`、`--msime-host-row-pad-y`，回退值就是原来的数，没有宿主块的平台画法不变。卡片圆角是 `var(--msime-skin-radius, var(--msime-host-card-radius, 6px))`：皮肤包的 `corner_radius_dip` 和用户设的圆角都写在 `--msime-skin-radius` 上，先于宿主默认值，与原生 `ResolveSkin` / `StyledCandidateSkin` 的顺序一致。

**主题槽位仍压过宿主默认值。** 调色板写在元素的内联样式上，宿主块写在样式表里，层叠关系就是原生「主题槽位叠在原生 token 上」。macOS 块里 `--cand-selected: var(--cand-accent)`，调色板只换了强调色时选中底也跟着换，对应 `SkinManifestBridge.mm` 里 `tokens.selected = tokens.accent`。

**选中前景色和竖条开关进预览。** `candidatePaletteStyle` 多写 `--cand-selected-text`、`--cand-selected-num`，`show_selected_bar` 为 boolean 时写 `--cand-selected-bar: block | none`，null 不写。选中行的文字和序号读这两个变量，没有时退回普通行的颜色；`.first::before` 的 `display` 读 `--cand-selected-bar`，没有时画。选中行悬停时保持选中底（`.first:hover` 写在 `.cand:hover` 之后，特异性相同、后者胜出）：选中前景色是按选中底配的，悬停换成 `--cand-hover` 后，macOS 浅色的白字会落在近白的卡片上，深色选中底的自定义主题在所有平台都一样。原生 macOS 是 `selectedHover = selected`，Windows 的选中底本来就是悬停底，所以这条对各平台都是照原生画。皮肤包 `show_selected_bar = false` 原来走 `skin-palette.ts` 的 `selectedBarCss`（`!important` 的 `display:none`），这条路保留，它和变量给的是同一个结论；包写 `true` 时只有变量能在 macOS 上把竖条重新画出来。

**macOS 的选中前景色推导。** 调色板给了选中底却没给选中文字色时（`system` 底上的皮肤包，或只挑了强调色的自定义主题），原生走 `DeriveSelectedForegrounds`：选中底不透明度低于 0.85 时文字、序号退回普通行颜色，否则按相对亮度 0.45 取白或 #1A1A1A，序号 82%。CSS 算不了对比度，所以 `candidatePaletteStyle` 在 TS 里按同一条规则算好，写成 `--cand-mac-selected-text` / `--cand-mac-selected-num`；只有 macOS 宿主块读它们（`--cand-selected-text: var(--cand-mac-selected-text, #ffffff)`），其他平台不受影响。纯数值亮度公式由[公共相对亮度方法](../simplification/2026-10-15-shared-relative-luminance.md)提供，本篇的透明度与阈值规则保持独立。

**用户的窗口样式画到每张卡上。** 用户圆角对行圆角的限制（`--msime-candidate-row-radius`）和不透明度的 `color-mix` 从 `appearance-candidate-preview` 挪进 `skin-card-preview`；「主题」页的主题卡片和皮肤包卡片（`ExternalSkinCard` 的 `windowStyle`）都铺上 `candidateWindowStyle(draft)`，包卡片的背景图同样乘上不透明度。缩放不铺：zoom 只在 `appearance-candidate-preview` 上生效，和原生预览一样不在卡片上放大。

**滑块跟随值按平台。** `CandidateWindowStyleSection` 多一个 `platform`，「跟随皮肤」时 macOS 显示 10，其他平台仍是 6，与预览画的卡片圆角一致。

**三份 token 的同步。** macOS 的值在 `CandidateSkin.cpp`/`CandidateSkin.h`/`CandidateChrome.h`、`styles.css` 和 `followedCornerRadius` 里各有一份。`apps/desktop/tests/candidate/candidate-preview-native-tokens.test.ts` 用 `?raw` 读原生源码，解析出 token 后与样式表里的宿主块逐项比对；`tests/settings/candidate-window-style-section.test.tsx` 的 macOS 滑块期望值同样从 `CandidateSkin.h` 的 `radius` 解析。改了一边而没改另一边会红。

## Alternatives considered

- **让 Rust 的 `resolve_theme` 为 `system` 返回当前平台的 token** — 只有一处来源，预览和所有宿主读到同一份解析结果，不会漂移。但 `system` 的 `candidate: null` 是共享契约，宿主据此决定「用自己的原生 token」：macOS、Windows、Linux 各自的原生 token 在宿主代码里，搬进 Rust 要改所有宿主读到的共享解析结果和 C ABI，而且原生端拿到非 null 调色板后会走「主题叠在原生 token 上」的那条路，`DeriveSelectedForegrounds` 之类的分支行为会变。本 issue 只是设置页预览画错，不值得改共享契约。
- **把 `--cand-mac-selected-*` 换成给 `candidatePaletteStyle` 传宿主平台** — 平台显式出现在函数签名里，名字里不用带 `mac`。但调用处有主题页卡片、候选窗口页预览、外部皮肤卡片和外部皮肤预览四处，每处都要把平台一路传下去；样式表已经按 `data-platform` 选默认值，推导结果只给 macOS 块读，就不用再传一遍平台。
- **删掉 `selectedBarCss` 那条 `!important` 规则，竖条只走变量** — 一个开关只有一条路。但那条路还连着样式表注入失败时的提示（`useSelectedBarPalette` 的 `paletteFailed`），删它是另一件事；两条路对 `false` 的结论相同，不冲突。
- **Windows、Linux 的宿主块一起补上** — 同一个根因在两边都存在（Linux 原生是 Adwaita token，Windows 是 Fluent token）。但 Windows 是旗舰平台，改它的预览是用户能看到的变化，应该在对应平台上逐项核对后再改；本 PR 只能在 macOS 上做视觉验收。另开 issue 处理。

## Consequences

- **收益**：macOS 上「跟随系统」和主题、皮肤包没定的槽位，预览画的是原生候选窗的选中色、白字、无竖条、10/6 圆角和 4 的内边距；内置主题的选中行文字色在所有平台的预览里都按主题画；「主题」页卡片随用户的圆角和不透明度变化。
- **用户可见的跨平台变化**：内置主题（shuishan、light、paper、night、ink）的预览选中行文字从普通文字色变成主题的 `selected_text`（强调色），Windows、Linux 也一样。这是对的：两边原生都画 `selected_text`（`platforms/windows/src/candidate/CandidatePalette.h`、`platforms/linux/src/candidates/CandidateColors.h`）。
- **已知差异**：行高、26pt 的首行、翻页指示器「1 / N ‹ ›」、横排为释义预留的高度（cc8105f5d8）、嵌入式预编辑时首行不显示拼音，这些预览都不画；首尾高亮与悬停的圆角差异由[内圆角决定](2026-10-14-macos-candidate-inset-radius.md)消除，用户圆角和最小内边距按同一公式绘制。竖条颜色仍读 `--accent-strong`，macOS 块不改它，免得连带改掉悬浮工具栏预览和光标的颜色。
- **代价与重访信号**：macOS token 有三份，靠对照测试维持一致；`CandidateSkin.cpp` 的写法（`Rgb(0x…, a)`、`Rgba{…}`、`if (dark) { … } else { … }`）变了，测试的解析要跟着改。Windows、Linux 补宿主块时沿用同一组 `--msime-host-*` 变量和对照测试的做法；补完后 `followedCornerRadius` 也要按平台改。

## Verification

- `pnpm --filter ./apps/desktop exec vitest run tests/candidate/candidate-preview-native-tokens.test.ts`：调色板输出、macOS 推导规则、选中行和竖条的样式规则、宿主块与原生源码的对照、几何变量的回退值。
- `tests/settings/skin-page-window-style.test.tsx`、`tests/settings/candidate-window-style-section.test.tsx`、`tests/candidate/external-skins.test.tsx`：主题页上的主题卡片和已安装皮肤包的卡片都铺上用户窗口样式；滑块跟随值按平台，macOS 的值对照原生源码，「候选窗口」页把宿主平台传给滑块。
- 选中行悬停的规则顺序在 `candidate-preview-native-tokens.test.ts` 里检查（jsdom 不算 `:hover`，所以查的是两条规则在横排、竖排里的先后）。

共享 UI 的装饰几何与背景透明度纯计算归属见[外部皮肤预览几何](../simplification/2026-10-15-skin-preview-geometry.md)，窗口透明度的覆盖及叠加规则保持本篇约定。
