// 候选栏：按水杉桌面端候选框的结构和样式画预编辑行、候选和翻页标记，配色和几何来自 skin.js 的 resolveSkin。
//
// 隔离：整个候选栏画在宿主元素 <msime-candidates> 的 Shadow DOM 里，页面的 CSS 改不到它，它的样式也漏不出去。样式表是可构造样式表（adoptedStyleSheets），每个皮肤的取值只经 `style.setProperty` 写到 Shadow DOM 内的根元素上，不用 <style> 元素，也不写 style 属性的文本，所以页面的 CSP 不开 `style-src 'unsafe-inline'` 也能用。皮肤里的图片用 <img> 加载，地址不进 CSS。
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
  /** 候选后的编码提示（helpcode 选项打开时）。 */
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
  --msime-skin-decoration-top: 0px;
  --msime-skin-decoration-width: 0px;
  --msime-skin-selected-bar: block;
  --msime-skin-background-fit: cover;
  --msime-skin-background-opacity: 1;
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
  padding: var(--msime-candidate-pad-y) var(--msime-candidate-pad-x);
  background: var(--cand-bg);
  border: 1.5px solid var(--cand-border);
  border-radius: var(--msime-skin-radius);
  box-shadow: var(--cand-shadow);
  min-width: max(7em, var(--msime-skin-min-width));
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

/* ---- 候选 ---- */
.row-wrapper {
  position: relative;
}
.wnd-h .row-wrapper {
  display: inline-block;
  vertical-align: top;
  max-width: 100%;
  min-width: 0;
}
.cand {
  position: relative;
  border-radius: 4px;
  cursor: pointer;
}
.wnd-h .cand {
  padding: 2px 4px 2px 0;
}
.wnd-v .cand {
  padding-top: 1px;
  padding-bottom: 1px;
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
  font-size: 0.8em;
  color: var(--cand-num);
}
.wnd-v .num {
  margin-right: 1.5px;
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
  left: 0;
  top: 53%;
  transform: translateY(-50%);
  height: 0.8em;
  width: 0.2em;
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

/**
 * 创建一个候选栏。skin 是内置皮肤 id、皮肤对象或 undefined（默认皮肤），见 skin.js 的 resolveSkin；layout 为 "horizontal" 或 "vertical"；dark 为 "auto"（跟随 prefers-color-scheme）、true 或 false；点击第 i 个候选时调用 onPick(i)；helpcode 为 true 时在候选后显示编码。皮肤 id 未知时抛 TypeError。
 */
export function createCandidateBar({ skin, layout = "horizontal", dark = "auto", onPick, container = document.body, helpcode = false } = {}) {
  const doc = container.ownerDocument ?? document;
  const view = doc.defaultView ?? globalThis;
  const sheet = sheetFor(doc);
  const media = view.matchMedia?.("(prefers-color-scheme: dark)") ?? null;

  let skinArg = skin;
  let layoutArg = checkLayout(layout);
  let darkArg = checkDark(dark);
  let frame = null;
  let anchor = null;
  let destroyed = false;
  let applied = [];
  // 这次组字里画过的最高高度，隐藏时清零。
  let tallest = 0;

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
  parent.append(decoration, card);
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

  // 把 resolveSkin 的结果写到根元素上：先清掉上一个皮肤写的属性，再逐个 setProperty 写 variables。图片不走 CSS 的 url()，用 geometry 里的地址交给 <img>，加载失败时能单独藏起来。
  const applySkin = () => {
    for (const name of applied) root.style.removeProperty(name);
    applied = [];
    root.className = `candidate wnd-${resolved.layout === "vertical" ? "v" : "h"}`;
    root.dataset.theme = resolved.dark ? "dark" : "light";
    for (const [name, value] of Object.entries(resolved.variables)) {
      if (!PROPERTY.test(name) || (typeof value !== "string" && typeof value !== "number")) continue;
      root.style.setProperty(name, String(value));
      applied.push(name);
    }

    const { decoration: deco, background: bg } = resolved.geometry;
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
    const rows = [];
    if (card.dataset.background !== undefined) rows.push(background);

    const pinyin = element(doc, "div", "row pinyin", PARTS.preedit);
    const reading = element(doc, "div", "text");
    reading.append(String(frame.preedit ?? ""), element(doc, "span", "cursor", PARTS.caret));
    pinyin.append(reading);
    if (frame.hasPrev || frame.hasNext) {
      const paging = element(doc, "div", "paging", PARTS.paging);
      paging.setAttribute("aria-hidden", "true");
      const prev = element(doc, "span", "page-prev");
      prev.textContent = "‹";
      if (!frame.hasPrev) prev.dataset.off = "";
      const next = element(doc, "span", "page-next");
      next.textContent = "›";
      if (!frame.hasNext) next.dataset.off = "";
      paging.append(prev, next);
      pinyin.append(paging);
    }
    rows.push(pinyin);

    const vertical = resolved.layout === "vertical";
    const page = Array.isArray(frame.page) ? frame.page : [];
    page.forEach((item, index) => {
      const highlighted = index === frame.highlight;
      const wrapper = element(doc, "div", index === 0 && !vertical ? "row-wrapper first-row-wrapper" : "row-wrapper");
      const cand = element(doc, "div", highlighted ? "row cand first" : "row cand", highlighted ? `${PARTS.candidate} ${PARTS.highlight}` : PARTS.candidate);
      cand.dataset.index = String(index);
      cand.setAttribute("role", "option");
      cand.setAttribute("aria-selected", String(highlighted));
      const text = element(doc, "div", "text");
      const num = element(doc, "span", vertical ? "num cand-no" : "num", PARTS.number);
      num.textContent = String(index + 1);
      const content = element(doc, "span", "cand-content", PARTS.text);
      content.textContent = String(item?.text ?? "");
      text.append(num, content);
      if (helpcode && item?.code) {
        const code = element(doc, "span", "cand-helpcode", PARTS.code);
        code.textContent = `(${item.code})`;
        content.append(code);
      }
      cand.append(text);
      wrapper.append(cand);
      rows.push(wrapper);
    });
    card.replaceChildren(...rows);
    root.hidden = false;
    place();
  };

  // 与 macOS 候选窗（CandidatePanel.mm）一致：整个候选栏（含装饰带）放在锚点正下方，下面放不下时翻到锚点上方，再夹在视口里。纵排时用这次组字里最高的一页决定上下，翻页时不会因为某页变高而跳到另一侧。
  const place = () => {
    const html = doc.documentElement;
    const vw = html.clientWidth || view.innerWidth;
    const vh = html.clientHeight || view.innerHeight;
    root.style.setProperty("--msime-max-width", `${Math.max(0, vw - 2 * MARGIN)}px`);
    root.style.setProperty("left", "0px");
    root.style.setProperty("top", "0px");
    const { width, height } = root.getBoundingClientRect();
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

  applySkin();
  container.append(host);

  const hide = () => {
    tallest = 0;
    frame = null;
    anchor = null;
    root.hidden = true;
    delete root.dataset.hover;
  };

  return {
    element: host,
    /** 画一帧；frame.composing 为 false 时隐藏。anchorRect 是光标或文本框的矩形（视口坐标），候选栏画在它下方（下方放不下时画在上方）。 */
    render(nextFrame, anchorRect) {
      if (destroyed) return;
      if (!nextFrame?.composing) {
        hide();
        return;
      }
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
      frame = null;
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
