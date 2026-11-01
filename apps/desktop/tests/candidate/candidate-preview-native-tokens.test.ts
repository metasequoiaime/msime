import { expect, test } from "vitest";
import { candidatePaletteStyle, themeEntry, type CandidateThemePalette } from "@msime/ui";
import { utilityCss } from "../support/utility-css";
import candidateSkin from "../../../../platforms/macos/src/candidate/CandidateSkin.cpp?raw";
import candidateSkinHeader from "../../../../platforms/macos/src/candidate/CandidateSkin.h?raw";
import candidateChrome from "../../../../platforms/macos/src/candidate/CandidateChrome.h?raw";
import candidateChromeMetrics from "../../../../shared/candidate/CandidateChromeMetrics.h?raw";

const card = utilityCss("skin-card-preview").replace(/\s+/g, "");

const empty: CandidateThemePalette = {
  surface: null,
  border: null,
  text: null,
  number: null,
  secondary: null,
  accent: null,
  selected: null,
  selected_text: null,
  selected_number: null,
  hover: null,
  show_selected_bar: null,
};

function style(palette: Partial<CandidateThemePalette>): Record<string, string> {
  return candidatePaletteStyle({ ...empty, ...palette }) as Record<string, string>;
}

test("a theme's selected foregrounds reach the preview", () => {
  const light = themeEntry("light").candidate!;
  const drawn = candidatePaletteStyle(light) as Record<string, string>;
  expect(drawn["--cand-selected-text"]).toBe(light.selected_text);
  expect(drawn["--cand-selected-num"]).toBe(light.selected_number);
  // 内置主题的竖条开关是 null：交给平台默认值，不写进内联样式。
  expect(drawn).not.toHaveProperty("--cand-selected-bar");
});

test("the selected-bar switch is written only when the palette sets it", () => {
  expect(style({ show_selected_bar: true })["--cand-selected-bar"]).toBe("block");
  expect(style({ show_selected_bar: false })["--cand-selected-bar"]).toBe("none");
  expect(style({ accent: "#2C7A4B" })).not.toHaveProperty("--cand-selected-bar");
  // `system` 没有调色板，什么都不写。
  expect(candidatePaletteStyle(null)).toEqual({});
});

test("the macOS selected foregrounds follow the fill that is drawn when the palette leaves them unset", () => {
  // 没给 selected 时选中行跟随强调色；浅色强调色上取深字，序号同色 82% 不透明。
  expect(style({ accent: "#FFEE88" })).toMatchObject({
    "--cand-mac-selected-text": "#1A1A1A",
    "--cand-mac-selected-num": "#1A1A1AD1",
  });
  expect(style({ accent: "#112233" })).toMatchObject({
    "--cand-mac-selected-text": "#FFFFFF",
    "--cand-mac-selected-num": "#FFFFFFD1",
  });
  // 半透明的选中底（不透明度低于 0.85）：文字和序号都回到普通行的颜色。
  expect(style({ selected: "#11223340", accent: "#FFEE88" })).toMatchObject({
    "--cand-mac-selected-text": "var(--cand-text)",
    "--cand-mac-selected-num": "var(--cand-num)",
  });
  // 调色板自己给了选中前景色时不再推导。
  const given = style({
    selected: "#112233",
    selected_text: "#00FF00",
    selected_number: "#00FF0080",
  });
  expect(given).not.toHaveProperty("--cand-mac-selected-text");
  expect(given).not.toHaveProperty("--cand-mac-selected-num");
  // 既没有 selected 也没有 accent：原生画的是平台的选中底，交给样式表里的默认值和白字。
  expect(style({ surface: "#FFFFFF" })).not.toHaveProperty("--cand-mac-selected-text");
  expect(style({ surface: "#FFFFFF" })).not.toHaveProperty("--cand-mac-selected");
});

test("the macOS selected fill follows a palette's accent only when the palette leaves selected unset", () => {
  // 原生 `ResolveSkin`：主题给了强调色、没给选中底时选中底跟着强调色；给了选中底就画选中底；都没给时是原生选中底（深色 #17783F，不是强调色）。
  expect(style({ accent: "#FFEE88" })["--cand-mac-selected"]).toBe("#FFEE88");
  expect(style({ selected: "#112233", accent: "#FFEE88" })).not.toHaveProperty(
    "--cand-mac-selected",
  );
  expect(style({ text: "#123456" })).not.toHaveProperty("--cand-mac-selected");
});

test("the selected row reads the selected foregrounds and the bar switch, falling back to the old look", () => {
  expect(card).toContain(".first.text{color:var(--cand-selected-text,var(--cand-text));}");
  expect(card).toContain(".first.num{color:var(--cand-selected-num,var(--cand-num));}");
  expect(card).toContain(".first.cand-no{color:var(--cand-selected-num,var(--cand-num));}");
  expect(card.match(/display:var\(--cand-selected-bar,block\)/g)).toHaveLength(2);
});

test("a hovered selected row keeps the selected fill its foregrounds are drawn for", () => {
  // `.cand:hover` 与 `.first:hover` 特异性相同，后写的胜出：横排、竖排里都必须是选中行的规则排在后面，否则悬停把选中底换成 `--cand-hover`，白字落在白卡片上。
  const order = [
    ...card.matchAll(
      /\.cand:hover\{background-color:var\(--cand-hover\);\}|\.first:hover\{background-color:var\(--cand-selected\);\}/g,
    ),
  ].map(([rule]) => (rule.startsWith(".first") ? "selected" : "hover"));
  expect(order).toEqual(["hover", "selected", "hover", "selected"]);
  // 原生 macOS 的选中悬停色就是选中色。
  expect(candidateSkin).toContain("tokens.selectedHover = tokens.selected;");
});

// ---- macOS 宿主默认值与原生 token 对齐 ----

/** 原生 `Rgb(0xRRGGBB, a)` / `Rgba{r, g, b, a}` 写成样式表里的小写 `#rrggbb` 或 `#rrggbbaa`。 */
function hex(expression: string): string {
  const channel = (value: number) =>
    Math.round(value * 255)
      .toString(16)
      .padStart(2, "0");
  const rgb = /^Rgb\(0x([0-9A-Fa-f]{6})(?:,\s*([\d.]+)f)?\)$/.exec(expression);
  if (rgb) {
    const alpha = rgb[2] === undefined ? 1 : Number(rgb[2]);
    return `#${rgb[1].toLowerCase()}${alpha === 1 ? "" : channel(alpha)}`;
  }
  const rgba = /^Rgba\{([\d.]+)f,\s*([\d.]+)f,\s*([\d.]+)f,\s*([\d.]+)f\}$/.exec(expression);
  if (!rgba) throw new Error(`unparsed native colour ${expression}`);
  const [r, g, b, a] = rgba.slice(1).map(Number);
  return `#${[r, g, b].map(channel).join("")}${a === 1 ? "" : channel(a)}`;
}

/** `NativeCandidateTokens` 里某个分支（`dark` 为真或否）给出的颜色 token。 */
function nativeColours(dark: boolean): Record<string, string> {
  const body = /SkinTokens NativeCandidateTokens\(bool dark\)\s*\{([\s\S]*?)\n\}/.exec(
    candidateSkin,
  )![1];
  const branch = /if \(dark\)\s*\{([\s\S]*?)\n {4}\}\s*else\s*\{([\s\S]*?)\n {4}\}/.exec(body)!;
  const shared = body.slice(branch.index + branch[0].length);
  const colours: Record<string, string> = {};
  for (const [, name, value] of `${branch[dark ? 1 : 2]}${shared}`.matchAll(
    /tokens\.(\w+) = ([^;]+);/g,
  ))
    colours[name] = value.startsWith("tokens.") ? value : hex(value);
  return colours;
}

/** 一条平铺规则里的自定义属性声明，值统一成小写、去掉空白。 */
function declarations(selector: string): Record<string, string> {
  const escaped = selector.replace(/\s+/g, "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const rule = new RegExp(`(?:^|[;{}])${escaped}\\{([^{}]*)\\}`).exec(
    card.replace(/\/\*[\s\S]*?\*\//g, ""),
  );
  if (!rule) throw new Error(`no rule for ${selector}`);
  return Object.fromEntries(
    [...rule[1].matchAll(/(--[a-z0-9-]+):([^;]+);/g)].map(([, name, value]) => [
      name,
      value.toLowerCase(),
    ]),
  );
}

const mac = ':is([data-platform="mac"]) .skin-card-preview';

test.each([
  ["light", false],
  ["dark", true],
] as const)("the macOS %s preview defaults are the native candidate tokens", (theme, dark) => {
  const native = nativeColours(dark);
  const selected = native.selected === "tokens.accent" ? native.accent : native.selected;
  expect(declarations(`${mac}[data-preview-theme="${theme}"]`)).toEqual({
    "--cand-bg": native.surface,
    "--cand-border": native.border,
    "--cand-text": native.text,
    "--cand-num": native.number,
    "--cand-accent": native.accent,
    "--cand-selected": `var(--cand-mac-selected,${selected})`,
    "--cand-hover": native.hover,
  });
});

/** WCAG 2 对比度：`foreground` 按自己的不透明度合成到不透明的 `fill` 上，两者都是 `#rrggbb` 或 `#rrggbbaa`。 */
function contrast(foreground: string, fill: string): number {
  const channels = (hex: string) =>
    [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255);
  const alpha = foreground.length === 9 ? parseInt(foreground.slice(7), 16) / 255 : 1;
  const under = channels(fill);
  const over = channels(foreground).map(
    (value, index) => value * alpha + under[index] * (1 - alpha),
  );
  const luminance = ([r, g, b]: number[]) =>
    [0.2126, 0.7152, 0.0722].reduce((sum, weight, index) => {
      const value = [r, g, b][index];
      return sum + weight * (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
    }, 0);
  const [a, b] = [luminance(over), luminance(under)];
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

test.each([false, true])(
  "the macOS default selected text reaches WCAG AA on the default fill (dark %s)",
  (dark) => {
    const native = nativeColours(dark);
    const selected = native.selected === "tokens.accent" ? native.accent : native.selected;
    expect(contrast(native.selectedText, selected)).toBeGreaterThanOrEqual(4.5);
    // 强调色 #5FBF84 上的白字只有约 2.3:1，深色的选中底因此不是强调色。
    if (dark) expect(contrast(native.selectedText, native.accent)).toBeLessThan(4.5);
  },
);

test("the macOS preview selects with a solid accent, white text and no bar, in the native geometry", () => {
  const native = nativeColours(false);
  // 浅色的选中底就是强调色；深色是更深的 #17783F。调色板给了强调色时两种都跟着它，与 `ResolveSkin` 相同（`--cand-mac-selected`）。
  expect(native.selected).toBe("tokens.accent");
  expect(nativeColours(true).selected).toBe("#17783f");
  const tokenField = (name: string) =>
    Number(new RegExp(`float ${name} = ([\\d.]+)f;`).exec(candidateSkinHeader)![1]);
  expect(tokenField("selectedRadius")).toBe(tokenField("candidateRadius"));
  expect(/bool showSelectedBar = (\w+);/.exec(candidateSkinHeader)![1]).toBe("false");
  // 行内边距的数值在 shared/candidate 里，macOS 的 MSIMECandidateRowPadding 只是引用它。
  expect(candidateChrome).toContain(
    "MSIMECandidateRowPadding = msime::candidate::kCandidateRowPadding;",
  );
  const rowPadding = Number(
    /constexpr double kCandidateRowPadding = ([\d.]+);/.exec(candidateChromeMetrics)![1],
  );
  expect(declarations(`${mac}[data-preview-theme]`)).toEqual({
    "--cand-selected-text": `var(--cand-mac-selected-text,${native.selectedText})`,
    "--cand-selected-num": `var(--cand-mac-selected-num,${native.selectedNumber})`,
    "--cand-selected-bar": "none",
    "--msime-host-card-radius": `${tokenField("radius")}px`,
    "--msime-host-row-radius": `${tokenField("candidateRadius")}px`,
    "--msime-host-border-width": `${tokenField("borderWidth")}px`,
    "--msime-host-pad": `${tokenField("pad")}px`,
    "--msime-host-row-pad-y": `${rowPadding / 2}px`,
  });
});

test("the card geometry reads the host values, keeping the old ones where no host sets them", () => {
  // 皮肤包或用户设的圆角（`--msime-skin-radius`）先于宿主默认值；用户设的内边距（`--msime-candidate-user-pad`）同样先于宿主的 `--msime-host-pad`。
  expect(
    card.match(/border-radius:var\(--msime-skin-radius,var\(--msime-host-card-radius,6px\)\)/g),
  ).toHaveLength(2);
  expect(
    card.match(/border:var\(--msime-host-border-width,1\.5px\)solidvar\(--cand-border\)/g),
  ).toHaveLength(2);
  expect(card).toContain(
    "--msime-candidate-pad-x:var(--msime-candidate-user-pad,var(--msime-host-pad,1px));--msime-candidate-pad-y:var(--msime-candidate-user-pad,var(--msime-host-pad,2px));",
  );
  expect(card).toContain(
    "--msime-candidate-pad-x:var(--msime-candidate-user-pad,var(--msime-host-pad,2px));--msime-candidate-pad-y:var(--msime-candidate-user-pad,var(--msime-host-pad,2px));",
  );
  // 行圆角：宿主的行圆角，用户设了窗口圆角时不超过它。
  expect(
    card.match(
      /--msime-candidate-row-radius:min\(var\(--msime-host-row-radius,4px\),var\(--msime-candidate-user-radius,var\(--msime-host-row-radius,4px\)\)\);/g,
    ),
  ).toHaveLength(2);
  expect(card.match(/border-radius:var\(--msime-candidate-row-radius\)/g)).toHaveLength(4);
  // 没有宿主时的默认配色保持原样。
  expect(declarations('.skin-card-preview[data-preview-theme="light"]')["--cand-selected"]).toBe(
    "#e8e8e8",
  );
  // 不透明度对所有卡片生效，主题页的卡片也跟着用户设置淡出。
  expect(card).toContain(
    "background:color-mix(insrgb,var(--cand-bg)var(--msime-candidate-opacity,100%),transparent);",
  );
});

test("macOS 分轴留白与独立行圆角优先，选中与悬停保持同形", () => {
  const geometry = declarations(`${mac}[data-preview-theme] :is(.wnd-h,.wnd-v)`);
  expect(geometry["--msime-candidate-pad-x"]).toBe(
    "max(0px,var(--msime-candidate-padding-horizontal,var(--msime-candidate-user-pad,var(--msime-host-pad))))",
  );
  expect(geometry["--msime-candidate-pad-y"]).toBe(
    "max(0px,var(--msime-candidate-padding-vertical,var(--msime-candidate-user-pad,var(--msime-host-pad))))",
  );
  expect(geometry["--msime-candidate-row-radius"]).toBe(
    "var(--msime-candidate-item-corner-radius,max(0px,calc(var(--msime-candidate-user-radius,calc(var(--msime-host-row-radius)+var(--msime-candidate-min-pad)))-var(--msime-candidate-min-pad)),calc(var(--msime-skin-radius,var(--msime-host-card-radius))-var(--msime-host-border-width)-max(var(--msime-candidate-min-pad),var(--msime-candidate-user-inset))*3.4142)))",
  );
  expect(geometry["--msime-candidate-min-pad"]).toBe(
    "min(var(--msime-candidate-pad-x),var(--msime-candidate-pad-y))",
  );
});

test("macOS 候选预览不在窗口留白之外叠加候选行上下外边距", () => {
  const selector = `${mac}[data-preview-theme] :is(.wnd-h,.wnd-v)`.replace(/\s+/g, "");
  const block = card.slice(card.indexOf(`${selector}{.container{`));
  expect(block).toMatch(/^.+?\{\.container\{[^}]*\}\.cand\{margin-top:0;margin-bottom:0;/);
});

test("macOS 横排预览不用内联基线撑高卡片，首行仍独占整行", () => {
  const selector = `${mac}[data-preview-theme] :is(.wnd-h,.wnd-v)`.replace(/\s+/g, "");
  const block = card.slice(card.indexOf(`${selector}{.container{`));
  expect(block).toContain(
    ".skin-card-preview.wnd-h.container{display:flex;flex-wrap:wrap;align-items:flex-start;}",
  );
  expect(block).toContain(
    ".skin-card-preview.wnd-h.pinyin,.skin-card-preview.wnd-h.candidate-brand-row{flex-basis:100%;}",
  );
});

test("macOS 系统主题预览补出原生材质的灰色边框，不覆盖自定义调色板", () => {
  expect(
    declarations(`${mac}[data-global-theme="system"][data-preview-theme]`)["--cand-border"],
  ).toBe("#8e8e9380");
  expect(
    card.match(/border:var\(--msime-host-border-width,1\.5px\)solidvar\(--cand-border\);/g),
  ).toHaveLength(2);
});

test("macOS 背景图容器在未覆盖候选项圆角时仍裁剪图片", () => {
  const selector = `${mac}[data-preview-theme] :is(.wnd-h,.wnd-v)`.replace(/\s+/g, "");
  const block = card.slice(card.indexOf(`${selector}{.container{`));
  expect(block).toContain(".container:has(>.skin-background-image){overflow:hidden!important;}");
});
