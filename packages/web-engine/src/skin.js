// 候选框皮肤的模型：把内置皮肤 ID 或皮肤对象解析成候选条要画的颜色、几何和 CSS 自定义属性。这是 `crates/client-core/src/skin/theme.rs` 的 `resolve()` 在网页上的移植，逐条照着设置页的镜像 `packages/ui/src/theme/global-theme.ts`（`normalizedColor`、`customCandidatePalette`、`candidatePaletteStyle`）和 `packages/ui/src/skin/external-skins.tsx`（`dimension`、`decorationImage`、`skinGeometryStyle`、`drawnPackagePalette`）写成；`apps/desktop/tests/candidate/custom-theme-parity.json` 由 Rust 测试写出，test/skin.test.mjs 用它核对这里的结果与 `resolve()` 一致。
//
// 内置配色表 `./theme-catalog.js` 不是手写的副本：它由 `packages/web-engine/tools/theme-catalog.mjs` 从 `packages/ui/src/theme/theme-catalog.json`（Rust 主题表的网页副本）和 `crates/client-core/src/skin/catalog/windows_looks.rs`（msime-windows 内置外观，没有网页副本）生成。scripts/build-web-engine.sh 构建 npm 包时生成一份放在本文件旁边；在仓库里直接对 src/ 运行时，先跑 `node packages/web-engine/tools/theme-catalog.mjs` 生成 `src/theme-catalog.js`（被 git 忽略），test/skin.test.mjs 每次运行前会自动重新生成。
import { plainWindowsLooks, themes, windowsLooks } from "./theme-catalog.js";

const LAYOUTS = ["horizontal", "vertical"];
const MODES = ["dark", "light"];
// `theme::SELECTED_ALPHA`、`theme::HOVER_ALPHA`、`theme::PICKED_NUMBER_ALPHA`。
const SELECTED_ALPHA = "24";
const HOVER_ALPHA = "0F";
const PICKED_NUMBER_ALPHA = "9D";
const BACKGROUND_FITS = ["cover", "contain", "stretch"];
const DECORATION_ALIGNS = ["left", "center", "right"];

/** 不传皮肤时画的皮肤。 */
export const DEFAULT_SKIN = "shuishan";

// 桌面上的 `system` 画宿主平台自己的配色，主题表里它没有配色（`candidate: null`），`resolve()` 在它之上留空的槽位也都交给平台。网页上的平台配色就是桌面设置页预览用的那套默认值：`packages/ui/src/styles.css` 的 `skin-card-preview` 按 `data-preview-theme` 声明的 `--cand-*`（选中项的文字和序号没有专门的颜色，就是普通的文字和序号色，强调色是样式表的 `#6B69D6`）。候选条的样式表（candidates.js 的 SHEET）也从这张表生成默认值。
export const PLATFORM_PALETTE = Object.freeze({
  light: Object.freeze({ surface: "#FFFFFF", border: "#0000001F", text: "#1A1A1A", number: "#1A1A1A8C", accent: "#6B69D6", selected: "#E8E8E8", hover: "#ECECEC" }),
  dark: Object.freeze({ surface: "#202020", border: "#9B9B9B2E", text: "#E9E8E8", number: "#E9E8E89D", accent: "#6B69D6", selected: "#3E3E3EB9", hover: "#414141" }),
});

const systemEntry = themes.find((entry) => entry.id === "system");
const builtinThemes = themes.filter((entry) => entry.candidate !== null);
if (!systemEntry) throw new Error("theme catalog: no system entry");

/**
 * SDK 接受的内置皮肤 ID，顺序同主题表：`system`（含义见上）、五个全局主题，再加 msime-windows 的五个内置外观。外观在桌面上只能作为皮肤包的 `base`，这里把它们当作一个两种布局、深浅两种模式都支持、没写任何颜色的皮肤包，按 `windows_looks::fill_from_look` 补齐配色和圆角后画在 `system` 之上，与 Windows 上同名外观一致。`fluent` 就是 Windows 原生配色、等同 `system`，不单列；`custom` 是桌面设置里的自定义主题，不是皮肤。
 */
export const SKINS = Object.freeze([
  ...themes.filter((entry) => entry.id === "system" || entry.candidate !== null).map((entry) => entry.id),
  ...windowsLooks.map((look) => look.id),
]);

/** `theme::normalized_color`：颜色写成大写的 `#RRGGBB` 或 `#RRGGBBAA`，接受 `#RGB`、通道 0-255 且透明度 0-1 的 `rgb()`/`rgba()` 和 `transparent`；其他一律 `null`。 */
export function normalizedColor(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (trimmed.toLowerCase() === "transparent") return "#00000000";
  if (trimmed.startsWith("#")) {
    const hex = trimmed.slice(1);
    if (!/^[0-9a-f]*$/i.test(hex)) return null;
    if (hex.length === 3) return `#${[...hex].map((digit) => digit + digit).join("")}`.toUpperCase();
    return hex.length === 6 || hex.length === 8 ? `#${hex.toUpperCase()}` : null;
  }
  const lower = trimmed.toLowerCase();
  const alpha = lower.startsWith("rgba(");
  if (!alpha && !lower.startsWith("rgb(")) return null;
  if (!lower.endsWith(")")) return null;
  const parts = lower
    .slice(alpha ? 5 : 4, -1)
    .split(",")
    .map((part) => part.trim());
  if (parts.length !== (alpha ? 4 : 3)) return null;
  const channels = parts.slice(0, 3).map((part) => (/^\+?\d+$/.test(part) ? Number(part) : NaN));
  if (channels.some((channel) => !(channel <= 255))) return null;
  const hex = (channel) => channel.toString(16).toUpperCase().padStart(2, "0");
  const rgb = `#${channels.map(hex).join("")}`;
  if (!alpha) return rgb;
  // 写成 `\d+(\.\d*)?` 而不是 `\d+\.?\d*`：两者接受的串相同，后者在一长串数字后跟非法字符时要回溯平方次，恶意皮肤的颜色能卡住页面。
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$/.test(parts[3])) return null;
  const opacity = Number(parts[3]);
  if (!(opacity >= 0 && opacity <= 1)) return null;
  return `${rgb}${hex(Math.round(opacity * 255))}`;
}

/** 自定义主题取色器的颜色：只认 `#RRGGBB`，同 `packages/ui/src/candidate/candidate-text-color.ts` 的 `candidateTextColor`。 */
function pickerColor(value) {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value) ? value.toUpperCase() : null;
}

/** `theme::with_alpha`：换掉 `#RRGGBB` 或 `#RRGGBBAA` 的透明度。 */
function withAlpha(color, alpha) {
  return `${color.slice(0, 7)}${alpha}`;
}

/** `ai::readable_text`：`background` 上看得清的黑或白，按相对亮度在同一处分界，透明度不计。 */
function readableText(background) {
  const channel = (offset) => {
    const value = parseInt(background.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  return luminance > 0.179 ? "#000000" : "#FFFFFF";
}

/** 主题表里的一项；未知 ID 当作 `system`，同 `themeEntry`。 */
function themeEntry(id) {
  return themes.find((entry) => entry.id === id) ?? systemEntry;
}

/**
 * `customCandidatePalette` 的移植，即 `resolve()` 给自定义主题的候选框配色：`base` 的配色，其上是 `packagePalette`（皮肤包在所画明暗下的配色，只在该布局和模式下画这个包时传入），再上是取色器 `colors`。槽位用 Rust 的 snake_case 名，没设的槽位为 `null`，什么都没设时整个为 `null`，与 Rust 的配色逐项相同。导出给 test/skin.test.mjs 跑 `custom-theme-parity.json`；`resolveSkin` 用的是同一个函数（不传取色器）。
 */
export function customCandidatePalette(base, colors, packagePalette) {
  const basePalette = themeEntry(base).candidate;
  const derived = basePalette !== null;
  const fromPackage = (value) => (typeof value === "string" ? normalizedColor(value) : null);
  const picked = (value) => pickerColor(value);
  const slot = (key) => picked(colors?.[key]) ?? fromPackage(packagePalette?.[key]);
  const pickedText = picked(colors?.text);
  const explicitNumber = pickedText && !picked(colors?.number) ? withAlpha(pickedText, PICKED_NUMBER_ALPHA) : slot("number");
  const text = slot("text") ?? basePalette?.text ?? null;
  const number = explicitNumber ?? basePalette?.number ?? null;
  const accent = slot("accent") ?? basePalette?.accent ?? null;
  const pickedSelected = picked(colors?.selected);
  const pickedSelectedText = pickedSelected ? readableText(pickedSelected) : null;
  const palette = {
    surface: slot("surface") ?? basePalette?.surface ?? null,
    border: slot("border") ?? basePalette?.border ?? null,
    text,
    number,
    secondary: fromPackage(packagePalette?.translation) ?? number,
    accent,
    selected: slot("selected") ?? (derived && accent ? withAlpha(accent, SELECTED_ALPHA) : null),
    selected_text: pickedSelectedText ?? (derived ? accent : null),
    selected_number: pickedSelectedText ? withAlpha(pickedSelectedText, PICKED_NUMBER_ALPHA) : derived ? number : null,
    hover: slot("hover") ?? (derived && text ? withAlpha(text, HOVER_ALPHA) : null),
    show_selected_bar: typeof packagePalette?.showSelectedBar === "boolean" ? packagePalette.showSelectedBar : null,
  };
  return Object.values(palette).some((value) => value !== null) ? palette : null;
}

/** `drawnPackagePalette`：皮肤包在 `mode` 下的配色，没声明这个模式时为 `null`，此时只画 base。 */
export function drawnPackagePalette(skin, mode) {
  return Array.isArray(skin?.themes) && skin.themes.includes(mode) ? (skin.candidate?.[mode] ?? null) : null;
}

/** `dimension`：0 到 `maximum` 之间的有限数原样保留，其他一律为 0。 */
function dimension(value, maximum) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= maximum ? value : 0;
}

/**
 * 皮肤图片的地址：只放行 `http:`、`https:`、`blob:`、`data:image/*` 和相对地址（由候选条按页面地址解析），其他协议（`javascript:` 等）一律丢弃。地址最终会写进 `url("…")`，所以含引号、反斜杠、括号、尖括号、花括号、反引号、空白或控制字符的也整个丢弃，而不是转义，保证它跳不出 CSS 声明。唯一的例外是 `data:image/*`：`encodeURIComponent` 不编码括号和单引号，按它写出的 SVG（`url(#g)`、`rotate(45)`）很常见，而 data URI 的内容本来就会先做百分号解码，所以这三个字符换成 `%28`、`%29`、`%27`，图片不变。
 */
export function skinImageUrl(value) {
  if (typeof value !== "string" || value.length === 0) return null;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(value);
  const kind = scheme ? scheme[1].toLowerCase() : null;
  const url = kind === "data" ? value.replace(/[()']/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`) : value;
  if (/[\u0000- \u007f-\u009f"'()\\<>`{}]/.test(url)) return null;
  switch (kind) {
    case null:
    case "http":
    case "https":
    case "blob":
      return url;
    case "data":
      return /^data:image\/[a-z0-9.+-]+[;,]/i.test(url) ? url : null;
    default:
      return null;
  }
}

/** 皮肤对象声明的布局或明暗：没写（`undefined`/`null`）时全都支持，写了数组只认其中合法的值，写了别的东西则什么都不支持。 */
function declared(value, allowed) {
  if (value === undefined || value === null) return allowed;
  return Array.isArray(value) ? allowed.filter((entry) => value.includes(entry)) : [];
}

const PACKAGE_SLOTS = ["surface", "border", "text", "number", "accent", "selected", "hover", "translation"];
const LOOK_SLOTS = ["surface", "border", "text", "number", "accent", "selected", "hover"];

/** 皮肤对象某个模式的配色：只取字符串颜色和布尔的 `showSelectedBar`，颜色留到 `customCandidatePalette` 里规范化。base 是 Windows 外观时，读不懂或没写的颜色按 `windows_looks::fill` 用外观的颜色补上，`showSelectedBar` 没写时取外观的。 */
function packagePalette(raw, look) {
  const source = raw !== null && typeof raw === "object" ? raw : {};
  const palette = {};
  for (const key of PACKAGE_SLOTS) {
    if (typeof source[key] === "string") palette[key] = source[key];
  }
  if (typeof source.showSelectedBar === "boolean") palette.showSelectedBar = source.showSelectedBar;
  if (look) {
    for (const key of LOOK_SLOTS) {
      if (normalizedColor(palette[key]) === null) palette[key] = look[key];
    }
    if (typeof palette.showSelectedBar !== "boolean") palette.showSelectedBar = look.showSelectedBar;
  }
  return palette;
}

/** 外观给高亮候选的文字色（`LookCandidate::selected_text`，文字和序号同色）：只在外观有这个颜色、且皮肤包没写自己能读懂的选中底色时才用，包换了选中底色，外观配好的字色就不一定看得清了。 */
function lookSelectedText(raw, look) {
  if (!look?.selectedText) return null;
  const source = raw !== null && typeof raw === "object" ? raw : {};
  return normalizedColor(source.selected) === null ? look.selectedText : null;
}

/** 皮肤对象（`SkinSummary` 的 camelCase JSON，或带 `supports` 的清单形状）读成解析需要的样子；`base` 是 Windows 外观时按 `catalog::load` 的做法补齐配色和圆角并换成 `system`，另记下外观给高亮候选的文字色（`resolve()` 没有这个槽位，由 `completePalette` 补）。 */
function readPackage(skin) {
  const rawBase = skin.base === undefined || skin.base === null ? "system" : skin.base;
  if (typeof rawBase !== "string") throw new TypeError("skin base must be a string");
  const look = windowsLooks.find((entry) => entry.id === rawBase) ?? null;
  const base = look || plainWindowsLooks.includes(rawBase) ? "system" : rawBase;
  if (base !== "system" && !builtinThemes.some((entry) => entry.id === base)) throw new TypeError(`unknown skin base: ${rawBase}`);
  const candidate = skin.candidate !== null && typeof skin.candidate === "object" ? skin.candidate : {};
  const supports = skin.supports !== null && typeof skin.supports === "object" ? skin.supports : {};
  let cornerRadius = null;
  if (typeof skin.cornerRadiusDip === "number") cornerRadius = dimension(skin.cornerRadiusDip, 32);
  else if (look) cornerRadius = look.cornerRadiusDip;
  return {
    id: typeof skin.id === "string" ? skin.id : null,
    base,
    layouts: declared(skin.layouts ?? supports.layouts, LAYOUTS),
    themes: declared(skin.themes ?? supports.themes, MODES),
    candidate: { dark: packagePalette(candidate.dark, look?.dark), light: packagePalette(candidate.light, look?.light) },
    selectedText: { dark: lookSelectedText(candidate.dark, look?.dark), light: lookSelectedText(candidate.light, look?.light) },
    cornerRadius,
    skin,
  };
}

/** 画出来的皮肤包的几何：最小宽度、圆角、装饰带和背景图，取值规则同 `skinGeometryStyle` 和 `usePreviewBackground`。装饰带没有可用的图片时不画（网页上空着的装饰带只会在候选条上方留一块透明区域）。 */
function packageGeometry(pkg) {
  const { skin } = pkg;
  const top = dimension(skin.decorationTopDip, 500);
  const width = dimension(skin.decorationWidthDip, 1000);
  const decorated = top > 0 && width > 0;
  const decorationUrl = decorated ? skinImageUrl(skin.decorationImage === undefined ? skin.preview : skin.decorationImage) : null;
  const rawBackground = skin.background !== null && typeof skin.background === "object" ? skin.background : null;
  const backgroundUrl = rawBackground ? skinImageUrl(rawBackground.image) : null;
  const opacity = rawBackground?.opacity;
  return {
    cornerRadius: pkg.cornerRadius,
    minWidth: skin.minWidthDip === undefined || skin.minWidthDip === null ? null : dimension(skin.minWidthDip, 1000),
    decoration: decorationUrl
      ? { url: decorationUrl, top, width, align: DECORATION_ALIGNS.includes(skin.decorationAlign) ? skin.decorationAlign : "right" }
      : null,
    background: backgroundUrl
      ? {
          url: backgroundUrl,
          fit: BACKGROUND_FITS.includes(rawBackground.fit) ? rawBackground.fit : "cover",
          // 清单不写透明度时是 1（`read_background`）；写了则同 `usePreviewBackground` 夹到 0-1，读不成数的当 0。
          opacity: opacity === undefined ? 1 : Math.min(1, Math.max(0, typeof opacity === "number" || typeof opacity === "string" ? Number(opacity) || 0 : 0)),
        }
      : null,
  };
}

const NO_GEOMETRY = { cornerRadius: null, minWidth: null, decoration: null, background: null };

/**
 * `resolve()` 的结果补成每个槽位都有颜色。`resolve()` 留给平台的槽位（base 为 `system` 时）用 `PLATFORM_PALETTE` 的同明暗配色补。选中项的文字和序号没有值时，`lookSelectedText` 是画出来的 Windows 外观只用文字色标出高亮候选时的那个颜色（见 `readPackage`），文字和序号都画成它；否则同桌面候选框的样式表画成普通的文字和序号色。次要文字没有值时用序号色。
 */
function completePalette(raw, mode, lookSelectedText) {
  const platform = PLATFORM_PALETTE[mode];
  const pick = (key) => raw?.[key] ?? platform[key];
  const text = pick("text");
  const number = pick("number");
  return {
    surface: pick("surface"),
    border: pick("border"),
    text,
    number,
    secondary: raw?.secondary ?? number,
    accent: pick("accent"),
    selected: pick("selected"),
    selectedText: raw?.selected_text ?? lookSelectedText ?? text,
    selectedNumber: raw?.selected_number ?? lookSelectedText ?? number,
    hover: pick("hover"),
    showSelectedBar: raw?.show_selected_bar ?? true,
  };
}

/**
 * 候选条用的 CSS 自定义属性。与桌面同名的沿用 `candidatePaletteStyle`（`--cand-*`、`--accent-strong`）和 `skinGeometryStyle`（`--msime-skin-*`）的名字；桌面样式表没有的几项是 SDK 新增的：`--cand-secondary`、`--cand-selected-text`、`--cand-selected-num`、`--msime-skin-selected-bar`（`block` 或 `none`，对应 `selectedBarCss` 隐藏选中条）、`--msime-skin-decoration-image`、`--msime-skin-background-image`、`--msime-skin-background-opacity` 和 `--msime-skin-background-fit`（`object-fit` 的值，`stretch` 写成 `fill`）。每个值都是规范化的颜色、有限的数字、白名单里的关键字或经 `skinImageUrl` 检查过的 `url("…")`，跳不出声明。阴影不随皮肤变，由候选条按明暗自己画。
 */
function cssVariables(palette, geometry) {
  const variables = {
    "--cand-bg": palette.surface,
    "--cand-border": palette.border,
    "--cand-text": palette.text,
    "--cand-num": palette.number,
    "--cand-accent": palette.accent,
    "--accent-strong": palette.accent,
    "--cand-selected": palette.selected,
    "--cand-hover": palette.hover,
    "--cand-secondary": palette.secondary,
    "--cand-selected-text": palette.selectedText,
    "--cand-selected-num": palette.selectedNumber,
    "--msime-skin-selected-bar": palette.showSelectedBar ? "block" : "none",
    "--msime-skin-decoration-top": `${geometry.decoration ? geometry.decoration.top : 0}px`,
    "--msime-skin-decoration-width": `${geometry.decoration ? geometry.decoration.width : 0}px`,
  };
  if (geometry.minWidth !== null) variables["--msime-skin-min-width"] = `${geometry.minWidth}px`;
  if (geometry.cornerRadius !== null) variables["--msime-skin-radius"] = `${geometry.cornerRadius}px`;
  if (geometry.decoration) variables["--msime-skin-decoration-image"] = `url("${geometry.decoration.url}")`;
  if (geometry.background) {
    variables["--msime-skin-background-image"] = `url("${geometry.background.url}")`;
    variables["--msime-skin-background-opacity"] = String(geometry.background.opacity);
    variables["--msime-skin-background-fit"] = geometry.background.fit === "stretch" ? "fill" : geometry.background.fit;
  }
  return variables;
}

function deepFreeze(value) {
  if (value !== null && typeof value === "object") {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}

/**
 * 把皮肤解析成候选条要画的样子。
 *
 * `skin`：`SKINS` 里的内置 ID；或皮肤对象，形状同桌面的 `SkinSummary` JSON（`base`、`layouts`/`themes` 或 `supports: { layouts, themes }`、`minWidthDip`、`cornerRadiusDip`、`decorationTopDip`、`decorationWidthDip`、`decorationImage`、`decorationAlign`、`background: { image, fit, opacity }`、`candidate: { dark, light }`），图片写成网址或 `data:` URI；不传时为 `DEFAULT_SKIN`。`base` 可以是全局主题、`system` 或 msime-windows 的内置外观，不写时为 `system`。
 *
 * 规则同 `resolve()`：内置全局主题和 base 为内置全局主题的皮肤固定画在那个主题的明暗下，`dark` 选项不起作用；皮肤对象只在它声明的布局和明暗下画，否则只画 base，结果的 `drawn` 为 `false`，也不带皮肤的几何。读不懂的颜色当没写，越界的尺寸按 `dimension` 归零，不合规的图片地址丢弃。
 *
 * 未知的内置 ID、未知的 `base` 和未知的 `layout` 抛 `TypeError`。返回值整个被冻结。
 */
export function resolveSkin(skin, { dark = false, layout = "horizontal" } = {}) {
  if (!LAYOUTS.includes(layout)) throw new TypeError(`unknown layout: ${layout}`);
  const requested = dark ? "dark" : "light";
  const target = skin === undefined ? DEFAULT_SKIN : skin;
  let id;
  let mode;
  let raw;
  let drawn = true;
  let geometry = NO_GEOMETRY;
  let selectedText = null;
  if (typeof target === "string") {
    if (!SKINS.includes(target)) throw new TypeError(`unknown skin: ${target}`);
    id = target;
    const theme = builtinThemes.find((entry) => entry.id === target);
    if (theme) {
      mode = theme.appearance;
      raw = theme.candidate;
    } else if (target === "system") {
      mode = requested;
      raw = null;
    } else {
      // Windows 外观：一个两种布局、两种明暗都支持、只写了 base 的皮肤包。
      const pkg = readPackage({ base: target });
      mode = requested;
      raw = customCandidatePalette(pkg.base, undefined, pkg.candidate[mode]);
      geometry = packageGeometry(pkg);
      selectedText = pkg.selectedText[mode];
    }
  } else if (target !== null && typeof target === "object" && !Array.isArray(target)) {
    const pkg = readPackage(target);
    id = pkg.id;
    mode = themeEntry(pkg.base).appearance ?? requested;
    drawn = pkg.layouts.includes(layout) && pkg.themes.includes(mode);
    raw = customCandidatePalette(pkg.base, undefined, drawn ? pkg.candidate[mode] : null);
    if (drawn) {
      geometry = packageGeometry(pkg);
      selectedText = pkg.selectedText[mode];
    }
  } else {
    throw new TypeError("skin must be a built-in skin id or a skin object");
  }
  const palette = completePalette(raw, mode, selectedText);
  return deepFreeze({
    id,
    dark: mode === "dark",
    layout,
    palette,
    geometry: { ...geometry },
    variables: cssVariables(palette, geometry),
    drawn,
  });
}
