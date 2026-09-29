/**
 * Rendering values for the touch keyboard, drawn from the shared global theme.
 *
 * A skin is built one of two ways. A theme's keyboard palette (or, where the theme has none, the Harmony native tokens) gives a flat keyboard in that theme's colours. The user's custom keyboard design gives everything the editor can set, photo, gradient, key shape and material included, because this host draws the full design rather than the flattened palette. Every colour is derived here rather than in the drawing code, so the same values reach the keyboard, the preview and the picker card.
 */
import { CustomKeyboardSkin } from "./CustomKeyboardSkin";
import { GlobalTheme, KeyboardThemePalette } from "./GlobalTheme";
import { KeyboardGeometry } from "../KeyboardGeometry";

function clampChannel(value: number): number {
  return Math.round(KeyboardGeometry.bounded(value, 0, 1) * 255);
}

/** Produces #AARRGGBB: the alpha byte is prefixed to the #RRGGBB part of a colour, replacing any alpha it had. */
function alpha(colour: string, value: number): string {
  return (
    "#" + clampChannel(value).toString(16).toUpperCase().padStart(2, "0") + colour.substring(colour.length - 6)
  );
}

/** The design's key radius, which a theme keyboard draws whatever its colours. */
const THEME_KEY_RADIUS: number = 8;

export class KeyboardSkin {
  /** The global theme id this skin draws: `system`, a built-in id, or `custom`. */
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly dark: boolean;
  readonly background: string;
  readonly keyBackground: string;
  readonly keyForeground: string;
  /** Shift, delete, the symbol and number toggles: the design's `function_key`. */
  readonly functionKeyBackground: string;
  readonly functionKeyForeground: string;
  /** Hints, spellings and the space-bar label. */
  readonly secondary: string;
  /** The selected candidate in the strip, drawn as text with no fill. */
  readonly accent: string;
  /** Text on anything filled with `accent`. */
  readonly onAccent: string;
  /** The return key, which keeps the platform accent with white text whatever the theme. */
  readonly actionBackground: string;
  readonly actionForeground: string;
  /** A switched-on tile, the logo button while its panel is open: the platform accent tint with the accent itself for the glyph, whatever the theme. */
  readonly toggleBackground: string;
  readonly toggleForeground: string;
  readonly cornerRadius: number;
  readonly borderWidth: number;
  readonly borderColor: string;
  readonly shadowOpacity: number;
  readonly shadowRadius: number;
  readonly shadowOffset: number;
  readonly monospaced: boolean;
  readonly pattern: number;
  readonly keyShape: string;
  readonly keyMaterial: string;
  readonly keyOpacity: number;
  readonly gradientEnd: string | null;
  readonly gradientHorizontal: boolean;
  readonly patternOpacity: number;
  readonly photo: Uint8Array | null;
  readonly photoSource: string | null;
  readonly photoShade: number;
  readonly photoPosition: number;
  private readonly designKey: string;

  /**
   * A theme skin passes design as null and takes the flat defaults; the custom design passes the user's design and takes everything from it. One constructor rather than two so no field can be set on one path and forgotten on the other.
   */
  private constructor(
    id: string,
    title: string,
    dark: boolean,
    palette: KeyboardThemePalette,
    design: CustomKeyboardSkin | null,
  ) {
    this.id = id;
    this.title = title;
    this.description = "";
    this.dark = dark;
    this.background = palette.background;
    this.keyBackground = palette.key;
    this.keyForeground = palette.text;
    this.functionKeyBackground = palette.function_key;
    this.functionKeyForeground = palette.text;
    this.secondary = palette.secondary;
    this.accent = palette.accent;
    this.onAccent = palette.on_accent;
    this.actionBackground = GlobalTheme.accent(dark);
    this.actionForeground = "#FFFFFF";
    this.toggleBackground = GlobalTheme.accentSoft(dark);
    this.toggleForeground = GlobalTheme.accent(dark);
    this.shadowRadius = 2;
    this.shadowOffset = 1;
    if (design === null) {
      this.cornerRadius = THEME_KEY_RADIUS;
      this.borderWidth = 0;
      this.borderColor = alpha(palette.accent, 0.28);
      this.shadowOpacity = 0;
      this.monospaced = false;
      this.pattern = 0;
      this.keyShape = "rounded";
      this.keyMaterial = "flat";
      this.keyOpacity = 1;
      this.gradientEnd = null;
      this.gradientHorizontal = false;
      this.patternOpacity = 0.15;
      this.photo = null;
      this.photoSource = null;
      this.photoShade = 0.25;
      this.photoPosition = 0.5;
      // A theme's colours can change under the same id (a custom theme over a new base), so they are part of the identity.
      this.designKey = [palette.background, palette.key, palette.function_key, palette.text,
        palette.secondary, palette.accent].join(",");
    } else {
      this.cornerRadius = design.cornerRadius();
      this.borderWidth = design.borderWidth();
      this.borderColor = design.borderColor();
      this.shadowOpacity = design.shadow();
      this.monospaced = design.monospaced();
      this.pattern = design.pattern();
      this.keyShape = design.keyShape();
      this.keyMaterial = design.keyMaterial();
      this.keyOpacity = design.keyOpacity();
      this.gradientEnd = design.gradientEnd();
      this.gradientHorizontal = design.gradientHorizontal();
      this.patternOpacity = design.patternOpacity();
      this.photo = design.photo();
      this.photoSource = design.photoSource();
      this.photoShade = design.photoShade();
      this.photoPosition = design.photoPosition();
      this.designKey = design.key();
    }
  }

  /**
   * A theme's keyboard. A `null` palette is what `system`, and a custom theme over `system` without a design, resolve to: the Harmony native tokens in the given mode.
   */
  static fromTheme(id: string, title: string, palette: KeyboardThemePalette | null,
                   dark: boolean): KeyboardSkin {
    return new KeyboardSkin(id, title, dark, palette ?? GlobalTheme.nativeKeyboard(dark), null);
  }

  /**
   * The user's custom keyboard design, drawn in full. Its colours are flattened the way the shared `custom_keyboard` flattens them, so the function keys take the design's action colour and hints its text at 60%.
   */
  static fromDesign(title: string, design: CustomKeyboardSkin, dark: boolean): KeyboardSkin {
    const palette: KeyboardThemePalette = {
      background: design.background(),
      key: design.keyBackground(),
      function_key: design.actionBackground(),
      text: design.keyForeground(),
      secondary: alpha(design.keyForeground(), 0.6),
      accent: design.accent(),
      on_accent: design.accentForeground(),
    };
    const skin: KeyboardSkin = new KeyboardSkin("custom", title, dark, palette, design);
    return skin;
  }

  /**
   * A surface's own light/dark mode wins, then the app mode (the `theme` preference), then whatever the system is doing. A theme that fixes its appearance overrides all three; see GlobalTheme.surfaceDark.
   */
  static resolveDark(surfaceTheme: string, appMode: string, systemDark: boolean): boolean {
    if (surfaceTheme === "dark") {
      return true;
    }
    if (surfaceTheme === "light") {
      return false;
    }
    if (appMode === "dark") {
      return true;
    }
    if (appMode === "light") {
      return false;
    }
    return systemDark;
  }

  /**
   * A translucent key background, for the surfaces that sit over the keyboard rather than among the
   * keys: the candidate strip and the nine-key sidebar.
   *
   * The alpha belongs to the colour, not to the view. Setting opacity on the container fades
   * everything inside it too, which turned the strip's icons and its scheme pill into smudges.
   */
  translucentKeyBackground(value: number): string {
    return alpha(this.keyBackground, value);
  }

  /** A wash of the accent, for marking a selected card without hiding what is printed on it. */
  tintedAccent(value: number): string {
    return alpha(this.accent, value);
  }

  patternColor(): string {
    return alpha(this.accent, this.patternOpacity);
  }
  photoShadeColor(): string {
    return alpha("#000000", this.photoShade);
  }
  shadowColor(): string {
    return alpha("#000000", this.shadowOpacity);
  }
  /** A key's fill: the platform accent for an emphasized key, the function-key colour for a special one (shift, delete, 123, the language key, an idle return), and the key colour for the rest. */
  keySurfaceBackground(emphasized: boolean, special: boolean = false): string {
    if (emphasized) {
      return this.actionBackground;
    }
    return alpha(special ? this.functionKeyBackground : this.keyBackground, this.keyOpacity);
  }

  /** The label colour matching keySurfaceBackground. */
  keyLabelColor(emphasized: boolean, special: boolean = false): string {
    if (emphasized) {
      return this.actionForeground;
    }
    return special ? this.functionKeyForeground : this.keyForeground;
  }
  keyCornerRadius(): number {
    if (this.keyShape === "capsule") return 999;
    if (this.keyShape === "ticket") return Math.min(3, this.cornerRadius);
    if (this.keyShape === "pebble") return Math.max(12, this.cornerRadius);
    return this.cornerRadius;
  }
  materialTop(): string {
    if (this.keyMaterial === "glass") return "#3DFFFFFF";
    if (this.keyMaterial === "raised") return "#21FFFFFF";
    if (this.keyMaterial === "paper") return "#0AFFFFFF";
    return "#00FFFFFF";
  }
  materialBottom(): string {
    if (this.keyMaterial === "glass") return "#08000000";
    if (this.keyMaterial === "raised") return "#1A000000";
    if (this.keyMaterial === "paper") return "#0F000000";
    return "#00000000";
  }

  /** Identity for caching a rendered skin, including the custom design it was built from. */
  key(): string {
    return this.id + ":" + this.dark + (this.designKey.length === 0 ? "" : ":" + this.designKey);
  }
}
