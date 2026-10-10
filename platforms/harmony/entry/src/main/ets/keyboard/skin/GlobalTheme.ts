/**
 * The shared global theme as this host reads it: the catalog the picker draws, the resolved answer every surface is coloured from, and the ArkUI tokens a `null` slot falls back to.
 *
 * The ids, titles and palettes all come from `msime_client_theme_catalog` and `msime_client_resolve_theme` (crates/client-core/src/skin/theme.rs). Nothing here keeps a copy of them. What this file does own is the Harmony native token table, because `system` and every `null` slot mean "draw the platform's own colours", and those are this host's to define.
 *
 * The shared layer writes colours as `#RRGGBB` or `#RRGGBBAA` with the alpha last. ArkUI reads an eight-digit colour as `#AARRGGBB`, so every colour is converted once, when the answer is parsed, and nothing downstream sees the shared order.
 */
import { CustomSkinDocument } from "./CustomKeyboardSkin";

export interface CandidateThemePalette {
  readonly surface: string | null;
  readonly border: string | null;
  readonly text: string | null;
  readonly number: string | null;
  readonly secondary: string | null;
  readonly accent: string | null;
  readonly selected: string | null;
  readonly selected_text: string | null;
  readonly selected_number: string | null;
  readonly hover: string | null;
  readonly show_selected_bar: boolean | null;
}

export interface KeyboardThemePalette {
  readonly background: string;
  readonly key: string;
  readonly function_key: string;
  readonly text: string;
  readonly secondary: string;
  readonly accent: string;
  readonly on_accent: string;
}

export interface ThemePreview {
  readonly background: string;
  readonly panel: string;
  readonly accent: string;
  readonly text: string;
}

export interface ThemeCatalogEntry {
  readonly id: string;
  readonly title: string;
  readonly appearance: string | null;
  readonly preview: ThemePreview | null;
  readonly candidate: CandidateThemePalette | null;
  readonly keyboard: KeyboardThemePalette | null;
}

export interface ResolvedTheme {
  readonly id: string;
  /** `system`, `builtin` or `custom`. */
  readonly source: string;
  /** `light` or `dark` when the theme fixes the mode; `null` when it follows the host. */
  readonly appearance: string | null;
  readonly candidate: CandidateThemePalette | null;
  readonly keyboard: KeyboardThemePalette | null;
  /** The external package whose colours were drawn. The single signal for its decoration and minimum width. */
  readonly candidate_skin: string | null;
}

export interface CustomCandidateColors {
  readonly text?: string | null;
  readonly number?: string | null;
  readonly accent?: string | null;
  readonly selected?: string | null;
  readonly hover?: string | null;
  readonly surface?: string | null;
  readonly border?: string | null;
}

/** The shared `custom_theme` preference. Every member is optional; an absent one takes its shared default. */
export interface CustomThemeDocument {
  readonly base?: string;
  /** 浅色模式的候选皮肤包，也是 `candidate_skin_dark` 没设时深色模式用的那款。 */
  readonly candidate_skin?: string;
  /** 深色模式的候选皮肤包。两个槽位都由 `msime_client_resolve_theme` 按 `dark` 取用，宿主不自己挑。 */
  readonly candidate_skin_dark?: string;
  readonly candidate_colors?: CustomCandidateColors;
  readonly keyboard?: CustomSkinDocument;
}

/** The `msime_client_resolve_theme` request. Harmony always names its skin root and never sends `package`. */
export interface ResolveThemeRequest {
  readonly global_theme: string;
  readonly custom_theme?: CustomThemeDocument;
  readonly dark: boolean;
  readonly layout: string;
  readonly skins_directory: string;
}

/** Every candidate-window colour, with the native token already standing in for any slot the theme left unset. */
export interface CandidateColors {
  readonly surface: string;
  readonly border: string;
  readonly text: string;
  readonly number: string;
  readonly secondary: string;
  readonly accent: string;
  readonly selected: string;
  readonly selectedText: string;
  readonly selectedNumber: string;
  readonly hover: string;
  readonly showSelectedBar: boolean;
}

interface Envelope {
  ok?: boolean;
  value?: Object;
  error?: string;
}

interface CatalogValue {
  themes?: Object[];
}

// ---- Harmony 原生令牌（设计稿 `tok()` 中的 harmony 条目，`全平台 UI.dc.html` L2526-2529：`kbBg`、`keyBg`、`keySpec`、`kbSub`） ----

// 尚未解析出季节种子时键盘回退使用的基础配色。有种子时，`system` 改用 `AppThemePalette` 的公式绘制。
const NATIVE_KEYBOARD_LIGHT: KeyboardThemePalette = {
  background: "#D9E2D6",
  key: "#FCFDFA",
  function_key: "#B8C8B5",
  text: "#182431",
  secondary: "#5A6B5D",
  accent: "#2C7A4B",
  on_accent: "#FFFFFF",
};

// `on_accent` 是 #5FBF84 与黑色按 25% 混合（`AppThemePalette.onAccent`，即共享层的深色规则），因为 #5FBF84 上的白字对比度只有约 2.3:1。
const NATIVE_KEYBOARD_DARK: KeyboardThemePalette = {
  background: "#121814",
  key: "#303A32",
  function_key: "#212923",
  text: "#E5E5E5",
  secondary: "#93A596",
  accent: "#5FBF84",
  on_accent: "#183021",
};

// 2in1 候选窗（设计稿中的 hm2 条目）：白色或 #262626 的卡片、细线边框，选中行用强调色淡底而不是色条。淡底即设计稿中季节的 `accent + '22'` / `accent + '40'`，套用在平台强调色上。
const NATIVE_CANDIDATE_LIGHT: CandidateColors = {
  surface: "#FFFFFF",
  border: "#0F000000",
  text: "#182431",
  number: "#99182431",
  secondary: "#99182431",
  accent: "#2C7A4B",
  selected: "#222C7A4B",
  selectedText: "#2C7A4B",
  selectedNumber: "#99182431",
  hover: "#0D000000",
  showSelectedBar: false,
};

const NATIVE_CANDIDATE_DARK: CandidateColors = {
  surface: "#262626",
  border: "#14FFFFFF",
  text: "#E5E5E5",
  number: "#99FFFFFF",
  secondary: "#99FFFFFF",
  accent: "#5FBF84",
  selected: "#405FBF84",
  selectedText: "#5FBF84",
  selectedNumber: "#99FFFFFF",
  hover: "#14FFFFFF",
  showSelectedBar: false,
};

function isObject(value: Object | null | undefined): boolean {
  return (
    value !== null && value !== undefined && typeof value === "object" && !Array.isArray(value)
  );
}

function field(value: Object, name: string): Object | null | undefined {
  return (value as Record<string, Object | null | undefined>)[name];
}

function text(value: Object, name: string): string | null {
  const member: Object | null | undefined = field(value, name);
  return typeof member === "string" ? (member as string) : null;
}

export class GlobalTheme {
  /** The platform accent, which the return key and the function tiles keep whatever the theme. */
  static accent(dark: boolean): string {
    return dark ? "#5FBF84" : "#2C7A4B";
  }

  /** 平台强调色的淡色（浅色下 alpha 0x22，约 13%；深色下 0x40，25%，即设计稿的 `accent + '22'` / `accent + '40'`）：未解析出季节种子时，功能块或 logo 按钮开启状态的填充色。 */
  static accentSoft(dark: boolean): string {
    return dark ? "#405FBF84" : "#222C7A4B";
  }

  /** The ArkUI keyboard tokens `system`, and any theme without a keyboard palette, draw. */
  static nativeKeyboard(dark: boolean): KeyboardThemePalette {
    return dark ? NATIVE_KEYBOARD_DARK : NATIVE_KEYBOARD_LIGHT;
  }

  /**
   * A shared colour in ArkUI order: `#RRGGBB` passes through, `#RRGGBBAA` becomes `#AARRGGBB`, and anything else is `null` so a malformed answer can never reach a component.
   */
  static arkColor(value: string | null | undefined): string | null {
    if (value === null || value === undefined) {
      return null;
    }
    if (/^#[0-9A-Fa-f]{6}$/.test(value)) {
      return value.toUpperCase();
    }
    if (/^#[0-9A-Fa-f]{8}$/.test(value)) {
      return ("#" + value.substring(7, 9) + value.substring(1, 7)).toUpperCase();
    }
    return null;
  }

  /**
   * An ArkUI colour with its alpha multiplied by `factor`, which is clamped to 0..1: `#RRGGBB` counts as opaque, and a colour in any other form comes back unchanged rather than being guessed at.
   */
  static withOpacity(value: string, factor: number): string {
    const scale: number = Number.isFinite(factor) ? Math.max(0, Math.min(1, factor)) : 1;
    if (scale >= 1) {
      return value;
    }
    let alpha: number;
    let rgb: string;
    if (/^#[0-9A-Fa-f]{6}$/.test(value)) {
      alpha = 255;
      rgb = value.substring(1);
    } else if (/^#[0-9A-Fa-f]{8}$/.test(value)) {
      alpha = parseInt(value.substring(1, 3), 16);
      rgb = value.substring(3);
    } else {
      return value;
    }
    const scaled: string = Math.round(alpha * scale)
      .toString(16)
      .toUpperCase();
    return "#" + (scaled.length === 1 ? "0" + scaled : scaled) + rgb.toUpperCase();
  }

  /** 浅色、深色两次解析给出同一个固定明暗时，主题才算固定了明暗（内置主题、没设皮肤的自定义主题叠在固定明暗的底上）。自定义主题的浅色、深色槽位各放一款皮肤时两次解析的明暗不同，这时返回 `null`，各个界面跟随自己的明暗规则，不能被浅色那次解析钉住。与 macOS 的 `FixedThemeMode` 同一条规则。 */
  static fixedAppearance(light: ResolvedTheme | null, dark: ResolvedTheme | null): string | null {
    if (light === null || dark === null || light.appearance !== dark.appearance) {
      return null;
    }
    return light.appearance;
  }

  /** A non-null appearance fixes the mode for every surface; otherwise the surface's own mode rule decides. */
  static surfaceDark(appearance: string | null, fallbackDark: boolean): boolean {
    if (appearance === "dark") {
      return true;
    }
    if (appearance === "light") {
      return false;
    }
    return fallbackDark;
  }

  /** The candidate window's colours for one mode, every unset slot filled from the native table. */
  static candidateColors(palette: CandidateThemePalette | null, dark: boolean): CandidateColors {
    const native: CandidateColors = dark ? NATIVE_CANDIDATE_DARK : NATIVE_CANDIDATE_LIGHT;
    if (palette === null) {
      return native;
    }
    const number: string = palette.number ?? native.number;
    return {
      surface: palette.surface ?? native.surface,
      border: palette.border ?? native.border,
      text: palette.text ?? native.text,
      number: number,
      secondary: palette.secondary ?? number,
      accent: palette.accent ?? native.accent,
      selected: palette.selected ?? native.selected,
      selectedText: palette.selected_text ?? native.selectedText,
      selectedNumber: palette.selected_number ?? native.selectedNumber,
      hover: palette.hover ?? native.hover,
      showSelectedBar: palette.show_selected_bar ?? native.showSelectedBar,
    };
  }

  static candidatePalette(value: Object | null | undefined): CandidateThemePalette | null {
    if (value === null || value === undefined || !isObject(value)) {
      return null;
    }
    const bar: Object | null | undefined = field(value, "show_selected_bar");
    return {
      surface: GlobalTheme.arkColor(text(value, "surface")),
      border: GlobalTheme.arkColor(text(value, "border")),
      text: GlobalTheme.arkColor(text(value, "text")),
      number: GlobalTheme.arkColor(text(value, "number")),
      secondary: GlobalTheme.arkColor(text(value, "secondary")),
      accent: GlobalTheme.arkColor(text(value, "accent")),
      selected: GlobalTheme.arkColor(text(value, "selected")),
      selected_text: GlobalTheme.arkColor(text(value, "selected_text")),
      selected_number: GlobalTheme.arkColor(text(value, "selected_number")),
      hover: GlobalTheme.arkColor(text(value, "hover")),
      show_selected_bar: typeof bar === "boolean" ? (bar as boolean) : null,
    };
  }

  /** The shared keyboard palette has no optional slot, so one unreadable colour drops the whole palette to the native one. */
  static keyboardPalette(value: Object | null | undefined): KeyboardThemePalette | null {
    if (value === null || value === undefined || !isObject(value)) {
      return null;
    }
    const background: string | null = GlobalTheme.arkColor(text(value, "background"));
    const key: string | null = GlobalTheme.arkColor(text(value, "key"));
    const functionKey: string | null = GlobalTheme.arkColor(text(value, "function_key"));
    const foreground: string | null = GlobalTheme.arkColor(text(value, "text"));
    const secondary: string | null = GlobalTheme.arkColor(text(value, "secondary"));
    const accent: string | null = GlobalTheme.arkColor(text(value, "accent"));
    const onAccent: string | null = GlobalTheme.arkColor(text(value, "on_accent"));
    if (
      background === null ||
      key === null ||
      functionKey === null ||
      foreground === null ||
      secondary === null ||
      accent === null ||
      onAccent === null
    ) {
      return null;
    }
    return {
      background: background,
      key: key,
      function_key: functionKey,
      text: foreground,
      secondary: secondary,
      accent: accent,
      on_accent: onAccent,
    };
  }

  private static preview(value: Object | null | undefined): ThemePreview | null {
    if (value === null || value === undefined || !isObject(value)) {
      return null;
    }
    const background: string | null = GlobalTheme.arkColor(text(value, "background"));
    const panel: string | null = GlobalTheme.arkColor(text(value, "panel"));
    const accent: string | null = GlobalTheme.arkColor(text(value, "accent"));
    const foreground: string | null = GlobalTheme.arkColor(text(value, "text"));
    if (background === null || panel === null || accent === null || foreground === null) {
      return null;
    }
    return { background: background, panel: panel, accent: accent, text: foreground };
  }

  private static appearance(value: Object): string | null {
    const appearance: string | null = text(value, "appearance");
    return appearance === "light" || appearance === "dark" ? appearance : null;
  }

  /** Reads a `msime_client_resolve_theme` response. A refusal or an unreadable answer is `null`, which draws the native tokens. */
  static parseResolved(response: string): ResolvedTheme | null {
    const envelope: Envelope = JSON.parse(response) as Envelope;
    if (envelope.ok !== true || envelope.value === undefined || !isObject(envelope.value)) {
      return null;
    }
    const value: Object = envelope.value;
    const id: string | null = text(value, "id");
    const source: string | null = text(value, "source");
    if (id === null || source === null) {
      return null;
    }
    return {
      id: id,
      source: source,
      appearance: GlobalTheme.appearance(value),
      candidate: GlobalTheme.candidatePalette(field(value, "candidate")),
      keyboard: GlobalTheme.keyboardPalette(field(value, "keyboard")),
      candidate_skin: text(value, "candidate_skin"),
    };
  }

  /** Reads a `msime_client_theme_catalog` response, in the shared picker order. An unreadable entry is skipped. */
  static parseCatalog(response: string): ThemeCatalogEntry[] {
    const envelope: Envelope = JSON.parse(response) as Envelope;
    if (envelope.ok !== true || envelope.value === undefined || !isObject(envelope.value)) {
      return [];
    }
    const themes: Object[] | undefined = (envelope.value as CatalogValue).themes;
    if (!Array.isArray(themes)) {
      return [];
    }
    const entries: ThemeCatalogEntry[] = [];
    for (const theme of themes) {
      if (!isObject(theme)) {
        continue;
      }
      const id: string | null = text(theme, "id");
      const title: string | null = text(theme, "title");
      if (id === null || title === null) {
        continue;
      }
      entries.push({
        id: id,
        title: title,
        appearance: GlobalTheme.appearance(theme),
        preview: GlobalTheme.preview(field(theme, "preview")),
        candidate: GlobalTheme.candidatePalette(field(theme, "candidate")),
        keyboard: GlobalTheme.keyboardPalette(field(theme, "keyboard")),
      });
    }
    return entries;
  }

  /** The catalog entry for an id, or `null` when the catalog does not carry it. */
  static entry(catalog: ThemeCatalogEntry[], id: string): ThemeCatalogEntry | null {
    for (const entry of catalog) {
      if (entry.id === id) {
        return entry;
      }
    }
    return null;
  }
}
