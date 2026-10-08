/**
 * Rendering values for the touch keyboard, drawn from the shared global theme.
 *
 * 皮肤有两种构建方式。主题的键盘配色（主题没有键盘配色时，用季节配色或 Harmony 原生 token）给出该主题颜色的扁平键盘。用户的自定义键盘设计给出编辑器能设置的一切，照片、渐变、键形和材质都包括在内，因为本宿主绘制完整设计，而不是扁平化后的配色。所有颜色都在这里推导，而不在绘制代码里，这样键盘、预览和选择卡片拿到的是同一组值。
 *
 * 已解析出应用主题的季节种子时，设计稿中用到平台强调色的地方都改用它：回车键、`system` 和自定义设计的开启态图块，以及 logo。没有种子时，这些都回落到 Harmony 基础 token。
 */
import { AppThemePalette, AppThemeSeed } from "./AppThemePalette";
import { CustomKeyboardSkin } from "./CustomKeyboardSkin";
import { GlobalTheme, KeyboardThemePalette } from "./GlobalTheme";
import { KeyboardGeometry } from "../KeyboardGeometry";

function clampChannel(value: number): number {
  return Math.round(KeyboardGeometry.bounded(value, 0, 1) * 255);
}

/** Produces #AARRGGBB: the alpha byte is prefixed to the #RRGGBB part of a colour, replacing any alpha it had. */
function alpha(colour: string, value: number): string {
  return (
    "#" +
    clampChannel(value).toString(16).toUpperCase().padStart(2, "0") +
    colour.substring(colour.length - 6)
  );
}

/** The design's key radius, which a theme keyboard draws whatever its colours. */
const THEME_KEY_RADIUS: number = 8;

/** 键盘细线（设计稿的 `kbHair`）：分隔线和未选中的分页圆点。 */
const HAIR_LIGHT: string = "#1F000000";
const HAIR_DARK: string = "#24FFFFFF";
/** 无论明暗模式键盘都是深色的命名主题，它们的细线用深底上的浅色那一种。 */
const DARK_SURFACE_THEMES: string[] = ["shuishan", "night", "ink"];
/** 没有季节时 logo 圆盘混入的卡片底色：设计稿的 Harmony `groupBg`。 */
const BASE_CARD_LIGHT: string = "#FFFFFF";
const BASE_CARD_DARK: string = "#1F1F1F";
/** 命名主题的开启态底色：按设计稿 tonal container 的比例取它自己的强调色。 */
const NAMED_TOGGLE_ALPHA_LIGHT: number = 0.13;
const NAMED_TOGGLE_ALPHA_DARK: number = 0.25;

export class KeyboardSkin {
  /** The global theme id this skin draws: `system`, a built-in id, or `custom`. */
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly dark: boolean;
  readonly background: string;
  readonly keyBackground: string;
  readonly keyForeground: string;
  /** Shift, delete, the symbol and number toggles: a theme's `function_key`; a keyboard design draws them on its letter-key face. */
  readonly functionKeyBackground: string;
  readonly functionKeyForeground: string;
  /** Hints, spellings and the space-bar label. */
  readonly secondary: string;
  /** The selected candidate in the strip, drawn as text with no fill. */
  readonly accent: string;
  /** Text on anything filled with `accent`. */
  readonly onAccent: string;
  /** 回车键。主题：已解析出种子时用季节强调色及其配套文字色，否则用平台强调色，文字色按共享的 on-accent 规则（浅色模式为白色，深色模式为强调色混入 25% 黑色），与主题无关。用户的键盘设计用它自己的 `actionBackground`，文字按亮度取黑或白，与 Android 的 `returnBackground` 和 iOS 的 `SkinKeySurfaceView` 一致：季节只给应用和 `system` 键盘着色，不改用户自己设计的键。 */
  readonly actionBackground: string;
  readonly actionForeground: string;
  /**
   * 开启态图块，例如面板打开时的 logo 按钮。`system` 和自定义设计用季节强调色的底色（没有种子时用平台强调色的），图形用强调色本身。命名主题用自己的强调色，浅色 13%、深色 25%，图形用其强调色，与 Android 的 `toolbarActiveBackground` 一致，而不用设计稿写死的绿色底，那种绿色和夜青、墨都不搭。
   */
  readonly toggleBackground: string;
  readonly toggleForeground: string;
  /** 键盘里的分隔线和未选中的分页圆点（设计稿的 `kbHair`）。 */
  readonly hair: string;
  /** logo 标志的填充色（设计稿的 `logoBg`）：季节或平台强调色混入 82% 黑色。 */
  readonly logoBg: string;
  /** logo 标志背后的圆盘（设计稿的 `logoCirc`）：把该强调色按 14% / 22% 混入季节卡片或基础卡片底色。 */
  readonly logoCirc: string;
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
   * 主题皮肤把 `design` 传为 null，取扁平默认值；自定义设计传入用户的设计，所有值都取自它。只用一个构造函数而不是两个，就不会有字段在一条路径上设置了、在另一条上漏掉。`named` 标记按自身配色绘制的内置主题，其开启态底色是它自己的强调色；`seed` 是已解析的应用主题，或 `null`。
   */
  private constructor(
    id: string,
    title: string,
    dark: boolean,
    palette: KeyboardThemePalette,
    design: CustomKeyboardSkin | null,
    named: boolean,
    seed: AppThemeSeed | null,
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
    const platformAccent: string = seed === null ? GlobalTheme.accent(dark) : seed.accent;
    if (design !== null) {
      this.actionBackground = design.actionBackground();
      this.actionForeground = design.actionForeground();
    } else {
      this.actionBackground = platformAccent;
      this.actionForeground =
        seed === null ? AppThemePalette.onAccent(platformAccent, dark) : seed.on_accent;
    }
    if (named) {
      this.toggleBackground = alpha(
        palette.accent,
        dark ? NAMED_TOGGLE_ALPHA_DARK : NAMED_TOGGLE_ALPHA_LIGHT,
      );
      this.toggleForeground = palette.accent;
    } else {
      this.toggleBackground = seed === null ? GlobalTheme.accentSoft(dark) : seed.accent_soft;
      this.toggleForeground = platformAccent;
    }
    this.hair = dark || DARK_SURFACE_THEMES.includes(id) ? HAIR_DARK : HAIR_LIGHT;
    this.logoBg = AppThemePalette.logoBackground(platformAccent);
    this.logoCirc = AppThemePalette.logoDisc(
      platformAccent,
      seed === null ? (dark ? BASE_CARD_DARK : BASE_CARD_LIGHT) : seed.card,
      dark,
    );
    // 同一个 id 下季节会改变回车键和各处底色，所以季节也是身份的一部分。
    const seedKey: string =
      seed === null
        ? ""
        : [seed.accent, seed.accent_soft, seed.on_accent, seed.background, seed.card].join(",");
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
      this.designKey =
        [
          palette.background,
          palette.key,
          palette.function_key,
          palette.text,
          palette.secondary,
          palette.accent,
        ].join(",") + (seedKey.length === 0 ? "" : ";" + seedKey);
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
      this.designKey = design.key() + (seedKey.length === 0 ? "" : ";" + seedKey);
    }
  }

  /**
   * 主题的键盘。`system`，以及没有设计、基于 `system` 的自定义主题，都解析为 `null` 配色：给了种子时用季节配色（`AppThemePalette.keyboard`），否则用给定模式下的 Harmony 原生 token。非 null 配色是命名主题，颜色和开启态底色都是它自己的；此时种子只重新着色回车键和 logo。
   *
   * `seed` 是与 `dark` 同一模式下已解析的应用主题，尚未解析时为 `null`。
   */
  static fromTheme(
    id: string,
    title: string,
    palette: KeyboardThemePalette | null,
    dark: boolean,
    seed: AppThemeSeed | null = null,
  ): KeyboardSkin {
    if (palette !== null) {
      return new KeyboardSkin(id, title, dark, palette, null, true, seed);
    }
    const base: KeyboardThemePalette =
      seed === null ? GlobalTheme.nativeKeyboard(dark) : AppThemePalette.keyboard(seed, dark);
    return new KeyboardSkin(id, title, dark, base, null, false, seed);
  }

  /**
   * 用户的键盘设计，完整绘制。功能键（⇧、⌫、123、中/英）画在字母键的底色上、用字母键的文字色，设计的 `actionBackground` 只给回车键，与 Android 的 `functionBackground` 和 iOS 的 `KeyboardTheme.functionKeyBackground` 一致。用动作色铺满所有功能键会让社区皮肤像拼布，而且深色动作色配深色文字的设计（比如默认的薄荷晨光）上，功能键的字几乎看不见。提示文字是设计文字色的 60%。
   */
  static fromDesign(
    title: string,
    design: CustomKeyboardSkin,
    dark: boolean,
    seed: AppThemeSeed | null = null,
  ): KeyboardSkin {
    const palette: KeyboardThemePalette = {
      background: design.background(),
      key: design.keyBackground(),
      function_key: design.keyBackground(),
      text: design.keyForeground(),
      secondary: alpha(design.keyForeground(), 0.6),
      accent: design.accent(),
      on_accent: design.accentForeground(),
    };
    const skin: KeyboardSkin = new KeyboardSkin(
      "custom",
      title,
      dark,
      palette,
      design,
      false,
      seed,
    );
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
   * 半透明的按键背景，用于叠在键盘之上、而不是与按键并列的表面：候选条和九键侧栏。
   *
   * 透明度属于颜色而不属于视图。在容器上设置 opacity 会连带淡化其中所有内容，曾把候选条的图标和方案胶囊淡成一团模糊。
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
  /** 按键的底色：强调的键用回车键颜色（`actionBackground`），特殊键（⇧、⌫、123、语言键、空闲的回车）用功能键颜色，其余用字母键颜色。 */
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
