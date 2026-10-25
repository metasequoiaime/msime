# Windows settings UI source

Source: https://github.com/metasequoiaime/MSIME-Windows at remote default branch `develop`, commit `0eaa35eed1dd699b28883068f2909afe3a5902da`. A second pin, `04a8df56f86312474a069f4335a1b58da7afaa9e`, is named explicitly wherever it is the source. Upstream repository license: GPL-3.0. Everything described here is tracked commit content, not adjacent worktree edits.

## Copied assets

`src/upstream/settings-variables.css` comes from `ui-html/webview2/settings/ime-settings/src/styles/variables.css`. The SVG files in `src/assets/` come from that settings project's `public/assets/`, with category icons under `sidebar/`; `utilities.svg` is `public/assets/sidebar/utilities.svg`. The window control icons `minimize.svg`, `maximize.svg`, `restore.svg` and `close.svg` come from the same `public/assets/` directory at `04a8df56f86312474a069f4335a1b58da7afaa9e`, with only a final newline added. `plugins.svg`, the 扩展 page's puzzle piece, has no upstream counterpart: it is this repository's own drawing, in the stroke colour and weight of `expression.svg`.

`src/upstream/skin-toolbar-preview.html` is extracted from `ui-html/webview2/ftb/default.html` at `04a8df56f86312474a069f4335a1b58da7afaa9e`, following upstream `skin.ts` fillToolbar: retain only `.status-bar`, remove `#en`, `#fullwidth`, `#puncEn`, and remove descendant IDs. No scripts or host handlers are included.

Rules adapted from the upstream settings styles — `styles/sidebar.css`, `styles/components/sections.css` and `components/forms.css`, the module sheets `styles/modules/input.css`, `skin.css`, `candidate/style-h.css`, `candidate/style-v.css`, `titlebar.css`, `floating-toolbar.css` and `appearance.css` — live in `src/styles.css`, scoped to React-owned selectors. Declaration bodies are kept as upstream wrote them; only the selectors are rewritten. All of these sources are GPL-3.0.

## Fluent UI System Icons

`src/core/fluent-icons.tsx` carries path data from Microsoft's Fluent UI System Icons (https://github.com/microsoft/fluentui-system-icons), the 20px Regular variants, as static strings: the icons the HarmonyOS redesign draws on settings rows, the phone tab bar and the 我的 tab. The paths are unchanged copies of the ones in the redesign's icon map, which took them from that repository; no npm package is involved. The exceptions are `chat` and `home`, the phone tab bar's 对话 and 首页 tabs, which the redesign does not draw: their paths are unchanged copies of `icons/chat_20_regular.svg` and `icons/home_20_regular.svg` from `@fluentui/svg-icons@1.1.344`, the published form of the same repository, copied by hand rather than installed. License: MIT, Copyright (c) 2020 Microsoft Corporation.

## Layout and shared behaviour

The React layout adapts the sidebar, cards, and helpcode rows to the shared preferences client. Native checkbox/select semantics and focus indicators are retained deliberately: the page keeps platform `<select>` elements rather than reimplementing upstream's custom dropdown, so keyboard, screen-reader and forced-colors behaviour comes from the platform. Explicit save/reload and cross-page drafts use the shared revision protocol. Titlebar window controls are driven by the host-injected `windowControl` action, and maximize/restore icons follow the injected host state subscription. The palette follows the saved `theme` and `settings_theme` preferences, defaulting to upstream's dark appearance when both are left at `system`/`follow`.

Titlebar drag initiation follows `src/main.ts` at `04a8df56`: primary press, two-pixel Manhattan movement threshold, no second double-click press, and no drag/maximize on resize edges. The shared React host uses pointer cancellation, leave and window blur to discard pending gestures.

## Input, frequency and mixed candidates

Input mode and scheme controls follow `src/partials/input.html` and `src/modules/input.ts` at the base commit. Radio sizes, colors, dividers, and mode layout are adapted from `styles/components/forms.css` and `styles/modules/input.css`, retaining keyboard focus and forced-color support. The shared active `scheme` remains compatible with existing hosts; optional `last_chinese_scheme` preserves the Chinese choice while Japanese is active.

Frequency controls follow the same `input.html`: five modes and numeric choices 1–6. Shared validation follows `server/assets/config/config.toml` (1–10); existing values above 6 stay visible without truncation. Defaults are promote/1/1. The learning switch remains the Engine's independent master gate.

Mixed English/emoji/kaomoji controls follow `input.html` and `input.ts` at the same pinned Windows commit. Persisted defaults follow `config.toml`: English enabled with a two-character threshold, emoji and kaomoji disabled. The English threshold has eight choices and is disabled (but retained) when English mixing is off. Candidate generation and ordering remain owned by Engine.

The utilities category and eight local-mode toggles follow `src/partials/tools-settings.html`, with descriptions condensed for the shared cards. Clipboard history uses the shared bounded store and desktop host actions; system clipboard observation remains a host responsibility. Quick phrases are one of the four local dictionary kinds on the dictionary page, with the same create/edit/delete, import and export path as pinyin, wubi and English.

## Built-in themes

The dedicated skin category follows the layout of `src/partials/skin.html` at the same pinned commit, but its cards are the global themes of `crates/client-core/src/theme.rs` (`Preferences.global_theme`), not the upstream Fluent/WeChat/Graphite/Willow skins, which were removed together with their palette and decoration rules. The compact candidate previews are CSS adaptations for the React settings page, coloured from the theme catalog.

The light/dark preview switch of `src/modules/skin.ts` at `04a8df56f86312474a069f4335a1b58da7afaa9e` is kept for the `system` card only, the one theme whose appearance follows the platform. Preview state is UI-only, separate from the saved theme selection.

Skin cards render both candidate layouts with the first six fixed samples from `src/partials/candidate/candidate-wnd-h.html` and `candidate-wnd-v.html` at `04a8df56f86312474a069f4335a1b58da7afaa9e`. The React translation omits repeated `realContainer` IDs and unused hidden candidates 7–9. Scoped layout CSS comes from `src/styles/modules/candidate/style-h.css` and `style-v.css`; the vertical preview is constrained to the card width.

Each built-in theme card includes a static toolbar preview built from `src/upstream/skin-toolbar-preview.html`, with the toolbar rules from settings `floating-toolbar.css` adapted to card-local selectors. This preview is decorative: it does not drive the real floating toolbar, which each native host draws itself.

Skin card arrangement follows the pinned `skin.html` and `skin.css`: one column, title and selection/preview actions above three preview stages, with 20px/24px header padding and 9px/24px stage padding. Selection uses a native button with switch semantics and the upstream 38px/19px toggle geometry. Like upstream `bindSkinSwitch`, activating an already-selected skin keeps it selected. Saving uses the shared draft/revision workflow, not immediate WebView2 writes.

## External skins

External skin discovery connects the host-owned `scan_skin_catalog` command to `src/skin/external-skins.tsx`. Metadata, the refresh button, empty state, invalid-folder diagnostics, compatibility-gated selection and independent preview themes follow the same pinned Windows `skin.ts`/`skin.html`. Unlike upstream, the list also scans once as it mounts, as the macOS native fallback `SkinSettingsView` does, so the package the custom theme is using is listed without a manual refresh. Candidate preview colour validation and supported palette rules follow upstream; generated rules are scoped to a React-owned identifier, never a manifest-provided selector. The directory/metadata/diagnostic CSS follows the pinned `skin.css` declarations. Selection uses the shared revisioned draft. Preview overrides do not alter compatibility. Refresh errors keep the last catalog, and late responses from a replaced host are ignored. No local path is sent by the UI.

The external catalog exposes the pinned upstream `openSkinDirectory` action as a host-injected, argument-free `open_skin_directory` command. Like `server/src/settings/settings_app.cpp` at the same Windows commit, explicit opening creates the host's skin directory first; scanning remains read-only. Windows uses ShellExecuteW (with COM initialized on a dedicated thread); macOS and Linux pass one absolute path argument to their directory opener, never a shell command. Failures are sanitized, existing files are preserved, and the UI guards duplicate requests and stale completions. The automated tests stop at the host command and do not drive the platform file manager.

External candidate preview geometry follows the same pinned Windows `skin.ts` `candidatePreviewCss` and `skin.css` decoration rules: conditional `containerParent`, manifest top/width/minimum-width variables, a 118px ornament layer, separate stacking, no pointer interception, and visible card/stage overflow. Both candidate orientations receive this wrapper; toolbar and built-in previews retain their markup. Numeric values are finite and bounded before use; refresh removes obsolete geometry. Like upstream, minimum width is applied by the decoration rule only when decoration is enabled.

Sparse light palettes layer over dark palette rules as upstream does, instead of dropping all unspecified dark fields. Tests cover geometry bindings, CSSOM declarations, refresh/reset and palette layering.

## Palette, stylesheet and resource delivery

Palette delivery uses constructed, adopted stylesheets rather than inline `<style>` text. `src/skin/skin-palette.ts` applies scoped colours, preserves light override order, and removes only its own sheet. Theme changes and unmount clean up sheets; unsupported browsers show a fallback notice without weakening CSP.

External decoration images load through the host-owned `read_skin_image` command using the bounded core resource reader. The host returns image MIME types and bytes only; the shared UI creates an image data URL accepted by the existing desktop img-src CSP, without broadening it. Both preview orientations share one request and use an image element with the pinned 118px, right-aligned, contain geometry in place of upstream's background URL. SVG bytes are never inserted as markup. Missing or invalid images retain the base preview; refresh reloads unchanged filenames and discards late responses.

External toolbar styles connect the manifest-owned host reader to a constructed `@scope` around the external preview, following the fixed Windows `skin.ts` parse-then-insert approach. Ordinary rules, media/supports groups, `:root` mapping and light-theme selectors are wired; refresh and unmount remove owned sheets, and late reads cannot install stale rules. Browser capability or parse/read failure retains the base preview. CSP is unchanged.

Native CSS nesting is preserved by sanitizing the browser-parsed rule tree in place before scoped insertion. Parent declarations, nested selectors, media/supports groups and CSSNestedDeclarations retain their original ordering; this also preserves pseudo-element behavior that rebuilding declarations as an `&` rule would change. Unsupported rules are filtered at every depth, without discarding their supported parent rule.

Toolbar CSS image URLs resolve against the same validated skin package through the host image reader, then become bounded image-only data URLs before scoped adoption. Repeated relative names share one read; nested declarations and conditional groups use the same rewrite path. Remote, absolute and traversal references never reach the reader. Failed resources omit only their declaration. Preparation is tied to the card's refresh generation, so stale image reads cannot install an old sheet. Per preparation, 32 unique reads and 16 MiB resolved/expanded content budgets bound fan-out.

Image-set support includes string/URL choices, resolution descriptors, type hints and the WebKit alias, including raw expressions stored in custom properties. Browser parsing normalizes each expression before the bounded image rewriter. Bare strings are never assumed safe if an older parser cannot normalize them, and quoted non-resource text is preserved.

Literal `url()` payloads support CSS hexadecimal/simple escapes and quoted line continuations. Decoded names pass the unchanged package path allowlist before image reads; escaped traversal and remote URLs are rejected. Quoted non-resource text, including icon code points, is preserved. Semantics follow https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point. Image-set expressions accept the same escaped string/URL options and quoted line continuations, in custom properties and under the WebKit alias: a delimiter-aware scan preserves escaped quotes/parentheses and comments before browser normalization, and normalized URLs still pass package-path validation. Escaped outer function identifiers remain unsupported and are reported through the partial-support notice.

Top-level and conditional external `@import` rules are inlined by `src/skin/toolbar-imports.ts` before parsing, bounded to 16 imports, 8 levels of nesting and 16 MiB, with layer/supports/media conditions preserved and resource URLs rebased onto the importing file.

## Animations, variables and fonts

Toolbar keyframes receive per-installation private names, with matching animation-name longhands rewritten after browser shorthand parsing. Duration, delay, easing, fill mode, play state and priority remain intact. Media/supports conditions, native nesting and duplicate definition order are retained. Quoted and escaped names use the browser's keyframes grammar. Keyframe image URLs use the bounded package reader, cache and final resource sanitizer. See https://www.w3.org/TR/css-animations-1/ for the animation-name/keyframes contract.

Whole-value `var()` animation-name and animation shorthand references use private, mode-specific custom-property aliases. Original variables remain unchanged for non-animation consumers. Alias definitions stay in the original rules with the original priorities, so browser inheritance, media/nesting, fallbacks and dependency-cycle handling remain active. Referenced definitions and nested whole-value fallback chains are rewritten with the same private keyframe names. Alias expansion is bounded to 256 source/mode pairs, 32 nested fallback levels and 16 MiB of duplicated values. Semantics follow https://www.w3.org/TR/css-variables-1/. Fragment substitutions such as `pulse var(--duration)` remain partial support.

Pending variable animation shorthands are expanded before browser parsing, because Chromium drops their source when individual longhands override them. PostCSS 8.5.28 (MIT, locked for the build toolchain and an explicit UI dependency) preserves declaration order, comments and opaque custom values. Whole-value variables are projected into private per-longhand aliases; original custom properties are retained. This preserves later duration/play-state overrides, earlier important longhands and later shorthand resets through both image preparation and scoped installation. Browser validation remains in place; source maps are disabled and malformed input falls back to browser recovery with a partial notice. Source and expanded output are each capped at 16 MiB. The production JS bundle carries roughly 13 KiB gzip for this.

Animation custom-property names support non-ASCII characters and CSS hexadecimal/simple escapes. Definitions and references share decoded logical names through source projection and private animation aliasing; escaped names also reserve their decoded spelling to avoid namespace collisions. Resource validation treats only a valid `var()` name as opaque, leaving fallback URLs subject to all existing image/path checks.

Root selector mapping happens on browser-parsed CSSStyleRule selectors, not by replacing text throughout the source. Actual `:root` pseudo-classes become `:scope`, while quoted attribute values, escaped class-name colons, custom values, content and animation names remain untouched. Browser-canonical escaped root pseudo-classes and root rules nested under supported conditions work too.

External `@font-face` rules, including those inside media/supports groups, load through an injected font reader and the host-managed `read_skin_font` resource API. The desktop command accepts only package ID/relative path and font MIME types, retaining the core containment and 8 MiB per-resource limit. Binary FontFace loading needs no font URL or relaxed CSP. Private family names and family-variable aliases isolate preview cards; font shorthand family values and keyframe family declarations are rewritten where browser CSSOM exposes them. Media changes update registration in source order, cleanup removes listeners, media resources preload within the existing budgets, and false supports conditions do not read resources. Fonts are adopted only after current-generation preparation and removed with the card stylesheet; stale results never register. Limits are 8 font faces, 16 distinct source reads and 16 MiB total bytes per preparation. Browser behavior follows https://www.w3.org/TR/css-font-loading/. `local()` and data sources and `tech()` hints are not supported; this is skin-package font support for the settings preview, and is separate from the candidate font-family controls below.

Whole-value variable font shorthands use the same bounded source projection as animation shorthands, before CSSOM can discard their pending substitution. Each font longhand receives a private variable alias; original variables and declaration priority/order are retained. Family projection then passes through the per-card font namespace. This covers later size overrides, earlier important sizes, shorthand resets, inheritance, conditional definitions, nested fallback and cyclic fallback.

## Appearance page

The appearance page includes the candidate preview section from pinned `04a8df56` settings `appearance.html` / `appearance.ts`. It reuses the scoped candidate markup and built-in palettes, follows the current draft layout, font size, page size, preedit visibility, helpcode visibility and built-in skin selection, and resets on reload without writing preferences. Fixed sample candidates 7–9 come from the same `candidate-wnd-h.html` partial; skin cards still default to six samples. No Engine input or private data is used. The preview follows the settings host theme.

External skins also render in the appearance preview, following the same pinned `skin.ts` candidate palette/decoration path used on skin cards. The selected package is resolved through the host catalog on entry, selection or host changes, and manual refresh. Draft layout, size, count and visibility continue to update without writes. Palette validation, dimension bounds and the bounded image reader are shared with the existing previews; no toolbar stylesheet is applied to the candidate-only appearance stage. Unsupported layouts/themes, missing packages and resource failures are explicit, not silent substitutions. Catalog generations discard late results when selection, host or page changes; refresh and leaving the page remove owned styles and discard pending image results.

Hidden preedit retains the upstream `.pinyin` row and marks its container `preedit-hidden`, matching pinned `appearance.ts` instead of removing the row. This keeps the adjacent-sibling selectors used by Willow green for leading corner highlights and padding. The hidden row is explicitly `display:none` so the preview's flex rule cannot override HTML hidden behavior.

Candidate and preedit font-size controls expose every integer from 12 to 32, matching pinned `appearance.html` and the client-core preference bounds. The shared UI declares and saves `candidate_preedit_font_size` independently, and falls back to the core's defaults when a value is missing: 18px for candidates and 15px for the preedit. Intermediate sizes such as 19 and 27 remain selected correctly. Bounded numeric CSS properties drive both built-in and external previews without weakening CSP. Native hosts keep their own platform ranges — the macOS native settings window still offers its 16/18/20 point list — and read the same shared fields.

The appearance page exposes the candidate text colour with the pinned `appearance.html` colour input/reset controls and `appearance.css` geometry/state styling. The colour is `custom_theme.candidate_colors.text`, one of the custom theme's seven pickers: setting a six-digit hex value selects the `custom` global theme (the theme selected before becomes `custom_theme.base`, so its palette stays underneath) and sets candidate text plus the upstream 0x9d-alpha number colour; reset saves null, leaves the selected theme alone and removes both preview variables so the base's colours return. Theme-following state is exposed through `aria-pressed`. Only bounded hex values become CSS; malformed values cannot inject declarations.

The core font preference contract accepts Unicode primary and fallback family names and up to 32 ordered fallbacks, matching pinned `04a8df56` `appearance.ts`, which permits 32 supplementary families. ASCII names, empty fallback lists, defaults and the per-name 128 UTF-8-byte bound are unchanged. Names are display strings, not paths or CSS declarations; consumers quote and encode them for their own rendering interface.

Shared appearance edits the primary family and up to 32 ordered fallback families with add/remove/reorder actions, using that Unicode/128-byte name contract. Invalid drafts show an error across settings pages and cannot submit. Both built-in and external previews use quoted literal family names followed by the system sans-serif fallback, like pinned `appearance.ts`; control characters, quotes and backslashes are escaped without interpreting names as CSS syntax. Other fallback selections are excluded from a control's own suggestions.

The font controls take an optional `SettingsClient.listFontFamilies` reader and present searchable comboboxes when it is available; the desktop shell supplies it through the `list_font_families` command backed by `crates/host-api`'s `system_fonts`. Loading is lazy and shared by the primary and fallback controls; refresh, failure, empty and unsupported states remain distinct. Unicode search, pointer selection, arrow/Enter/Escape navigation and active-option scrolling retain manual entry. Catalogs validate all names, deduplicate and cap at 16,384 entries; each popup shows at most 100 search results with a refinement notice. Reader changes discard old results without changing drafts.

## Credential testing

The settings page adapts pinned Windows credential testing from `529060a` at upstream `4e5c96248e868ca5beca3f9cdc0c513a354b9cb0` through an optional host interface, covering Tencent, NiuTrans and custom translation, AI assistance, voice ASR and voice polishing. Windows, macOS, Linux and iOS each inject their own `testApiCredential` adapter. Unlike the Windows settings process, the Linux WebView and Tauri shell do not own private AI, Tencent or voice secrets: the shell routes a bounded `credential_test` request to the user-owned provider socket, which reloads its owner-only file, binds the selected provider/model/endpoint to that private credential and returns only a fixed bounded status. NiuTrans and custom translation reuse the credential fields already present in their normal provider query contract.

## Local verification

React tests cover the draft/save/reload behaviour, catalog lifecycles, stale readers and the bounds described above. The browser-level regressions run real Chromium under the desktop CSP against bundles of the actual shared code, with no relaxed policy and no user data:

```sh
node scripts/build-skin-browser.mjs <temporary-directory>   # skin palette, toolbar CSS, images, fonts
node scripts/build-settings-browser.mjs <temporary-directory> # the settings page with an in-memory host
```

Serve the chosen directory on a loopback HTTP port, then run `scripts/test-skin-palette-csp.py` or `scripts/test-appearance-preview.py` with `--url http://127.0.0.1:<port>`, `--csp` from the desktop Tauri config and optionally `--executable <installed-chromium>` (and `--screenshot` for the appearance test). Python Playwright is required; stop the temporary server afterward. Between them these fixtures check computed styles, conditional rules, root mapping, scope containment and cleanup, declaration order and pseudo-elements, decoded image pixels and image-set deduplication, fixed-time animation playback with overridden longhands, real font rendering and measured glyph-width fallback, candidate/preedit text sizes for every 12–32 value in both orientations, selected skin colours on built-in and external previews, and rejection of remote or unprepared resources. `scripts/skin-font-fixture.json` contains an original synthetic rectangular glyph font (1000 units/em and advance), not copied font artwork.

## Mascot illustrations

`src/assets/mascot/` 的十张图来自仓库负责人于 2026-10-19 提供的「水杉娘四季／幼年」角色素材：`settings.webp` 对应「修设置.png」，`welcome.webp` 对应「欢迎.png」，`empty.webp`、`offline.webp`、`downloading.webp`、`updated.webp` 分别对应「应用-无结果.png」「应用-断网.png」「应用-下载词库.png」「应用-更新完成.png」。上述六张与下述三张在「场景」目录；`input.webp` 对应「拼音积木.png」，`privacy.webp` 对应「隐私保护.png」，`sync.webp` 对应「云同步.png」；`notes.webp` 对应「表情／47-记笔记.png」。未采用网络下载的图片或第三方角色。

原图为 1536×1536 的浅色背景 PNG。接入时去除与边缘相连的近白色背景，保留角色内部浅色衣服与道具，按透明区域裁切并等比缩放到留有 8px 边距的 256×256 透明 WebP（质量 70，透明通道无损保留）。显示尺寸不超过 128 CSS 像素，沿用本仓库第一方资源的 GPL-3.0 授权。此次替换原四张状态图，新增设置、欢迎、拼音积木、记笔记、隐私保护与云同步六张；旧状态图由项目用 Seedream 5.0 pro 生成的来源说明仅适用于替换前版本。

## AI 服务商品牌图标

`src/assets/providers/` 来自 [Lobe Icons](https://github.com/lobehub/lobe-icons/tree/8a1b2821bb768c484fe8da20c5e1838b6bf551a9/packages/static-svg/icons)，固定默认分支 `master` 的提交 `8a1b2821bb768c484fe8da20c5e1838b6bf551a9`，MIT 许可（Copyright (c) 2023 LobeHub）；完整许可随图标保存在 `src/assets/providers/LICENSE`。品牌标志的商标权仍归各自所有者。

本地 `claude.svg`、`gemini.svg`、`deepseek.svg`、`qwen.svg`、`kimi.svg`、`zhipu.svg`、`siliconcloud.svg`、`openrouter.svg` 分别原样复制同名的 `*-color.svg`，保留完整品牌配色；`openai.svg` 原样复制同名黑白标志，显示在白色底板上。`groq.svg` 来自同名 SVG，仅把 `currentColor` 换成该提交 `src/Groq/style.ts` 的 `COLOR_PRIMARY`（`#F55036`）。Kimi 白色笔画和 OpenRouter 亮绿标志使用深色底板，其他品牌使用白色底板，避免选中高亮和深浅主题改写品牌颜色。

`everyapi.svg` 为 EveryAPI 公开站点的 [favicon](https://everyapi.ai/favicon.svg)，于 2026-10-19 获取，原样用于标识该服务商，品牌权利归 EveryAPI。自定义与未知服务商使用已有 Fluent 连接图标。

AI 服务商选择是原生 select 原则的局部例外：该控件用 combobox/listbox 呈现选项内图标，自行维护键盘、焦点、定位与高对比度；其他 select 继续使用系统控件，鸿蒙手机继续使用整行选择面板。

## 共享 AI 服务目录

`settings/ai-provider-options.ts` 读取 `settings/ai-provider-catalog.json`，事实源是 `crates/client-providers/src/ai/provider_catalog.rs`。十三项标题、默认地址、默认模型、建议模型和文档沿用原有值与顺序；空建议列表和空文档不导出字段，保留原 UI 对象形状。

```sh
cargo run --quiet --locked -p msime-client-providers --example export_ai_provider_catalog -- --write
pnpm exec vp fmt packages/ui/src/settings/ai-provider-catalog.json
cargo run --quiet --locked -p msime-client-providers --example export_ai_provider_catalog -- --check
```

同一工具生成 Windows 设置消费的 `shared/ai/AiProviderCatalog.h`，静态宽字符串表保留原 `Provider` 字段与惰性初始化生命周期。Linux 设置应用 CI 使用真实导出器检查 JSON 语义和 C++ 字节；消费者不增加运行时文件或网络访问。平台过滤、Ollama 免 Token、自定义字段优先和未知服务回退继续由原策略处理；iOS、语音和偏好首次默认值的独立策略不由本目录替换。

## 共享语音目录

识别、润色服务预设和豆包流式接口读取 `voice/voice-provider-catalog.json`，事实源是 `crates/client-providers/src/voice/provider_catalog.rs`。修改 Rust 目录后在仓库根目录生成，再检查：

```sh
mkdir -p target
cargo run --quiet --locked -p msime-client-providers --example export_voice_provider_catalog -- --write
pnpm exec vp fmt packages/ui/src/voice/voice-provider-catalog.json
cargo run --quiet --locked -p msime-client-providers --example export_voice_provider_catalog -- --check
```

导出器同时生成鸿蒙 `VoiceProviderCatalog.ts` 的润色与 HTTP 识别默认地址、模型和 HTTP 请求格式，润色器、识别策略与初始配置同步读取；HTTP 服务范围由 Rust `asr_request_format` 筛选，流式与本地服务不进入上传路径；Linux 的 `msime_voice_provider_catalog.py` 同步生成服务集合、默认地址/模型与 HTTP 格式，由原生语音脚本导入并随各版本安装；无参时仍只向标准输出生成 JSON。Linux 设置应用 CI 在构建 Rust 依赖后执行同一个检查，JSON 比较语义值，TypeScript 和 Python 逐字比较。JSON 随前端打包，宿主无需增加异步桥接，读取预设不会访问网络或偏好。UI 只追加系统识别和本地模型，并保持公开凭据服务数组的历史顺序；选择器顺序由 Rust 目录决定。

macOS 同时读取同一导出器生成的 `shared/voice/VoiceProviderCatalog.h`，共享 C++17 静态表持有识别服务的 id、标题、默认地址与模型，`--check` 逐字检查。宿主只保存历史 id 顺序，并追加系统与本地识别；不增加运行时 ABI 初始化、JSON 解析或配置读取。

## 共享润色提示词

内置方案的名称与正文归 `crates/client-providers/src/voice/polish_catalog.rs`。修改它后，在仓库根目录运行：

```sh
cargo run --quiet --locked -p msime-client-providers --example export_polish_prompt_catalog -- --write
pnpm exec vp fmt packages/ui/src/voice/polish-prompt-catalog.json packages/ui/src/voice/polish-preset-ids.ts
cargo run --quiet --locked -p msime-client-providers --example export_polish_prompt_catalog -- --check
```

导出工具同步更新 UI JSON、公开 id 字面量元组、共享 C++ 窄字符串目录头、Windows 设置页的宽字符串目录头、Linux Python 模块、iOS 的内置方案 Swift 目录、Android 的内置方案 Java 目录和鸿蒙简版正文的 TypeScript 类，Linux 设置应用 CI 检查它们。鸿蒙简版在 Rust 条目的 `compact_prompt` 中维护，不进入 UI JSON；`HarmonyVoicePolisher` 同步调用生成类，四篇正文、空槽位和未知 id 的 cleanup 回退保持。macOS 回退设置的名称与 id 也读取 C++ 窄字符串目录。`scripts/test-polish-prompt-catalog.py` 用真实 C++ 解析函数、Windows 设置目录、Python 模块、编译后的 Swift 内置方案目录，以及 Android 和 macOS 实际设置选择表对照 UI JSON；没有 C++17、Swift 编译器、JDK 17 或 Apple 宿主时分别明确跳过对应部分。Java 生成方法每次返回独立数组，Android 和 macOS 设置策略追加三个自定义槽位。自定义槽位、未知方案与空正文的既有处理继续由各消费方的回归覆盖。
