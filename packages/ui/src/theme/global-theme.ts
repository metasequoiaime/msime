/**
 * The global theme contract, as the settings page sees it. The authoritative table is `crates/client-core/src/skin/theme.rs`; `theme-catalog.json` is its serialized `catalog()`, checked against it by the Rust test `web_catalog_copy_matches_the_catalog`, so the page can draw the picker and its previews synchronously on every host, including hosts whose bridge has no theme call.
 *
 * Every colour is `#RRGGBB` or `#RRGGBBAA` (uppercase, alpha last). A `null` slot means "the host's own platform token", never "transparent".
 */
import type { CSSProperties } from "react";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { candidateTextColor } from "../candidate/candidate-text-color";
import catalog from "./theme-catalog.json";

/** `Preferences.global_theme`: one id for the candidate window, floating toolbar, menus and touch keyboard. `siji`（水杉四季）没有自己的色板，按月份画春芽、夏荫、秋杉或冬雪。 */
export type GlobalTheme =
  | "system"
  | "siji"
  | "shuishan"
  | "light"
  | "paper"
  | "night"
  | "ink"
  | "chunya"
  | "xiayin"
  | "qiushan"
  | "dongxue"
  | "custom";
export type BuiltinGlobalTheme = Exclude<GlobalTheme, "system" | "siji" | "custom">;
export type ThemeAppearance = "light" | "dark";
/** `skin::season::Season`。 */
export type Season = "spring" | "summer" | "autumn" | "winter";

/** `season::season_for_month`：3-5 月春，6-8 月夏，9-11 月秋，12-2 月冬；不是 1-12 的月份为 `null`。 */
export function seasonForMonth(month: number): Season | null {
  if (!Number.isInteger(month) || month < 1 || month > 12) return null;
  if (month >= 3 && month <= 5) return "spring";
  if (month >= 6 && month <= 8) return "summer";
  if (month >= 9 && month <= 11) return "autumn";
  return "winter";
}

/** `GlobalTheme::seasonal_theme`：`siji` 在 `month` 所在季节代表的主题，其他主题原样返回。`month` 省略时取本地日历的当前月份。 */
export function seasonalThemeId(id: GlobalTheme, month?: number): GlobalTheme {
  if (id !== "siji") return id;
  const season = seasonForMonth(month ?? new Date().getMonth() + 1) ?? "autumn";
  const themes: Record<Season, GlobalTheme> = {
    spring: "chunya",
    summer: "xiayin",
    autumn: "qiushan",
    winter: "dongxue",
  };
  return themes[season];
}

export type CandidateThemePalette = {
  surface: string | null;
  border: string | null;
  text: string | null;
  number: string | null;
  /** Comments, translations and pinyin hints. */
  secondary: string | null;
  accent: string | null;
  selected: string | null;
  selected_text: string | null;
  selected_number: string | null;
  hover: string | null;
  show_selected_bar: boolean | null;
};

export type KeyboardThemePalette = {
  background: string;
  key: string;
  function_key: string;
  text: string;
  secondary: string;
  /** The return key while composing, and the selected candidate in a touch strip. */
  accent: string;
  on_accent: string;
};

export type ThemePreview = { background: string; panel: string; accent: string; text: string };

/** One picker entry. `system`, `siji` and `custom` carry no palette; `siji` carries a fixed 秋杉 preview. */
export type ThemeCatalogEntry = {
  id: GlobalTheme;
  title: string;
  appearance: ThemeAppearance | null;
  preview: ThemePreview | null;
  /** 只有 `siji` 为真：按季节变化。 */
  seasonal: boolean;
  candidate: CandidateThemePalette | null;
  keyboard: KeyboardThemePalette | null;
};

/** `msime_client_theme_catalog()`. */
export type ThemeCatalog = { themes: ThemeCatalogEntry[]; default: GlobalTheme };

/** The seven candidate colour pickers of the custom theme, each `#RRGGBB` or unset. */
export type CustomCandidateColors = {
  text?: string | null;
  number?: string | null;
  accent?: string | null;
  selected?: string | null;
  hover?: string | null;
  surface?: string | null;
  border?: string | null;
};

/** `Preferences.custom_theme`: what the `custom` theme is made of. It is kept while another theme is selected. */
export type CustomTheme = {
  /** The theme the custom theme is drawn over: `system` (the default, platform tokens), `siji` (the season's palette) or a built-in theme, never `custom`. An applied package's own manifest base replaces it. */
  base?: Exclude<GlobalTheme, "custom">;
  /** The external candidate skin package id; never a global theme id. Unset means "no package". */
  candidate_skin?: string | null;
  candidate_colors?: CustomCandidateColors;
  /** The keyboard editor design. Unset or `null` means "no design": the custom theme draws its base theme's keyboard. */
  keyboard?: TouchKeyboardSkinDesign | null;
};

/** The base a custom theme is drawn over once a picker is used while `current` is selected: the theme on screen stays underneath, and an already selected custom theme keeps its own base. */
export function customThemeBase(
  current: GlobalTheme,
  custom: CustomTheme | undefined,
): Exclude<GlobalTheme, "custom"> {
  return current === "custom" ? (custom?.base ?? "system") : current;
}

export type ThemeSource = "system" | "builtin" | "custom";

/** `msime_client_resolve_theme()` and `SettingsClient.resolveTheme`. */
export type ResolvedTheme = {
  id: GlobalTheme;
  source: ThemeSource;
  appearance: ThemeAppearance | null;
  candidate: CandidateThemePalette | null;
  keyboard: KeyboardThemePalette | null;
  /** The package the custom candidate colours came from, when one was found. */
  candidate_skin: string | null;
  /** 画的是哪一季，只在 `siji` 作为主题或底时出现。 */
  season?: Season;
};

export type ResolveThemeRequest = {
  global_theme: GlobalTheme;
  custom_theme?: CustomTheme;
  /** The mode the host is drawing in; it only matters for a custom theme's package. */
  dark: boolean;
  /** The candidate layout being drawn; a package is drawn only in a layout its manifest declares. */
  layout: "horizontal" | "vertical";
  /** 宿主本地日历的月份（1-12），只决定 `siji` 画哪一季；省略时宿主库用 UTC 月份。 */
  month?: number;
};

/** `Preferences.app_theme`：Android 宿主自己的页面配色，`siji` 按月份取当季。 */
export type AppTheme = "siji" | "chunya" | "xiayin" | "qiushan" | "dongxue";

/** 应用主题在一种明暗模式下的颜色。 */
export type AppThemeColors = {
  accent: string;
  accent_soft: string;
  on_accent: string;
  background: string;
  card: string;
  hair: string;
};

/** `msime_client_app_theme_catalog()` 的一项；`app-theme-catalog.json` 是它的副本。 */
export type AppThemeCatalogEntry = {
  id: AppTheme;
  title: string;
  season: Season | null;
  seasonal: boolean;
  light: AppThemeColors;
  dark: AppThemeColors;
};

/** `msime_client_resolve_app_theme()`。 */
export type ResolvedAppTheme = AppThemeColors & { id: AppTheme; season: Season };

/** 设置页用的目录：`theme-catalog.json` 不随时钟变化，水杉四季只带固定的秋杉预览；页面画的是当季，所以载入时把它的明暗、预览和色板换成本地当前月份所在季节的那个主题，与宿主 `resolve` 画出来的一致。 */
function withCurrentSeason(entries: ThemeCatalogEntry[]): ThemeCatalogEntry[] {
  const current = entries.find((entry) => entry.id === seasonalThemeId("siji"));
  if (!current) return entries;
  return entries.map((entry) =>
    entry.seasonal
      ? {
          ...entry,
          appearance: current.appearance,
          preview: current.preview,
          candidate: current.candidate,
          keyboard: current.keyboard,
        }
      : entry,
  );
}

export const themeCatalog = withCurrentSeason(catalog as ThemeCatalogEntry[]);
export const defaultGlobalTheme: GlobalTheme = "system";
export const globalThemeIds: GlobalTheme[] = themeCatalog.map((entry) => entry.id);

export function isGlobalTheme(value: unknown): value is GlobalTheme {
  return typeof value === "string" && (globalThemeIds as string[]).includes(value);
}

/** The catalog entry for an id; unknown ids read as `system`, the theme that is always drawable. */
export function themeEntry(id: string | undefined): ThemeCatalogEntry {
  return themeCatalog.find((entry) => entry.id === id) ?? themeCatalog[0];
}

/** The `--cand-*` custom properties the vendored candidate markup reads, for the slots a palette sets. Unset slots keep the stylesheet's own platform defaults. */
export function candidatePaletteStyle(palette: CandidateThemePalette | null): CSSProperties {
  if (!palette) return {};
  const slots: [string, string | null][] = [
    ["--cand-bg", palette.surface],
    ["--cand-border", palette.border],
    ["--cand-text", palette.text],
    ["--cand-num", palette.number],
    ["--cand-accent", palette.accent],
    ["--accent-strong", palette.accent],
    ["--cand-selected", palette.selected],
    ["--cand-hover", palette.hover],
  ];
  return Object.fromEntries(slots.filter(([, value]) => value !== null)) as CSSProperties;
}

/** The candidate preview style of a global theme id: a built-in palette, or nothing for `system`, `custom` and unknown ids. */
export function themeCandidateStyle(id: string | undefined): CSSProperties {
  return candidatePaletteStyle(themeEntry(id).candidate);
}

/** One mode of an external package's candidate palette as the skin scan (`msime_client_skin_catalog`) carries it: manifest colours as written, and the selected-bar switch. */
export type PackageCandidatePalette = Partial<
  Record<
    "surface" | "border" | "text" | "number" | "accent" | "selected" | "hover" | "translation",
    string | null
  >
> & { showSelectedBar?: boolean | null };

/** `theme::normalized_color`: a package colour as `#RRGGBB` or `#RRGGBBAA` (uppercase), converting `#RGB`, `rgb()`/`rgba()` with 0-255 channels and a 0-1 alpha, and `transparent`; `null` for anything else. */
export function normalizedColor(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed.toLowerCase() === "transparent") return "#00000000";
  if (trimmed.startsWith("#")) {
    const hex = trimmed.slice(1);
    if (!/^[0-9a-f]*$/i.test(hex)) return null;
    if (hex.length === 3)
      return `#${[...hex].map((digit) => digit + digit).join("")}`.toUpperCase();
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
  const hex = (channel: number) => channel.toString(16).toUpperCase().padStart(2, "0");
  const rgb = `#${channels.map(hex).join("")}`;
  if (!alpha) return rgb;
  if (!/^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/.test(parts[3])) return null;
  const opacity = Number(parts[3]);
  if (!(opacity >= 0 && opacity <= 1)) return null;
  return `${rgb}${hex(Math.round(opacity * 255))}`;
}

/** `color` (`#RRGGBB` or `#RRGGBBAA`) with its alpha replaced, as `theme::with_alpha` does. */
function withAlpha(color: string, alpha: string): string {
  return `${color.slice(0, 7)}${alpha}`;
}

/** `ai::readable_text`: black or white, whichever reads on `background` (`#RRGGBB` or `#RRGGBBAA`; the alpha is ignored), split at the same relative luminance. */
function readableText(background: string): string {
  const channel = (offset: number) => {
    const value = parseInt(background.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  return luminance > 0.179 ? "#000000" : "#FFFFFF";
}

/** The candidate palette `resolve()` gives a custom theme over `base`, layered the same way: the base palette, then `packagePalette` (the package's palette for the drawn mode, only when the package is drawn in that layout and mode), then the pickers. A text picker also sets the numbers at `9D` alpha unless the number picker is set, the secondary text is the package's translation colour or else the numbers, and over a built-in base the selected row, hover fill, selected text and selected numbers follow the final accent, text and numbers unless set. A selected-row picker instead gives the selected text black or white by the row's luminance, with the selected numbers that colour at `9D` alpha, so a chosen highlight stays readable. `null` when nothing is set, like the Rust palette. `custom-theme-parity.json`, written by the Rust tests, pins this to `resolve()`. */
export function customCandidatePalette(
  base: GlobalTheme | undefined,
  colors: CustomCandidateColors | undefined,
  packagePalette?: PackageCandidatePalette | null,
  month?: number,
): CandidateThemePalette | null {
  // 水杉四季作底时画当季主题的色板，与 `resolve_in` 一致；`month` 省略时取本地当前月份。
  const basePalette = themeEntry(base && seasonalThemeId(base, month)).candidate;
  const derived = basePalette !== null;
  const fromPackage = (value: string | null | undefined) =>
    typeof value === "string" ? normalizedColor(value) : null;
  const picked = (value: unknown) => candidateTextColor(value)?.toUpperCase() ?? null;
  const slot = (key: "surface" | "border" | "text" | "number" | "accent" | "selected" | "hover") =>
    picked(colors?.[key]) ?? fromPackage(packagePalette?.[key]);
  const pickedText = picked(colors?.text);
  const explicitNumber =
    pickedText && !picked(colors?.number) ? withAlpha(pickedText, "9D") : slot("number");
  const text = slot("text") ?? basePalette?.text ?? null;
  const number = explicitNumber ?? basePalette?.number ?? null;
  const accent = slot("accent") ?? basePalette?.accent ?? null;
  const pickedSelected = picked(colors?.selected);
  const pickedSelectedText = pickedSelected ? readableText(pickedSelected) : null;
  const palette: CandidateThemePalette = {
    surface: slot("surface") ?? basePalette?.surface ?? null,
    border: slot("border") ?? basePalette?.border ?? null,
    text,
    number,
    secondary: fromPackage(packagePalette?.translation) ?? number,
    accent,
    selected: slot("selected") ?? (derived && accent ? withAlpha(accent, "24") : null),
    selected_text: pickedSelectedText ?? (derived ? accent : null),
    selected_number: pickedSelectedText
      ? withAlpha(pickedSelectedText, "9D")
      : derived
        ? number
        : null,
    hover: slot("hover") ?? (derived && text ? withAlpha(text, "0F") : null),
    show_selected_bar:
      typeof packagePalette?.showSelectedBar === "boolean" ? packagePalette.showSelectedBar : null,
  };
  return Object.values(palette).some((value) => value !== null) ? palette : null;
}

/** The candidate preview style of a custom theme: `customCandidatePalette` as `--cand-*` properties. */
export function customCandidateStyle(
  base: GlobalTheme | undefined,
  colors: CustomCandidateColors | undefined,
  packagePalette?: PackageCandidatePalette | null,
): CSSProperties {
  return candidatePaletteStyle(customCandidatePalette(base, colors, packagePalette));
}

/** The theme whose keyboard palette is drawn: the selected theme, or for a custom theme with no keyboard design its base, as `resolve()` does. `siji` 换成 `month`（省略时取本地当前月份）所在季节的主题。 */
export function keyboardThemeId(
  theme: GlobalTheme | undefined,
  custom: CustomTheme | undefined,
  month?: number,
): GlobalTheme {
  const id = themeEntry(theme).id;
  const drawn = id === "custom" && !custom?.keyboard ? (custom?.base ?? "system") : id;
  return seasonalThemeId(drawn, month);
}
