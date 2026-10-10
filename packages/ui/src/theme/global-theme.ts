/**
 * The global theme contract, as the settings page sees it. The authoritative table is `crates/client-core/src/skin/theme.rs`; `theme-catalog.json` is its serialized `catalog()`, checked against it by the Rust test `web_catalog_copy_matches_the_catalog`, so the page can draw the picker and its previews synchronously on every host, including hosts whose bridge has no theme call.
 *
 * Every colour is `#RRGGBB` or `#RRGGBBAA` (uppercase, alpha last). A `null` slot means "the host's own platform token", never "transparent".
 */
import type { CSSProperties } from "react";
import type { HostPlatform } from "../index";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { candidateTextColor } from "../candidate/candidate-text-color";
import catalog from "./theme-catalog.json";

/** `Preferences.global_theme`：候选窗、悬浮工具栏、菜单和触屏键盘共用的一个主题 id。`native`（原生）只在 iOS 提供，见 `offeredThemeCatalog`。 */
export type GlobalTheme =
  | "system"
  | "native"
  | "shuishan"
  | "light"
  | "paper"
  | "night"
  | "ink"
  | "custom";
export type BuiltinGlobalTheme = Exclude<GlobalTheme, "system" | "native" | "custom">;
/** 自定义主题能画在其上的主题：`system` 或内置主题，不能是 `custom`，也不能是只在部分宿主上提供的 `native`。 */
export type BaseGlobalTheme = Exclude<GlobalTheme, "native" | "custom">;
export type ThemeAppearance = "light" | "dark";

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

/** 选择器的一项。`system`、`native` 和 `custom` 不带调色板。 */
export type ThemeCatalogEntry = {
  id: GlobalTheme;
  title: string;
  /** 提供这个主题的宿主，`null` 表示所有宿主都提供。 */
  platforms: HostPlatform[] | null;
  appearance: ThemeAppearance | null;
  preview: ThemePreview | null;
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
  /** 自定义主题的底：`system`（默认，平台 token）或内置主题，不能是 `custom` 或 `native`。应用了皮肤包时改用包清单自己的 base。 */
  base?: BaseGlobalTheme;
  /** 浅色模式用的候选窗皮肤包 id，也是没设 `candidate_skin_dark` 时深色模式用的那款；不会是全局主题 id。未设表示没有皮肤包。 */
  candidate_skin?: string | null;
  /** 深色模式用的候选窗皮肤包 id。未设时深色模式也取 `candidate_skin`。 */
  candidate_skin_dark?: string | null;
  candidate_colors?: CustomCandidateColors;
  /** The keyboard editor design. Unset or `null` means "no design": the custom theme draws its base theme's keyboard. */
  keyboard?: TouchKeyboardSkinDesign | null;
};

/** 以 `base` 为底的皮肤包放在哪个槽位：固定明暗的内置主题给出它自己的明暗，`system` 为 `null`，两个槽位都放（`theme::skin_appearance`）。 */
export function skinSlot(base: GlobalTheme): ThemeAppearance | null {
  return themeEntry(base).appearance;
}

/**
 * 把皮肤包 `id`（清单 `base` 为 `base`）放进它所属的槽位，返回新的 `custom_theme`：深色皮肤写 `candidate_skin_dark`，浅色皮肤写 `candidate_skin`，`system` 底的写两个。`base` 照旧写成包的 base。
 *
 * 写深色槽位时，原来放在 `candidate_skin` 里的深色皮肤一并清掉。写浅色槽位时，如果深色槽位还空着，而原来的 `candidate_skin` 确知是深色或 `system` 底的皮肤（`slotOf` 给出 `"dark"` 或 `null`），就先把它挪到深色槽位；`slotOf` 给出 `undefined`（包不在目录里，槽位不知道）时不挪，直接覆盖，免得把一个本机没有的包塞进深色槽位：只设过一款深色皮肤的旧文档把它存在 `candidate_skin` 里，深色模式靠回退取到它，直接覆盖会让它悄悄消失。
 */
export function applyCandidateSkin(
  custom: CustomTheme | undefined,
  id: string,
  base: BaseGlobalTheme,
  slotOf: (id: string) => ThemeAppearance | null | undefined,
): CustomTheme {
  const slot = skinSlot(base);
  const next: CustomTheme = { ...custom, base };
  if (slot !== "light") next.candidate_skin_dark = id;
  // 旧文档放在 `candidate_skin` 里的深色皮肤被新的深色皮肤取代：浅色模式本来就不画它，留着只会让它看起来还在用。
  if (slot === "dark" && custom?.candidate_skin && slotOf(custom.candidate_skin) === "dark")
    next.candidate_skin = null;
  if (slot !== "dark") {
    const previous = custom?.candidate_skin || null;
    if (
      slot === "light" &&
      !custom?.candidate_skin_dark &&
      previous &&
      (slotOf(previous) === "dark" || slotOf(previous) === null)
    )
      next.candidate_skin_dark = previous;
    next.candidate_skin = id;
  }
  return next;
}

/** 取消使用皮肤包 `id`：清掉放着它的槽位，另一个槽位不动。取下的是最后一款皮肤时底改回 `system`：应用皮肤时底被写成包的 base，两个槽位都空了以后 `resolve` 会在两种明暗下都用这个底，留着深色皮肤的 `night` 会让浅色模式变成深色。 */
export function removeCandidateSkin(custom: CustomTheme | undefined, id: string): CustomTheme {
  if (custom?.candidate_skin !== id && custom?.candidate_skin_dark !== id) return { ...custom };
  const next: CustomTheme = { ...custom };
  if (next.candidate_skin === id) next.candidate_skin = null;
  if (next.candidate_skin_dark === id) next.candidate_skin_dark = null;
  if (!next.candidate_skin && !next.candidate_skin_dark) next.base = "system";
  return next;
}

/** 选着 `current` 时用了取色器，自定义主题画在哪个底上：屏幕上的主题留在下面，已经选着的自定义主题保留自己的底。`native` 不能当底，从它开始自定义时底是 `system`。 */
export function customThemeBase(
  current: GlobalTheme,
  custom: CustomTheme | undefined,
): BaseGlobalTheme {
  if (current === "custom") return custom?.base ?? "system";
  return current === "native" ? "system" : current;
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
};

export type ResolveThemeRequest = {
  global_theme: GlobalTheme;
  custom_theme?: CustomTheme;
  /** The mode the host is drawing in; it only matters for a custom theme's package. */
  dark: boolean;
  /** The candidate layout being drawn; a package is drawn only in a layout its manifest declares. */
  layout: "horizontal" | "vertical";
};

export const themeCatalog = catalog as ThemeCatalogEntry[];
export const defaultGlobalTheme: GlobalTheme = "system";
export const globalThemeIds: GlobalTheme[] = themeCatalog.map((entry) => entry.id);

export function isGlobalTheme(value: unknown): value is GlobalTheme {
  return typeof value === "string" && (globalThemeIds as string[]).includes(value);
}

/** `platform` 的选择器列出的主题，顺序同 `themeCatalog`。只在部分宿主上提供的主题（`platforms` 不为 `null`）只列在那些宿主上；不知道宿主时一个也不列。 */
export function offeredThemeCatalog(platform: HostPlatform | undefined): ThemeCatalogEntry[] {
  return themeCatalog.filter(
    (entry) =>
      entry.platforms === null || (platform !== undefined && entry.platforms.includes(platform)),
  );
}

/** 宿主实际画的主题：宿主不提供的主题（iOS 以外收到的 `native`）按 `system` 画，选择器也把「跟随系统」标为选中。 */
export function offeredGlobalTheme(
  theme: GlobalTheme,
  platform: HostPlatform | undefined,
): GlobalTheme {
  return offeredThemeCatalog(platform).some((entry) => entry.id === theme) ? theme : "system";
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
  // 写成 `\d+(\.\d*)?` 而不是 `\d+\.?\d*`：两者接受的串相同，后者在一长串数字后跟非法字符时要回溯平方次，恶意皮肤的颜色能卡住页面。
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$/.test(parts[3])) return null;
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
): CandidateThemePalette | null {
  const basePalette = themeEntry(base).candidate;
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

/** `CustomTheme::candidate_skin_for`：`dark` 模式下取哪个槽位的皮肤包。深色模式先取 `candidate_skin_dark`，没设时与浅色模式一样取 `candidate_skin`。 */
export function candidateSkinFor(custom: CustomTheme | undefined, dark: boolean): string | null {
  const light = custom?.candidate_skin || null;
  return dark ? custom?.candidate_skin_dark || light : light;
}

/** `ThemePackage::draws_in`：以 `base` 为底的皮肤包能否在 `dark` 模式下画。固定明暗的内置主题只画在自己的明暗下，`system` 两种都画。 */
export function skinDrawsIn(base: GlobalTheme, dark: boolean): boolean {
  const appearance = themeEntry(base).appearance;
  return appearance === null || (appearance === "dark") === dark;
}

/** `resolve()` 给自定义主题选的底：画了皮肤包时是包的 base（`drawnPackageBase`）；设过皮肤、却没有能在 `dark` 下画的包时，`custom.base` 只在属于这种明暗时作底，否则是 `system`；没设皮肤时就是 `custom.base`。 */
export function customDrawnBase(
  custom: CustomTheme | undefined,
  drawnPackageBase: BaseGlobalTheme | null,
  dark: boolean,
): BaseGlobalTheme {
  if (drawnPackageBase !== null) return drawnPackageBase;
  const base = custom?.base ?? "system";
  const configured = Boolean(custom?.candidate_skin || custom?.candidate_skin_dark);
  return !configured || skinDrawsIn(base, dark) ? base : "system";
}

/** The candidate preview style of a custom theme: `customCandidatePalette` as `--cand-*` properties. */
export function customCandidateStyle(
  base: GlobalTheme | undefined,
  colors: CustomCandidateColors | undefined,
  packagePalette?: PackageCandidatePalette | null,
): CSSProperties {
  return candidatePaletteStyle(customCandidatePalette(base, colors, packagePalette));
}

/** The theme whose keyboard palette is drawn: the selected theme, or for a custom theme with no keyboard design its base, as `resolve()` does. */
export function keyboardThemeId(
  theme: GlobalTheme | undefined,
  custom: CustomTheme | undefined,
): GlobalTheme {
  const id = themeEntry(theme).id;
  return id === "custom" && !custom?.keyboard ? (custom?.base ?? "system") : id;
}
