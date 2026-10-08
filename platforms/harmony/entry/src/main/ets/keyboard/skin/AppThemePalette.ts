/**
 * 应用主题（水杉四季 / 春芽 / 夏荫 / 秋杉 / 冬雪）在键盘上绘制时用到的颜色，按设计稿的 `color-mix` 公式（全平台 UI.dc.html 中的 `kbBase` 和 `logoBg` / `logoCirc`）从季节种子色推导而来。
 *
 * 种子色本身来自 `msime_client_resolve_app_theme`（crates/client-core/src/skin/app_theme.rs），季节规则和种子表都归它管，这里不保存任何一份副本。本文件负责的是设计稿在种子色之上计算出的展示层：`system` 键盘的背景、字母键和功能键，以及两种 logo 颜色。它移植自 platforms/android/java/app/msime/android/settings/AppThemePalette.java，两个宿主的取整方式一致，得出的颜色也一致。
 *
 * 这里所有颜色都是 ArkUI 顺序（`#RRGGBB`，或 alpha 在前的 `#AARRGGBB`），与经过 GlobalTheme 解析之后的其他颜色相同。
 */
import { GlobalTheme, KeyboardThemePalette } from "./GlobalTheme";

/** 解析后的应用主题的一种模式，即 `msime_client_resolve_app_theme` 的返回值，所有颜色都已转换成 ArkUI 顺序。 */
export interface AppThemeSeed {
  readonly accent: string;
  /** alpha 为 0x22（浅色）或 0x40（深色）的强调色，由共享层计算。 */
  readonly accent_soft: string;
  /** 填充了 `accent` 的元素上的文字和图标颜色。 */
  readonly on_accent: string;
  /** 页面背景（设计稿中季节的 `bg`）。 */
  readonly background: string;
  /** 行和卡片背景（应用季节后设计稿中鸿蒙的 `groupBg`）。 */
  readonly card: string;
}

interface Envelope {
  ok?: boolean;
  value?: Object;
}

/** 参与混合的颜色通道数：alpha、红、绿、蓝。 */
const CHANNELS: number = 4;
const WHITE: string = "#FFFFFF";
const BLACK: string = "#000000";
const DARK_KEYBOARD_BASE: string = "#161716";
const DARK_KEY_BASE: string = "#3A3C3A";
const DARK_FUNCTION_BASE: string = "#262826";
/** 共享层的 DARK_ON_ACCENT_ACCENT_PERCENT：深色模式的 on-accent 与黑色混合时保留的强调色比例。 */
const DARK_ON_ACCENT_ACCENT_PERCENT: number = 25;

function field(value: Object, name: string): Object | null | undefined {
  return (value as Record<string, Object | null | undefined>)[name];
}

function colour(value: Object, name: string): string | null {
  const member: Object | null | undefined = field(value, name);
  return typeof member === "string" ? GlobalTheme.arkColor(member as string) : null;
}

/** ArkUI 颜色的 alpha、红、绿、蓝分量，各为 0-255。`#RRGGBB` 视为不透明。 */
function channels(value: string): number[] {
  if (/^#[0-9A-Fa-f]{6}$/.test(value)) {
    return [
      255,
      parseInt(value.substring(1, 3), 16),
      parseInt(value.substring(3, 5), 16),
      parseInt(value.substring(5, 7), 16),
    ];
  }
  if (/^#[0-9A-Fa-f]{8}$/.test(value)) {
    return [
      parseInt(value.substring(1, 3), 16),
      parseInt(value.substring(3, 5), 16),
      parseInt(value.substring(5, 7), 16),
      parseInt(value.substring(7, 9), 16),
    ];
  }
  throw new Error(`Not an ArkUI colour: ${value}`);
}

function hex(value: number): string {
  const text: string = value.toString(16).toUpperCase();
  return text.length === 1 ? "0" + text : text;
}

/**
 * 按设计稿 token 表的生成方式对浮点通道取整：那些十六进制值读自 Chrome 的 `getComputedStyle`，它先把每个 0-1 通道打印为六位有效数字，再放大回 0-255。直接对原始值取整，会让恰好落在 .5 的通道（#FDF9F6 中的 249.5）差一。
 */
function quantize(value: number): number {
  const printed: number = Number((value / 255).toPrecision(6));
  return Math.max(0, Math.min(255, Math.round(printed * 255)));
}

export class AppThemePalette {
  /** 键盘文字颜色，用的是鸿蒙的文字 token 而不是季节颜色。 */
  static readonly KEYBOARD_TEXT_LIGHT: string = "#182431";
  static readonly KEYBOARD_TEXT_DARK: string = "#E5E5E5";
  /** 提示文字、空格键上的方案名和工具栏图标（设计稿中的 kbSub）。 */
  static readonly KEYBOARD_SECONDARY_LIGHT: string = "#5A6B5D";
  static readonly KEYBOARD_SECONDARY_DARK: string = "#93A596";

  /**
   * CSS `color-mix(in srgb, first weight%, second)`：包括 alpha 在内的每个通道按 `weight` 和 `100 - weight` 线性混合，再按 `quantize` 所述取整。不透明时返回 `#RRGGBB`，否则返回 `#AARRGGBB`。
   */
  static mix(first: string, weight: number, second: string): string {
    if (!Number.isFinite(weight) || weight < 0 || weight > 100) {
      throw new Error(`Mix weight out of range: ${weight}`);
    }
    const a: number[] = channels(first);
    const b: number[] = channels(second);
    const mixed: number[] = [];
    for (let index = 0; index < CHANNELS; index++) {
      mixed.push(quantize((a[index] * weight + b[index] * (100 - weight)) / 100));
    }
    const rgb: string = hex(mixed[1]) + hex(mixed[2]) + hex(mixed[3]);
    return mixed[0] === 255 ? "#" + rgb : "#" + hex(mixed[0]) + rgb;
  }

  /**
   * 该季节和模式下的 `system` 键盘（开启应用主题时设计稿中的 `kbBase`）：浅色 bg = mix(accent 12%, bg)，key = mix(bg 25%, #FFF)，function = mix(accent 24%, bg)；深色 bg = mix(accent 10%, #161716)，key = mix(accent 10%, #3A3C3A)，function = mix(accent 14%, #262826)。文字和提示用鸿蒙 token，强调色及其上的文字用季节颜色。
   */
  static keyboard(seed: AppThemeSeed, dark: boolean): KeyboardThemePalette {
    return {
      background: dark
        ? AppThemePalette.mix(seed.accent, 10, DARK_KEYBOARD_BASE)
        : AppThemePalette.mix(seed.accent, 12, seed.background),
      key: dark
        ? AppThemePalette.mix(seed.accent, 10, DARK_KEY_BASE)
        : AppThemePalette.mix(seed.background, 25, WHITE),
      function_key: dark
        ? AppThemePalette.mix(seed.accent, 14, DARK_FUNCTION_BASE)
        : AppThemePalette.mix(seed.accent, 24, seed.background),
      text: dark ? AppThemePalette.KEYBOARD_TEXT_DARK : AppThemePalette.KEYBOARD_TEXT_LIGHT,
      secondary: dark
        ? AppThemePalette.KEYBOARD_SECONDARY_DARK
        : AppThemePalette.KEYBOARD_SECONDARY_LIGHT,
      accent: seed.accent,
      on_accent: seed.on_accent,
    };
  }

  /**
   * 填充了 `accent` 的元素上的文字和图标颜色，遵循共享层的规则（crates/client-core/src/skin/app_theme.rs 的 `colors`）：浅色模式为白色，深色模式为强调色与黑色按 25% 混合，因为深色模式下的浅强调色上放白色达不到对比度要求（在 #5FBF84 上约 2.3:1）。解析出的种子已经以 `on_accent` 带上这个值；这里用于没有解析出季节时的平台强调色。
   */
  static onAccent(accent: string, dark: boolean): string {
    return dark ? AppThemePalette.mix(accent, DARK_ON_ACCENT_ACCENT_PERCENT, BLACK) : WHITE;
  }

  /** logo 标志的填充色（设计稿中的 logoBg）：强调色与黑色按 82% 混合。 */
  static logoBackground(accent: string): string {
    return AppThemePalette.mix(accent, 82, BLACK);
  }

  /** logo 标志背后的圆盘（设计稿中的 logoCirc）：强调色按 14%（浅色）或 22%（深色）混入卡片颜色。 */
  static logoDisc(accent: string, card: string, dark: boolean): string {
    return AppThemePalette.mix(accent, dark ? 22 : 14, card);
  }

  /**
   * 读取 `msime_client_resolve_app_theme` 的响应（`{ok, value: {id, season, accent, accent_soft, on_accent, background, card, hair}}`）。被拒绝，或任一颜色缺失、无法读取时返回 `null`，键盘保持基础 token。响应是共享层自己产出的 JSON，所以与 GlobalTheme.parseResolved 一样，这里不对解析本身做防护。
   */
  static parseResolved(response: string): AppThemeSeed | null {
    const envelope: Envelope = JSON.parse(response) as Envelope;
    const value: Object | undefined = envelope.value;
    if (
      envelope.ok !== true ||
      value === undefined ||
      value === null ||
      typeof value !== "object" ||
      Array.isArray(value)
    ) {
      return null;
    }
    const accent: string | null = colour(value, "accent");
    const accentSoft: string | null = colour(value, "accent_soft");
    const onAccent: string | null = colour(value, "on_accent");
    const background: string | null = colour(value, "background");
    const card: string | null = colour(value, "card");
    if (
      accent === null ||
      accentSoft === null ||
      onAccent === null ||
      background === null ||
      card === null
    ) {
      return null;
    }
    return {
      accent: accent,
      accent_soft: accentSoft,
      on_accent: onAccent,
      background: background,
      card: card,
    };
  }
}
