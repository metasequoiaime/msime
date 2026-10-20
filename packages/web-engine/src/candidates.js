// 候选栏：按水杉桌面端候选框的结构和样式画预编辑行、候选和翻页标记，配色和几何来自 skin.js 的 resolveSkin。
//
// 隔离：整个候选栏画在宿主元素 <msime-candidates> 的 Shadow DOM 里，页面的 CSS 改不到它，它的样式也漏不出去。样式表是可构造样式表（adoptedStyleSheets），每个皮肤的取值只经 `style.setProperty` 写到 Shadow DOM 内的根元素上，不用 <style> 元素，也不写 style 属性的文本，所以页面的 CSP 不开 `style-src 'unsafe-inline'` 也能用。装饰图用 <img> 加载；canvas 面板只使用宿主生成且已校验的 border-image 属性，不执行包内 CSS。
//
// 页面能改的只有 `::part()` 暴露出来的部分，见 PARTS 和 README。
import { PLATFORM_PALETTE, resolveSkin } from "./skin.js";

const LAYOUTS = new Set(["horizontal", "vertical"]);
const ALIGNS = new Set(["left", "center", "right"]);
// 候选栏与锚点、与视口边缘之间留的距离（px）。
const GAP = 4;
const MARGIN = 4;
const PROPERTY = /^--[A-Za-z0-9-]+$/;

/** 宿主元素的标签名；它没有注册成自定义元素，只是一个页面 CSS 不会顺手选中的名字。 */
export const CANDIDATE_BAR_TAG = "msime-candidates";

/** `::part()` 名字，页面可以用 `msime-candidates::part(candidate)` 这样的选择器改样式。 */
export const PARTS = Object.freeze({
  /** 整个候选栏（定位的那一层，含装饰带）。 */
  candidates: "candidates",
  /** 画出来的候选框：底色、边框、圆角、阴影。 */
  card: "card",
  /** 预编辑行（拼音或编码）。 */
  preedit: "preedit",
  /** 预编辑行末尾的光标。 */
  caret: "caret",
  /** 翻页标记。 */
  paging: "paging",
  /** 每个候选。 */
  candidate: "candidate",
  /** 高亮的候选，同时也带 candidate。 */
  highlight: "highlight",
  /** 候选前的序号。 */
  number: "number",
  /** 候选文字。 */
  text: "text",
  /** 候选后的辅助码提示（帧里有时），或编码提示（helpcode 选项打开时）。 */
  code: "code",
  /** 皮肤的装饰图。 */
  decoration: "decoration",
  /** 皮肤的背景图。 */
  background: "background",
});

// 桌面端候选框的样式。类名沿用 msime-windows 候选框（ui-html/webview2/candwnd）的写法，规则由 packages/ui/src/styles.css 的 `skin-card-preview`、`external-skin-decorated` 两个 @utility 和 packages/ui/src/upstream/candidate-themes 的候选框页面改写成普通 CSS。
//
// Source: MSIME-Windows 04a8df56 candidate window styles (candwnd style-h.css / style-v.css, skin.ts candidatePreviewCss / skin.css), GPL-3.0.
//
// 根元素上先声明每个用到的自定义属性的默认值（跟随系统时的桌面端原生配色，即 skin.js 的 `PLATFORM_PALETTE`），皮肤再用 setProperty 覆盖；这样页面在祖先元素上声明的同名自定义属性也继承不进来。
function platformVariables(palette) {
  return [
    ["--cand-bg", palette.surface],
    ["--cand-border", palette.border],
    ["--cand-text", palette.text],
    ["--cand-num", palette.number],
    ["--cand-hover", palette.hover],
    ["--cand-selected", palette.selected],
    ["--cand-accent", palette.accent],
    ["--accent-strong", palette.accent],
  ]
    .map(([name, value]) => `  ${name}: ${value};`)
    .join("\n");
}

const SHEET = `
:host {
  all: initial !important;
}
.candidate {
  --cand-font-family: "Segoe UI", "PingFang SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif;
  --cand-font-size: 16px;
  --cand-shadow: 4px 4px 12px rgba(0, 0, 0, 0.12);
${platformVariables(PLATFORM_PALETTE.light)}
  --cand-secondary: var(--cand-num);
  --cand-selected-text: var(--cand-text);
  --cand-selected-num: var(--cand-num);
  --msime-skin-radius: 6px;
  --msime-skin-min-width: 0px;
  --msime-skin-card-floor: 7em;
  --msime-skin-preedit-margin-top: 0px;
  --msime-skin-preedit-margin-right: 0px;
  --msime-skin-preedit-margin-bottom: 0px;
  --msime-skin-preedit-margin-left: 0px;
  --msime-skin-shadow-scale: 1;
  --msime-skin-decoration-top: 0px;
  --msime-skin-decoration-width: 0px;
  --msime-skin-selected-bar: block;
  --msime-skin-border-width: 1.5px;
  --msime-skin-item-radius: 4px;
  --msime-skin-item-padding-right: 4px;
  --msime-skin-selected-bar-width: 0.2em;
  --msime-skin-selected-bar-height: 0.8em;
  --msime-skin-background-fit: cover;
  --msime-skin-background-opacity: 1;
  --msime-skin-animation-duration: 0ms;
  --msime-skin-animation-stagger: 0ms;
  --msime-max-width: calc(100vw - ${2 * MARGIN}px);
  position: fixed;
  z-index: 2147483647;
  left: 0;
  top: 0;
  display: block;
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  pointer-events: none;
  font-family: var(--cand-font-family);
  font-size: var(--cand-font-size);
  font-weight: normal;
  font-style: normal;
  line-height: normal;
  letter-spacing: normal;
  text-align: left;
  text-transform: none;
  direction: ltr;
  writing-mode: horizontal-tb;
  color: var(--cand-text);
  white-space: normal;
  cursor: default;
  user-select: none;
  -webkit-user-select: none;
  -webkit-tap-highlight-color: transparent;
}
.candidate[data-theme="dark"] {
  --cand-shadow: 5px 5px 10px rgba(0, 0, 0, 0.5);
${platformVariables(PLATFORM_PALETTE.dark)}
}
.candidate[data-animation-enter="fade"] .container {
  animation: msime-candidate-fade var(--msime-skin-animation-duration) ease-out both;
}
.candidate[data-animation-enter="fade_scale"] .container {
  animation: msime-candidate-fade-scale var(--msime-skin-animation-duration) cubic-bezier(.16, 1, .3, 1) both;
}
.candidate[data-animation-enter="slide_up"] .container {
  animation: msime-candidate-slide-up var(--msime-skin-animation-duration) cubic-bezier(.16, 1, .3, 1) both;
}
.candidate[data-animation-enter="spring"] .container {
  animation: msime-candidate-spring var(--msime-skin-animation-duration) cubic-bezier(.22, 1.35, .36, 1) both;
}
.candidate[data-animation-enter="glow"] .container {
  animation: msime-candidate-glow var(--msime-skin-animation-duration) ease-out both;
}
.candidate[data-animation-enter="fade_scale"][data-entering] .row-wrapper,
.candidate[data-animation-enter="slide_up"][data-entering] .row-wrapper {
  animation: msime-candidate-row-reveal var(--msime-skin-animation-duration) cubic-bezier(.16, 1, .3, 1) both;
  animation-delay: calc(var(--msime-candidate-row-index, 0) * var(--msime-skin-animation-stagger));
}
.candidate[data-animation-selection="spring"] .cand.first[data-selecting] {
  animation: msime-candidate-spring var(--msime-skin-animation-duration) cubic-bezier(.22, 1.35, .36, 1) both;
}
.candidate[data-animation-selection="glow"] .cand.first[data-selecting] {
  animation: msime-candidate-glow var(--msime-skin-animation-duration) ease-out both;
}
.candidate[data-animation-selection="fade"] .cand.first[data-selecting],
.candidate[data-animation-selection="fade_scale"] .cand.first[data-selecting],
.candidate[data-animation-selection="slide_up"] .cand.first[data-selecting] {
  animation: msime-candidate-select-fade var(--msime-skin-animation-duration) ease-out both;
}
.candidate[data-exiting="fade"] .container {
  animation: msime-candidate-fade var(--msime-skin-animation-duration) ease-in reverse both;
}
.candidate[data-exiting="fade_scale"] .container,
.candidate[data-exiting="spring"] .container {
  animation: msime-candidate-fade-scale var(--msime-skin-animation-duration) ease-in reverse both;
}
.candidate[data-exiting="slide_up"] .container {
  animation: msime-candidate-slide-up var(--msime-skin-animation-duration) ease-in reverse both;
}
.candidate[data-exiting="glow"] .container {
  animation: msime-candidate-glow var(--msime-skin-animation-duration) ease-in reverse both;
}
@keyframes msime-candidate-fade { from { opacity: 0; } to { opacity: 1; } }
@keyframes msime-candidate-fade-scale { from { opacity: 0; transform: translateY(5px) scale(.96); } to { opacity: 1; transform: translateY(0) scale(1); } }
@keyframes msime-candidate-slide-up { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
@keyframes msime-candidate-spring { from { transform: scale(.94); } 65% { transform: scale(1.025); } to { transform: scale(1); } }
@keyframes msime-candidate-glow { from { filter: brightness(1.22); } to { filter: brightness(1); } }
@keyframes msime-candidate-select-fade { from { opacity: .45; } to { opacity: 1; } }
@keyframes msime-candidate-row-reveal { from { opacity: 0; transform: translateY(3px); } to { opacity: 1; transform: translateY(0); } }
@media (prefers-reduced-motion: reduce) {
  .candidate[data-animation-enter] .container,
  .candidate[data-animation-enter] .row-wrapper,
  .candidate[data-animation-selection] .cand.first,
  .candidate[data-exiting] .container { animation: none !important; }
}
.candidate[hidden] {
  display: none !important;
}
.candidate *,
.candidate *::before {
  box-sizing: border-box;
}
.candidate.wnd-h {
  --msime-candidate-pad-x: 1px;
  --msime-candidate-pad-y: 2px;
}
.candidate.wnd-v {
  --msime-candidate-pad-x: 2px;
  --msime-candidate-pad-y: 2px;
}

/* ---- 候选框 ---- */
.containerParent {
  position: relative;
  width: fit-content;
  max-width: var(--msime-max-width);
}
.container {
  position: relative;
  pointer-events: auto;
  margin: 0;
  padding: var(--msime-skin-padding, var(--msime-candidate-pad-y)) var(--msime-skin-padding, var(--msime-candidate-pad-x));
  background: var(--cand-bg);
  border: var(--msime-skin-border-width) solid var(--cand-border);
  border-radius: var(--msime-skin-radius);
  box-shadow: var(--cand-shadow);
  min-width: max(var(--msime-skin-card-floor, 7em), var(--msime-skin-min-width));
  width: max-content;
  max-width: var(--msime-max-width);
  overflow-wrap: anywhere;
  word-break: break-word;
}
.wnd-v .container {
  max-width: min(720px, var(--msime-max-width));
}
/* 皮肤的背景图：盖在底色上、候选下面，按候选框的圆角裁切。 */
.container[data-background] {
  overflow: hidden;
  isolation: isolate;
}
.container[data-background] > :not(.skin-background-image) {
  position: relative;
}
.skin-background-image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: var(--msime-skin-background-fit);
  opacity: var(--msime-skin-background-opacity);
  pointer-events: none;
}
.skin-background-image[hidden],
.skin-decoration-image[hidden] {
  display: none;
}

/* Canvas 的挂件只参与绘制，留白由父层参与窗口测量；面板与文字的层级单独隔离。 */
.skin-stage { position: relative; isolation: isolate; }
.skin-layer { position: absolute; inset: 0; pointer-events: none; }
.skin-layer-below { z-index: 0; }
.skin-layer-above { z-index: 2; }
.skin-stage > .container { z-index: 1; }
.skin-picture { position: absolute; pointer-events: none; }
.skin-picture > img, .skin-picture > canvas { width: 100%; height: 100%; object-fit: contain; }
.skin-picture[hidden], .skin-panel[hidden] { display: none; }
.skin-panel { position: absolute; inset: 0; z-index: 0; border: 0 solid transparent; pointer-events: none; }
.skin-panel > canvas { position: absolute; inset: 0; width: 100%; height: 100%; }
.container[data-panel] { background: transparent; border: 0; border-radius: 0; box-shadow: none; }
.container[data-panel] > .row, .container[data-panel] > .row-wrapper { position: relative; z-index: 1; }
.candidate[data-canvas] .container {
  padding: var(--msime-canvas-content-top) var(--msime-canvas-content-right) var(--msime-canvas-content-bottom) var(--msime-canvas-content-left);
}
.candidate[data-canvas] .containerParent {
  max-width: none;
  padding: calc(var(--msime-canvas-outer-top) + var(--msime-skin-decoration-top)) var(--msime-canvas-outer-right) var(--msime-canvas-outer-bottom) var(--msime-canvas-outer-left);
}
.candidate[data-canvas] { overflow: clip; }
.candidate[data-canvas] .pinyin { margin-bottom: var(--msime-canvas-line-gap, 0px); }

/* ---- 行 ---- */
.row {
  margin-top: 2px;
  padding-top: 0;
  padding-bottom: 0;
}
.wnd-h .row {
  margin-bottom: 1px;
}
.wnd-v .row {
  padding-left: 2px;
  padding-right: 12px;
}
.text {
  display: block;
  min-width: 0;
  max-width: 100%;
  padding-left: calc(0.2em + 3px);
  color: var(--cand-text);
  overflow-wrap: anywhere;
  word-break: break-word;
}
.wnd-h .text {
  padding-right: 5px;
}

/* 预编辑行：左边是拼音或编码和光标，右边是翻页标记。 */
.pinyin {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 0;
}
.cursor {
  display: inline-block;
  margin-left: 1px;
  width: 1.5px;
  height: 1.2em;
  vertical-align: top;
  background: var(--accent-strong);
  transform: translateY(10%);
}
.paging {
  flex: none;
  padding-right: 4px;
  font-size: 0.8em;
  color: var(--cand-num);
  letter-spacing: 2px;
}
.paging > [data-off] {
  opacity: 0.35;
}

/* 共享版式采用来源的双层阴影；只缩放透明度，换回旧皮肤时仍使用旧宿主默认。 */
.candidate[data-shadow] .container {
  --msime-shadow-outer: 0.18;
  --msime-shadow-inner: 0.10;
  box-shadow: 8px 10px 24px rgb(0 0 0 / calc(var(--msime-shadow-outer) * var(--msime-skin-shadow-scale))),
    2px 3px 8px rgb(0 0 0 / calc(var(--msime-shadow-inner) * var(--msime-skin-shadow-scale)));
}
.candidate[data-theme="dark"][data-shadow] .container {
  --msime-shadow-outer: 0.34;
  --msime-shadow-inner: 0.22;
}
.candidate[data-shadow="none"] .container,
.candidate[data-shadow] .container[data-panel] { box-shadow: none; }
.candidate[data-canvas][data-shadow] { overflow: visible; }
.candidate[data-font-family] { font-family: var(--msime-skin-font-family), var(--cand-font-family); }
.candidate[data-layout] .container { display: flex; flex-direction: column; }
.candidate[data-layout] .pinyin {
  margin: var(--msime-skin-preedit-margin-top, 0px) var(--msime-skin-preedit-margin-right, 0px)
    var(--msime-skin-preedit-margin-bottom, 0px) var(--msime-skin-preedit-margin-left, 0px);
}
.candidate[data-layout] .pinyin[hidden] { display: none; }
.candidate[data-layout][data-canvas] .pinyin {
  margin-bottom: calc(var(--msime-canvas-line-gap, calc(var(--msime-skin-preedit-margin-bottom, 0px) + 2px)) - 2px);
}
.candidate-items { position: relative; z-index: 1; margin-top: 2px; }
.candidate[data-trim-preedit] .candidate-items { margin-top: 0; }
.candidate[data-layout] .candidate-items .row-wrapper { margin: 0; }
.candidate[data-layout] .candidate-items .cand { margin: 0; }
.candidate[data-layout] .cand {
  padding: calc(var(--msime-skin-item-extra-height, 0px) / 2) var(--msime-skin-item-padding-right)
    calc(var(--msime-skin-item-extra-height, 0px) / 2) 0;
}
.candidate[data-layout] .cand .text { padding-left: max(3px, var(--msime-skin-number-inset, 3px)); padding-right: 0; line-height: 1.35; }
.candidate[data-layout] .num { margin-left: 0; }
.candidate[data-fit-content] .cand { padding-right: max(3px, var(--msime-skin-number-inset, 3px)); }
.wnd-h .candidate-items { display: flex; flex-wrap: wrap; gap: var(--msime-skin-horizontal-gap, 0px); }
.wnd-v .candidate-items { display: flex; flex-direction: column; gap: var(--msime-skin-vertical-gap, 0px); }
.candidate[data-justify-rows].wnd-h .candidate-items > .row-wrapper { flex-grow: 1; }
.candidate[data-layout] .paging-row { display: flex; justify-content: flex-end; margin-top: 2px; }
.candidate-body { display: flex; position: relative; z-index: 1; }
.candidate-body > .candidate-items { flex: 1; min-width: 0; }
.candidate-body > .paging-row { flex: none; align-items: flex-end; }

/* ---- 候选 ---- */
.row-wrapper {
  position: relative;
}
.wnd-h .row-wrapper {
  margin-right: var(--msime-skin-horizontal-gap, 0px);
  display: inline-block;
  vertical-align: top;
  max-width: 100%;
  min-width: 0;
}
.wnd-v .row-wrapper { margin-bottom: var(--msime-skin-vertical-gap, 0px); }
.cand {
  position: relative;
  min-width: var(--msime-skin-item-min-width, 0px);
  border-radius: var(--msime-skin-item-radius);
  cursor: pointer;
}
.wnd-h .cand {
  padding: calc(2px + var(--msime-skin-item-extra-height, 0px) / 2) var(--msime-skin-item-padding-right) calc(2px + var(--msime-skin-item-extra-height, 0px) / 2) 0;
}
.wnd-v .cand {
  padding-top: calc(1px + var(--msime-skin-item-extra-height, 0px) / 2);
  padding-bottom: calc(1px + var(--msime-skin-item-extra-height, 0px) / 2);
}
.cand .text {
  display: flex;
  align-items: baseline;
}
.wnd-h .cand .text {
  padding-right: 4px;
  line-height: 1.25;
}
.wnd-v .cand .text {
  line-height: 1.35;
}
.cand-content {
  min-width: 0;
}
.num {
  flex: none;
  font-size: var(--msime-skin-number-size, 0.8em);
  min-width: var(--msime-skin-number-column, 0px);
  margin-left: var(--msime-skin-number-inset, 0px);
  margin-right: var(--msime-skin-number-gap, 0px);
  color: var(--cand-num);
}
.wnd-v .num {
  margin-right: var(--msime-skin-number-gap, 1.5px);
}
.cand-helpcode {
  margin-left: 0.15em;
  font-size: 0.85em;
  color: var(--cand-secondary);
}
/* 悬停只在鼠标真的在候选栏里移动过之后才生效，候选栏弹出在鼠标底下时不会误亮（桌面端的 hover-active）。 */
.candidate[data-hover] .cand:not(.first):hover {
  background-color: var(--cand-hover);
}
.first {
  background-color: var(--cand-selected);
}
.first .text {
  color: var(--cand-selected-text);
}
.first .num {
  color: var(--cand-selected-num);
}
/* 高亮行的编码提示跟序号同色：次要文字色在微信、柳绿这类实心高亮底上几乎看不见，序号色是皮肤为高亮底挑过的。 */
.first .cand-helpcode {
  color: var(--cand-selected-num);
}
/* 高亮候选左边的选中条；皮肤的 showSelectedBar 为 false 时 --msime-skin-selected-bar 是 none，不画。 */
.first::before {
  content: "";
  position: absolute;
  top: 53%;
  transform: translateY(-50%);
  left: var(--msime-skin-selected-bar-left, 0px);
  height: max(6px, calc(var(--msime-skin-selected-bar-width) * 2), var(--msime-skin-selected-bar-height));
  width: max(3px, var(--msime-skin-selected-bar-width));
  display: var(--msime-skin-selected-bar);
  background: var(--accent-strong);
  border-radius: 8px;
}
.wnd-v .first::before {
  top: 50%;
}

/* ---- 装饰带 ----
 * 与桌面端各平台一致（Windows CandidateWindow.cpp）：整体比候选框高出 decoration-top 的透明装饰带，候选框从装饰带下面开始；装饰图宽 decoration-width、保持比例，底边落在候选框上沿往下 pad_y 处，盖在候选框上；太高时等比缩小（contain，贴底和对齐的那一侧）；水平方向相对候选框定位：left 贴左 pad_x，center 居中，right（默认）贴右 pad_x，且不越过候选框左边。 */
.candidate[data-decorated] .containerParent {
  --msime-skin-decoration-bottom: calc(var(--msime-skin-decoration-top) + var(--msime-candidate-pad-y));
  padding-top: var(--msime-skin-decoration-top);
}
.skin-decoration-image {
  display: none;
}
.candidate[data-decorated] .skin-decoration-image {
  display: block;
  position: absolute;
  z-index: 2;
  bottom: calc(100% - var(--msime-skin-decoration-bottom));
  left: max(0px, calc(100% - var(--msime-candidate-pad-x) - var(--msime-skin-decoration-width)));
  width: var(--msime-skin-decoration-width);
  height: auto;
  max-height: var(--msime-skin-decoration-bottom);
  object-fit: contain;
  object-position: right bottom;
  pointer-events: none;
}
.candidate[data-decorated] .container {
  z-index: 1;
}
.candidate[data-decoration-align="left"] .skin-decoration-image {
  left: var(--msime-candidate-pad-x);
  object-position: left bottom;
}
.candidate[data-decoration-align="center"] .skin-decoration-image {
  left: max(0px, calc(50% - var(--msime-skin-decoration-width) / 2));
  object-position: center bottom;
}
.pinyin .text { color: var(--cand-preedit-text, var(--cand-text)); }
.cursor { background: var(--cand-caret, var(--accent-strong)); }
.first::before { background: var(--cand-selected-bar-color, var(--accent-strong)); }
.first .cand-helpcode { color: var(--cand-selected-secondary, var(--cand-selected-num)); }
`;

// 可构造样式表只能被创建它的文档采用，所以按文档各建一份，同一文档里的候选栏共用。
const sheets = new WeakMap();

function sheetFor(doc) {
  let sheet = sheets.get(doc);
  if (sheet) return sheet;
  const view = doc.defaultView;
  if (!view || !("adoptedStyleSheets" in doc) || typeof view.CSSStyleSheet?.prototype.replaceSync !== "function") {
    throw new Error("@msime/web-engine: the candidate bar needs constructable stylesheets (adoptedStyleSheets)");
  }
  sheet = new view.CSSStyleSheet();
  sheet.replaceSync(SHEET);
  sheets.set(doc, sheet);
  return sheet;
}

function checkLayout(layout) {
  if (!LAYOUTS.has(layout)) throw new TypeError(`unknown layout: ${layout}`);
  return layout;
}

function checkDark(dark) {
  if (dark !== "auto" && dark !== true && dark !== false) throw new TypeError(`dark must be "auto", true or false: ${dark}`);
  return dark;
}

function finite(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function element(doc, tag, className, part) {
  const node = doc.createElement(tag);
  if (className) node.className = className;
  if (part) node.setAttribute("part", part);
  return node;
}

// 换图片地址时先撤掉上一张加载失败留下的 hidden；地址相同就不动，免得每帧重新加载。
function setImage(img, url) {
  if (img.getAttribute("src") === url) return;
  img.hidden = false;
  img.setAttribute("src", url);
}

/** 图片保留浏览器原生动画；减弱动态效果时用 canvas 的首帧绘制，不读回或导出跨域像素。 */
function skinPicture(doc, url, reduced, onReady) {
  const node = element(doc, "div", "skin-picture");
  const image = element(doc, "img");
  image.alt = "";
  image.draggable = false;
  let alive = true;
  let loaded = false;
  const display = () => {
    if (!alive || !loaded) return;
    if (reduced()) {
      const still = element(doc, "canvas");
      still.width = image.naturalWidth;
      still.height = image.naturalHeight;
      still.getContext("2d").drawImage(image, 0, 0);
      node.replaceChildren(still);
    } else node.replaceChildren(image);
  };
  const ready = () => {
    if (!alive) return;
    loaded = image.naturalWidth > 0 && image.naturalHeight > 0;
    node.hidden = !loaded;
    display();
    onReady?.(loaded);
  };
  const error = () => {
    if (!alive) return;
    loaded = false;
    node.hidden = true;
    onReady?.(false);
  };
  image.addEventListener("load", ready);
  image.addEventListener("error", error);
  node.hidden = true;
  image.src = url;
  return { node, image, get loaded() { return loaded; }, display,
    dispose() { alive = false; image.removeEventListener("load", ready); image.removeEventListener("error", error); node.remove(); } };
}

/** 九宫格的源像素边距和目标 CSS px 边距分别缩到各自可用范围，四角保持固定边宽。 */
function nineSlice(imageWidth, imageHeight, width, height, border) {
  const axis = (length, before, after) => {
    const scale = Math.min(1, length / (before + after || 1));
    return [0, before * scale, length - after * scale, length];
  };
  const sx = axis(imageWidth, border.slice.left, border.slice.right);
  const sy = axis(imageHeight, border.slice.top, border.slice.bottom);
  const dx = axis(width, border.width.left, border.width.right);
  const dy = axis(height, border.width.top, border.width.bottom);
  return Array.from({ length: 9 }, (_, i) => {
    const x = i % 3, y = Math.floor(i / 3);
    return { sx: sx[x], sy: sy[y], sw: sx[x + 1] - sx[x], sh: sy[y + 1] - sy[y],
      x: dx[x], y: dy[y], width: dx[x + 1] - dx[x], height: dy[y + 1] - dy[y], draw: border.fill || i !== 4 };
  });
}

function px(node, key, value) { node.style.setProperty(key, `${value}px`); }

/**
 * 创建一个候选栏。skin 是内置皮肤 id、皮肤对象或 undefined（默认皮肤），见 skin.js 的 resolveSkin；layout 为 "horizontal" 或 "vertical"；dark 为 "auto"（跟随 prefers-color-scheme）、true 或 false；点击第 i 个候选时调用 onPick(i)；helpcode 为 true 时在候选后显示编码；帧里有辅助码提示（page[i].hint）时总是显示提示。皮肤 id 未知时抛 TypeError。
 */
export function createCandidateBar({ skin, layout = "horizontal", dark = "auto", onPick, container = document.body, helpcode = false } = {}) {
  const doc = container.ownerDocument ?? document;
  const view = doc.defaultView ?? globalThis;
  const sheet = sheetFor(doc);
  const media = view.matchMedia?.("(prefers-color-scheme: dark)") ?? null;
  const motionMedia = view.matchMedia?.("(prefers-reduced-motion: reduce)") ?? null;

  let skinArg = skin;
  let layoutArg = checkLayout(layout);
  let darkArg = checkDark(dark);
  let frame = null;
  let anchor = null;
  let destroyed = false;
  let applied = [];
  let pictures = [];
  let panelPicture = null;
  // 这次组字里画过的最高高度，隐藏时清零。
  let tallest = 0;
  let hideTimer = null;
  // 上一帧高亮的格位，隐藏时清空：每帧都重建候选行，只有高亮换了格才给新高亮那一行挂选中动效，打字不会重放。
  let shownHighlight = null;

  const isDark = (value) => (value === "auto" ? Boolean(media?.matches) : value);
  let resolved = resolveSkin(skinArg, { dark: isDark(darkArg), layout: layoutArg });

  const host = doc.createElement(CANDIDATE_BAR_TAG);
  const shadow = host.attachShadow({ mode: "open" });
  shadow.adoptedStyleSheets = [sheet];

  const root = element(doc, "div", "candidate", PARTS.candidates);
  root.hidden = true;
  root.setAttribute("role", "listbox");
  root.setAttribute("aria-label", "候选");
  const parent = element(doc, "div", "containerParent");
  const decoration = element(doc, "img", "skin-decoration-image", PARTS.decoration);
  decoration.alt = "";
  const card = element(doc, "div", "container", PARTS.card);
  const background = element(doc, "img", "skin-background-image", PARTS.background);
  background.alt = "";
  const stage = element(doc, "div", "skin-stage");
  const below = element(doc, "div", "skin-layer skin-layer-below");
  const above = element(doc, "div", "skin-layer skin-layer-above");
  below.setAttribute("aria-hidden", "true");
  above.setAttribute("aria-hidden", "true");
  const panel = element(doc, "div", "skin-panel");
  panel.setAttribute("aria-hidden", "true");
  stage.append(below, card, above);
  parent.append(decoration, stage);
  root.append(parent);
  shadow.append(root);

  // 装饰图、背景图加载失败就不画，候选框照常显示（桌面端解码失败时也是这样）。
  const onImageError = (e) => {
    e.currentTarget.hidden = true;
  };
  decoration.addEventListener("error", onImageError);
  background.addEventListener("error", onImageError);

  // 点候选时不要让文本框失焦。
  const onMouseDown = (e) => e.preventDefault();
  const onPointerMove = () => {
    root.dataset.hover = "";
  };
  const onClick = (e) => {
    const cand = e.target instanceof view.Element ? e.target.closest(".cand") : null;
    if (!cand || !root.contains(cand)) return;
    const index = Number(cand.dataset.index);
    if (Number.isInteger(index)) onPick?.(index);
  };
  root.addEventListener("mousedown", onMouseDown);
  root.addEventListener("pointermove", onPointerMove);
  root.addEventListener("click", onClick);

  const reduced = () => Boolean(motionMedia?.matches);
  const setPalette = (active) => {
    const variables = active ? resolved.variables : resolved.fallbackVariables;
    for (const [key, value] of Object.entries(variables)) {
      if (key.startsWith("--cand-") || key === "--accent-strong" || key === "--msime-skin-selected-bar") root.style.setProperty(key, value);
    }
    if (active) card.dataset.panel = "";
    else delete card.dataset.panel;
    panel.hidden = !active;
  };
  const layoutCanvas = () => {
    const canvas = resolved.geometry.canvas;
    if (!canvas) return;
    const width = card.offsetWidth, height = card.offsetHeight;
    const bounds = root.getBoundingClientRect();
    const layerBounds = stage.getBoundingClientRect();
    // 挂件裁到预留弹窗范围，卡片阴影则可以延伸到范围外。
    const clip = `inset(${bounds.top - layerBounds.top}px ${layerBounds.right - bounds.right}px ${layerBounds.bottom - bounds.bottom}px ${bounds.left - layerBounds.left}px)`;
    below.style.setProperty("clip-path", clip);
    above.style.setProperty("clip-path", clip);
    for (const picture of pictures) {
      const deco = picture.decoration;
      if (!deco) continue;
      const x = deco.anchor.includes("left") || deco.anchor === "left" ? 0 : deco.anchor.includes("right") || deco.anchor === "right" ? width - deco.width : (width - deco.width) / 2;
      const y = deco.anchor.includes("top") || deco.anchor === "top" ? 0 : deco.anchor.includes("bottom") || deco.anchor === "bottom" ? height - deco.height : (height - deco.height) / 2;
      px(picture.node, "left", x + deco.offsetX);
      px(picture.node, "top", y + deco.offsetY);
    }
    if (!panelPicture?.loaded) return;
    const image = panelPicture.image;
    const border = canvas.borderImage;
    if (reduced()) {
      panel.style.removeProperty("border-image-source");
      let still = panel.firstChild;
      if (!still) { still = element(doc, "canvas"); panel.append(still); }
      const scale = view.devicePixelRatio || 1;
      still.width = Math.ceil(width * scale); still.height = Math.ceil(height * scale);
      const context = still.getContext("2d"); context.scale(scale, scale);
      for (const tile of nineSlice(image.naturalWidth, image.naturalHeight, width, height, border)) {
        if (tile.draw && tile.sw > 0 && tile.sh > 0 && tile.width > 0 && tile.height > 0) context.drawImage(image, tile.sx, tile.sy, tile.sw, tile.sh, tile.x, tile.y, tile.width, tile.height);
      }
    } else {
      panel.replaceChildren();
      // 这里只生成固定的 border-image 属性；URL 与数值均由 resolveSkin 校验，包内 CSS 不会执行。
      panel.style.setProperty("border-image-source", `url("${border.url}")`);
      panel.style.setProperty("border-image-slice", `${Object.values(border.slice).join(" ")}${border.fill ? " fill" : ""}`);
      panel.style.setProperty("border-image-width", Object.values(border.width).map((n) => `${n}px`).join(" "));
    }
  };
  const clearCanvas = () => {
    for (const picture of pictures) picture.dispose();
    pictures = []; panelPicture = null;
    panel.replaceChildren(); panel.removeAttribute("style"); below.replaceChildren(); above.replaceChildren();
    panel.hidden = true;
    delete card.dataset.panel;
  };
  const applyCanvas = () => {
    clearCanvas();
    const canvas = resolved.geometry.canvas;
    if (!canvas) { delete root.dataset.canvas; return; }
    root.dataset.canvas = "";
    const border = canvas.borderImage?.width;
    for (const side of ["top", "right", "bottom", "left"]) {
      const base = side === "top" || side === "bottom" ? 2 : resolved.layout === "vertical" ? 2 : 1;
      const content = canvas.contentInsets ? `${Math.max(canvas.contentInsets[side], border?.[side] ?? 0)}px`
        : `calc(${border?.[side] ?? 0}px + var(--msime-skin-padding, ${base}px))`;
      root.style.setProperty(`--msime-canvas-content-${side}`, content);
      root.style.setProperty(`--msime-canvas-outer-${side}`, `${canvas.outerInsets?.[side] ?? 0}px`);
    }
    if (canvas.lineGap !== null) root.style.setProperty("--msime-canvas-line-gap", `${canvas.lineGap}px`);
    else root.style.removeProperty("--msime-canvas-line-gap");
    for (const deco of canvas.decorations) {
      const picture = skinPicture(doc, deco.url, reduced);
      picture.decoration = deco;
      px(picture.node, "width", deco.width); px(picture.node, "height", deco.height);
      (deco.zIndex < 1 ? below : above).append(picture.node);
      pictures.push(picture);
    }
    if (canvas.borderImage) {
      panelPicture = skinPicture(doc, canvas.borderImage.url, () => false, (loaded) => {
        if (destroyed) return;
        setPalette(loaded);
        if (frame && !root.hidden) place();
      });
      pictures.push(panelPicture);
      setPalette(false);
    }
  };

  // 把 resolveSkin 的结果写到根元素上：先清掉上一个皮肤写的属性，再逐个 setProperty 写 variables。图片地址来自已校验的 geometry；先由 <img> 确认可用，面板失败时保留可读回退。
  const applySkin = () => {
    for (const name of applied) root.style.removeProperty(name);
    applied = [];
    root.className = `candidate wnd-${resolved.layout === "vertical" ? "v" : "h"}`;
    const geometry = resolved.geometry.layout;
    root.toggleAttribute("data-layout", Boolean(geometry));
    root.toggleAttribute("data-font-family", Boolean(resolved.geometry.fontFamily));
    root.toggleAttribute("data-fit-content", geometry?.fitContent === true);
    root.toggleAttribute("data-justify-rows", geometry?.justifyRows === true);
    if (geometry?.shadow != null) root.dataset.shadow = geometry.shadow;
    else delete root.dataset.shadow;
    if (resolved.geometry.layout) root.style.setProperty("--msime-skin-number-column", "9px");
    else root.style.removeProperty("--msime-skin-number-column");
    root.dataset.theme = resolved.dark ? "dark" : "light";
    root.dataset.animationEnter = resolved.animation.enter;
    root.dataset.animationSelection = resolved.animation.selection;
    for (const [name, value] of Object.entries(resolved.variables)) {
      if (!PROPERTY.test(name) || (typeof value !== "string" && typeof value !== "number")) continue;
      root.style.setProperty(name, String(value));
      applied.push(name);
    }

    applyCanvas();
    const { decoration: deco } = resolved.geometry;
    const bg = resolved.geometry.canvas?.borderImage ? null : resolved.geometry.background;
    if (deco) {
      root.dataset.decorated = "";
      root.dataset.decorationAlign = ALIGNS.has(deco.align) ? deco.align : "right";
      setImage(decoration, deco.url);
    } else {
      delete root.dataset.decorated;
      delete root.dataset.decorationAlign;
      decoration.removeAttribute("src");
    }
    if (bg) {
      card.dataset.background = "";
      setImage(background, bg.url);
    } else {
      delete card.dataset.background;
      background.removeAttribute("src");
    }
  };

  const draw = () => {
    // 只有从隐藏到显示的这一帧错峰揭开候选行；之后的帧重建的行不再动。
    const entering = root.hidden;
    if (entering) root.dataset.entering = "";
    else delete root.dataset.entering;
    const rows = [];
    if (resolved.geometry.canvas?.borderImage) rows.push(panel);
    if (card.dataset.background !== undefined) rows.push(background);

    const pinyin = element(doc, "div", "row pinyin", PARTS.preedit);
    const geometry = resolved.geometry.layout;
    const preeditHidden = Boolean(geometry) && !String(frame.preedit ?? "");
    pinyin.hidden = preeditHidden;
    root.toggleAttribute("data-trim-preedit", preeditHidden && (geometry.trimHiddenPreeditGap === true || geometry.itemCornersFollowCard === true));
    const reading = element(doc, "div", "text");
    reading.append(String(frame.preedit ?? ""), element(doc, "span", "cursor", PARTS.caret));
    pinyin.append(reading);
    let pager = null;
    if ((frame.hasPrev || frame.hasNext) && geometry?.pageArrows !== false) {
      const paging = element(doc, "div", "paging", PARTS.paging);
      paging.setAttribute("aria-hidden", "true");
      const prev = element(doc, "span", "page-prev");
      prev.textContent = geometry?.pageArrowStyle === "triangle" ? "◀" : "‹";
      if (!frame.hasPrev) prev.dataset.off = "";
      const next = element(doc, "span", "page-next");
      next.textContent = geometry?.pageArrowStyle === "triangle" ? "▶" : "›";
      if (!frame.hasNext) next.dataset.off = "";
      paging.append(prev, next);
      if (geometry) {
        pager = element(doc, "div", "paging-row");
        pager.append(paging);
      } else pinyin.append(paging);
    }
    rows.push(pinyin);

    const vertical = resolved.layout === "vertical";
    const page = Array.isArray(frame.page) ? frame.page : [];
    const items = geometry ? element(doc, "div", "candidate-items") : null;
    page.forEach((item, index) => {
      const highlighted = index === frame.highlight;
      const wrapper = element(doc, "div", index === 0 && !vertical ? "row-wrapper first-row-wrapper" : "row-wrapper");
      wrapper.style.setProperty("--msime-candidate-row-index", String(index));
      const cand = element(doc, "div", highlighted ? "row cand first" : "row cand", highlighted ? `${PARTS.candidate} ${PARTS.highlight}` : PARTS.candidate);
      cand.dataset.index = String(index);
      if (highlighted && shownHighlight !== null && shownHighlight !== index) cand.dataset.selecting = "";
      cand.setAttribute("role", "option");
      cand.setAttribute("aria-selected", String(highlighted));
      const text = element(doc, "div", "text");
      const num = element(doc, "span", vertical ? "num cand-no" : "num", PARTS.number);
      num.textContent = String(index + 1);
      const content = element(doc, "span", "cand-content", PARTS.text);
      content.textContent = String(item?.text ?? "");
      text.append(num, content);
      // 引擎给的辅助码提示（打开辅助码时才有，自带括号）优先；没有时 helpcode 选项打开才显示编码。
      const note = item?.hint ? String(item.hint) : helpcode && item?.code ? `(${item.code})` : "";
      if (note) {
        const code = element(doc, "span", "cand-helpcode", PARTS.code);
        code.textContent = note;
        content.append(code);
      }
      cand.append(text);
      wrapper.append(cand);
      if (items) items.append(wrapper);
      else rows.push(wrapper);
    });
    if (items && pager && !vertical) {
      // 横排为箭头保留右侧窄列；竖排仍在列表下方另起一行。
      const body = element(doc, "div", "candidate-body");
      body.append(items, pager);
      rows.push(body);
    } else {
      if (items) rows.push(items);
      if (pager) rows.push(pager);
    }
    card.replaceChildren(...rows);
    shownHighlight = Number.isInteger(frame.highlight) && frame.highlight >= 0 && frame.highlight < page.length ? frame.highlight : null;
    root.hidden = false;
    place();
  };

  // 与 macOS 候选窗（CandidatePanel.mm）一致：整个候选栏（含装饰带）放在锚点正下方，下面放不下时翻到锚点上方，再夹在视口里。纵排时用这次组字里最高的一页决定上下，翻页时不会因为某页变高而跳到另一侧。
  const place = () => {
    const html = doc.documentElement;
    const vw = html.clientWidth || view.innerWidth;
    const vh = html.clientHeight || view.innerHeight;
    const outer = resolved.geometry.canvas?.outerInsets;
    root.style.setProperty("--msime-max-width", `${Math.max(0, vw - 2 * MARGIN - (outer?.left ?? 0) - (outer?.right ?? 0))}px`);
    root.style.setProperty("left", "0px");
    root.style.setProperty("top", "0px");
    layoutCanvas();
    const { width, height } = root.getBoundingClientRect();
    const items = card.querySelector(".candidate-items");
    if (items && resolved.geometry.layout?.itemCornersFollowCard) {
      const bounds = items.getBoundingClientRect();
      const hiddenPreedit = card.querySelector(".pinyin")?.hidden;
      const positions = [...items.querySelectorAll(".cand")].map((node) => [node, node.getBoundingClientRect()]);
      // 按实际换行结果只放大四个外角；直角卡片同样覆盖普通候选圆角。
      for (const [node, rect] of positions) {
        for (const [corner, outer] of Object.entries({
          "top-left": hiddenPreedit && Math.abs(rect.top - bounds.top) < 1 && Math.abs(rect.left - bounds.left) < 1,
          "top-right": hiddenPreedit && Math.abs(rect.top - bounds.top) < 1 && Math.abs(rect.right - bounds.right) < 1,
          "bottom-left": Math.abs(rect.bottom - bounds.bottom) < 1 && Math.abs(rect.left - bounds.left) < 1,
          "bottom-right": Math.abs(rect.bottom - bounds.bottom) < 1 && Math.abs(rect.right - bounds.right) < 1,
        })) node.style.setProperty(`border-${corner}-radius`, outer ? "var(--msime-skin-radius)" : "var(--msime-skin-item-radius)");
      }
    }
    const left = finite(anchor?.left, MARGIN);
    const top = finite(anchor?.top, 0);
    const bottom = finite(anchor?.bottom, top);
    tallest = resolved.layout === "vertical" ? Math.max(tallest, height) : height;
    const x = Math.max(MARGIN, Math.min(left, vw - MARGIN - width));
    let y = bottom + GAP;
    if (y + tallest > vh - MARGIN) {
      const above = top - GAP - height;
      if (top - GAP - tallest >= MARGIN) y = above;
    }
    y = Math.max(MARGIN, Math.min(y, vh - MARGIN - height));
    root.style.setProperty("left", `${Math.round(x)}px`);
    root.style.setProperty("top", `${Math.round(y)}px`);
  };

  const refresh = () => {
    applySkin();
    if (frame) draw();
  };

  // 换皮肤、排布或明暗时先解析，解析失败（未知皮肤）就抛出，原来的状态不变。
  const update = (next) => {
    const nextSkin = "skin" in next ? next.skin : skinArg;
    const nextLayout = "layout" in next ? checkLayout(next.layout) : layoutArg;
    const nextDark = "dark" in next ? checkDark(next.dark) : darkArg;
    resolved = resolveSkin(nextSkin, { dark: isDark(nextDark), layout: nextLayout });
    skinArg = nextSkin;
    layoutArg = nextLayout;
    darkArg = nextDark;
    if (!destroyed) refresh();
  };

  const onScheme = () => {
    if (darkArg === "auto" && !destroyed) update({});
  };
  media?.addEventListener?.("change", onScheme);
  const onMotion = () => {
    if (destroyed) return;
    for (const picture of pictures) picture.display();
    if (frame && !root.hidden) layoutCanvas();
  };
  motionMedia?.addEventListener?.("change", onMotion);
  const onResize = () => { if (!destroyed && frame && !root.hidden) place(); };
  view.addEventListener?.("resize", onResize);

  applySkin();
  container.append(host);

  const hide = () => {
    if (hideTimer !== null) {
      view.clearTimeout?.(hideTimer);
      hideTimer = null;
    }
    tallest = 0;
    frame = null;
    anchor = null;
    shownHighlight = null;
    root.hidden = true;
    delete root.dataset.exiting;
    delete root.dataset.entering;
    delete root.dataset.hover;
  };

  return {
    element: host,
    /** 画一帧；frame.composing 为 false 时隐藏。anchorRect 是光标或文本框的矩形（视口坐标），候选栏画在它下方（下方放不下时画在上方）。 */
    render(nextFrame, anchorRect) {
      if (destroyed) return;
      if (hideTimer !== null) {
        view.clearTimeout?.(hideTimer);
        hideTimer = null;
      }
      if (!nextFrame?.composing) {
        if (resolved.animation.exit !== "none" && resolved.animation.durationMs > 0 && !motionMedia?.matches &&
            typeof view.setTimeout === "function") {
          root.dataset.exiting = resolved.animation.exit;
          hideTimer = view.setTimeout(() => hide(), resolved.animation.durationMs);
        } else hide();
        return;
      }
      delete root.dataset.exiting;
      frame = nextFrame;
      anchor = anchorRect;
      draw();
    },
    hide,
    setSkin(nextSkin) {
      update({ skin: nextSkin });
    },
    setLayout(nextLayout) {
      update({ layout: nextLayout });
    },
    setDark(nextDark) {
      update({ dark: nextDark });
    },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      if (hideTimer !== null) {
        view.clearTimeout?.(hideTimer);
        hideTimer = null;
      }
      frame = null;
      clearCanvas();
      view.removeEventListener?.("resize", onResize);
      motionMedia?.removeEventListener?.("change", onMotion);
      media?.removeEventListener?.("change", onScheme);
      root.removeEventListener("mousedown", onMouseDown);
      root.removeEventListener("pointermove", onPointerMove);
      root.removeEventListener("click", onClick);
      decoration.removeEventListener("error", onImageError);
      background.removeEventListener("error", onImageError);
      host.remove();
    },
  };
}
