/**
 * Rendering values for the touch-keyboard skin preference, ported from
 * platforms/android/java/app/msime/client/KeyboardSkin.java.
 *
 * Eight built-in skins plus the user's custom design. Every colour is derived here rather than in the
 * drawing code, so the same values reach the keyboard, the preview and the settings card.
 */
import { CustomKeyboardSkin } from "./CustomKeyboardSkin";
import { KeyboardGeometry } from "../KeyboardGeometry";

function clampChannel(value: number): number {
  return Math.round(KeyboardGeometry.bounded(value, 0, 1) * 255);
}

/** Matches the Java String.format("#%02X%02X%02X", ...) exactly, including the upper case. */
function rgb(red: number, green: number, blue: number): string {
  return (
    "#" +
    [clampChannel(red), clampChannel(green), clampChannel(blue)]
      .map((channel: number) => channel.toString(16).toUpperCase().padStart(2, "0"))
      .join("")
  );
}

/** Produces #AARRGGBB: the alpha byte is prefixed to an existing #RRGGBB. */
function alpha(colour: string, value: number): string {
  return (
    "#" + clampChannel(value).toString(16).toUpperCase().padStart(2, "0") + colour.substring(1)
  );
}

function adaptive(dark: boolean, light: string, darkValue: string): string {
  return dark ? darkValue : light;
}

function label(dark: boolean): string {
  return dark ? "#FFFFFF" : "#000000";
}

export class KeyboardSkin {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly dark: boolean;
  readonly background: string;
  readonly keyBackground: string;
  readonly keyForeground: string;
  readonly accent: string;
  readonly actionBackground: string;
  readonly actionForeground: string;
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
   * A built-in skin passes design as null and takes the flat defaults; the custom skin passes the
   * user's design and takes everything from it. One constructor rather than two so no field can be
   * set on one path and forgotten on the other.
   */
  private constructor(
    id: string,
    title: string,
    description: string,
    dark: boolean,
    background: string,
    keyBackground: string,
    keyForeground: string,
    accent: string,
    actionBackground: string,
    cornerRadius: number,
    borderWidth: number,
    shadowOpacity: number,
    shadowRadius: number,
    shadowOffset: number,
    monospaced: boolean,
    pattern: number,
    design: CustomKeyboardSkin | null = null,
  ) {
    this.id = id;
    this.title = title;
    this.description = description;
    this.dark = dark;
    this.background = background;
    this.keyBackground = keyBackground;
    this.keyForeground = keyForeground;
    this.accent = accent;
    this.actionBackground = actionBackground;
    this.cornerRadius = cornerRadius;
    this.borderWidth = borderWidth;
    this.shadowOpacity = shadowOpacity;
    this.shadowRadius = shadowRadius;
    this.shadowOffset = shadowOffset;
    this.monospaced = monospaced;
    this.pattern = pattern;
    if (design === null) {
      this.actionForeground = "#FFFFFF";
      // Midnight carries a neon edge, so its border is far less transparent than the others.
      this.borderColor = alpha(accent, id === "midnight" ? 0.65 : 0.28);
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
      this.designKey = "";
    } else {
      this.actionForeground = design.actionForeground();
      this.borderColor = design.borderColor();
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

  private static fromDesign(design: CustomKeyboardSkin, dark: boolean): KeyboardSkin {
    return new KeyboardSkin(
      "custom",
      "我的皮肤",
      "自由配色 · 自定义键帽",
      dark,
      design.background(),
      design.keyBackground(),
      design.keyForeground(),
      design.accent(),
      design.actionBackground(),
      design.cornerRadius(),
      design.borderWidth(),
      design.shadow(),
      2,
      1,
      design.monospaced(),
      design.pattern(),
      design,
    );
  }

  /** Keyboard theme wins, then the global theme, then whatever the system is doing. */
  static resolveDark(keyboardTheme: string, globalTheme: string, systemDark: boolean): boolean {
    if (keyboardTheme === "dark") {
      return true;
    }
    if (keyboardTheme === "light") {
      return false;
    }
    if (globalTheme === "dark") {
      return true;
    }
    if (globalTheme === "light") {
      return false;
    }
    return systemDark;
  }

  static from(
    value: string | null,
    dark: boolean = false,
    design: CustomKeyboardSkin | null = null,
  ): KeyboardSkin {
    const id: string = value === null ? "" : value;
    switch (id) {
      case "custom":
        return KeyboardSkin.fromDesign(
          design === null ? CustomKeyboardSkin.defaults() : design,
          dark,
        );
      case "ocean":
        return new KeyboardSkin(
          id,
          "海盐蓝",
          "海盐浅蓝 · 轻盈平面",
          dark,
          adaptive(dark, rgb(0.9, 0.94, 0.98), rgb(0.09, 0.12, 0.17)),
          adaptive(dark, "#FFFFFF", rgb(0.18, 0.22, 0.29)),
          label(dark),
          adaptive(dark, rgb(0.12, 0.36, 0.64), rgb(0.5, 0.74, 0.98)),
          adaptive(dark, rgb(0.12, 0.36, 0.64), rgb(0.16, 0.36, 0.62)),
          8,
          0,
          0,
          3,
          2,
          false,
          0,
        );
      case "rose":
        return new KeyboardSkin(
          id,
          "浅蔷薇",
          "柔和蔷薇 · 简洁圆角",
          dark,
          adaptive(dark, rgb(0.98, 0.91, 0.94), rgb(0.16, 0.1, 0.13)),
          adaptive(dark, "#FFFFFF", rgb(0.27, 0.19, 0.23)),
          label(dark),
          adaptive(dark, rgb(0.63, 0.25, 0.39), rgb(0.96, 0.62, 0.74)),
          adaptive(dark, rgb(0.63, 0.25, 0.39), rgb(0.56, 0.23, 0.36)),
          8,
          0,
          0,
          3,
          2,
          false,
          0,
        );
      case "porcelain":
        return new KeyboardSkin(
          id,
          "素白瓷",
          "细线边框 · 克制直角",
          dark,
          adaptive(dark, rgb(0.92, 0.93, 0.94), rgb(0.1, 0.11, 0.13)),
          adaptive(dark, rgb(0.99, 0.99, 0.99), rgb(0.2, 0.21, 0.23)),
          label(dark),
          adaptive(dark, rgb(0.2, 0.24, 0.28), rgb(0.8, 0.84, 0.89)),
          adaptive(dark, rgb(0.2, 0.24, 0.28), rgb(0.27, 0.31, 0.36)),
          3,
          0.5,
          0,
          3,
          2,
          false,
          0,
        );
      case "typewriter":
        return new KeyboardSkin(
          id,
          "纸上时光",
          "暖纸网点 · 复古键帽",
          dark,
          adaptive(dark, rgb(0.89, 0.84, 0.74), rgb(0.15, 0.13, 0.1)),
          adaptive(dark, rgb(0.99, 0.96, 0.88), rgb(0.25, 0.22, 0.17)),
          label(dark),
          adaptive(dark, rgb(0.37, 0.25, 0.15), rgb(0.87, 0.72, 0.51)),
          adaptive(dark, rgb(0.37, 0.25, 0.15), rgb(0.4, 0.28, 0.18)),
          5,
          1,
          0.3,
          0,
          3,
          true,
          1,
        );
      case "candy":
        return new KeyboardSkin(
          id,
          "奶油桃桃",
          "奶油波纹 · 饱满圆角",
          dark,
          adaptive(dark, rgb(0.99, 0.88, 0.82), rgb(0.19, 0.12, 0.15)),
          adaptive(dark, rgb(1, 0.97, 0.93), rgb(0.3, 0.2, 0.24)),
          label(dark),
          adaptive(dark, rgb(0.58, 0.22, 0.32), rgb(1, 0.66, 0.73)),
          adaptive(dark, rgb(0.58, 0.22, 0.32), rgb(0.58, 0.22, 0.32)),
          18,
          0,
          0.16,
          3,
          2,
          false,
          3,
        );
      case "midnight":
        return new KeyboardSkin(
          id,
          "霓虹夜航",
          "紫色星点 · 霓虹描边",
          dark,
          rgb(0.075, 0.06, 0.14),
          rgb(0.16, 0.12, 0.25),
          "#FFFFFF",
          rgb(0.78, 0.69, 1),
          rgb(0.4, 0.23, 0.7),
          10,
          1,
          0,
          3,
          2,
          false,
          1,
        );
      // The four source candidate skins, in their own colours rather than the nearest touch-keyboard
      // palette. Values come from packages/ui/src/upstream/candidate-themes/skins/<id>/horizontal_*
      // — the same stylesheets the Windows candidate window renders — so a skin looks like itself on
      // both hosts. These are candidate palettes only: they are not in BUILT_IN_IDS, because the
      // touch-keyboard picker is a different preference offering a different set.
      case "fluent":
        return new KeyboardSkin(
          id,
          "Fluent",
          "云母白 · 靛紫高亮",
          dark,
          adaptive(dark, "#FFFFFF", "#2D2D2D"),
          adaptive(dark, "#FFFFFF", "#414141"),
          adaptive(dark, "#1A1A1A", "#E9E8E8"),
          "#6B69D6",
          "#6B69D6",
          6,
          0.5,
          0,
          3,
          2,
          false,
          0,
        );
      case "wechat":
        return new KeyboardSkin(
          id,
          "微信绿",
          "浅灰面 · 微信绿高亮",
          dark,
          adaptive(dark, "#F7F7F7", "#151515"),
          adaptive(dark, "#FFFFFF", "#2A2A2A"),
          adaptive(dark, "#333333", "#B7B7B7"),
          "#07C160",
          "#07C160",
          6,
          0.5,
          0,
          3,
          2,
          false,
          0,
        );
      case "graphite":
        return new KeyboardSkin(
          id,
          "石墨",
          "石板灰 · 冷调高亮",
          dark,
          adaptive(dark, "#F1F3F5", "#1C1F23"),
          adaptive(dark, "#FBFBFC", "#23272C"),
          adaptive(dark, "#1A1A1A", "#E9E8E8"),
          adaptive(dark, "#5F6B7A", "#8993A0"),
          adaptive(dark, "#5F6B7A", "#8993A0"),
          6,
          0.5,
          0,
          3,
          2,
          false,
          0,
        );
      case "willow_green":
        return new KeyboardSkin(
          id,
          "杨柳青",
          "柳色面 · 柔绿高亮",
          dark,
          adaptive(dark, "#F4F5F3", "#343635"),
          adaptive(dark, "#FBFCFA", "#414441"),
          adaptive(dark, "#333333", "#B7B7B7"),
          adaptive(dark, "#58B980", "#65C98D"),
          adaptive(dark, "#58B980", "#65C98D"),
          9,
          0.5,
          0,
          3,
          2,
          false,
          0,
        );
      case "blueprint":
        return new KeyboardSkin(
          id,
          "工程蓝图",
          "蓝图网格 · 等宽字形",
          dark,
          rgb(0.055, 0.13, 0.22),
          rgb(0.09, 0.2, 0.32),
          "#FFFFFF",
          rgb(0.54, 0.84, 1),
          rgb(0.12, 0.34, 0.54),
          3,
          1,
          0,
          3,
          2,
          true,
          2,
        );
      default:
        return new KeyboardSkin(
          "forest",
          "水杉绿",
          "清新留白 · 经典圆角",
          dark,
          adaptive(dark, rgb(0.91, 0.94, 0.92), rgb(0.09, 0.13, 0.11)),
          adaptive(dark, "#FFFFFF", rgb(0.19, 0.24, 0.21)),
          label(dark),
          adaptive(dark, rgb(0.094, 0.36, 0.28), rgb(0.45, 0.8, 0.65)),
          adaptive(dark, rgb(0.094, 0.36, 0.28), rgb(0.12, 0.38, 0.29)),
          8,
          0,
          0,
          3,
          2,
          false,
          0,
        );
    }
  }

  static readonly BUILT_IN_IDS: string[] = [
    "forest",
    "ocean",
    "rose",
    "porcelain",
    "typewriter",
    "candy",
    "midnight",
    "blueprint",
  ];

  static builtIns(dark: boolean): KeyboardSkin[] {
    return KeyboardSkin.BUILT_IN_IDS.map((id: string) => KeyboardSkin.from(id, dark));
  }

  static choices(dark: boolean, design: CustomKeyboardSkin | null): KeyboardSkin[] {
    const skins: KeyboardSkin[] = KeyboardSkin.builtIns(dark);
    skins.push(KeyboardSkin.from("custom", dark, design));
    return skins;
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
  keySurfaceBackground(emphasized: boolean): string {
    return emphasized ? this.actionBackground : alpha(this.keyBackground, this.keyOpacity);
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
