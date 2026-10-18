import { themeEntry, type GlobalTheme } from "../theme/global-theme";
import {
  readableSkinText,
  skinColor,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";

type Palette = {
  background: string;
  key: string;
  foreground: string;
  accent: string;
  action: string;
  actionForeground: string;
};

/** How a global theme draws a touch keyboard when no custom design applies. */
export type KeyboardThemeLook = {
  palette: Palette;
  cornerRadius: number;
  borderWidth: number;
  shadowOpacity: number;
  shadowRadius: number;
  shadowOffset: number;
  monospaced: boolean;
  pattern: 0 | 1 | 2 | 3;
};

// `system` is the platform's own keyboard, which the page cannot read; it previews the design's iOS system keyboard tokens (dc.html `kbBg` / `keyBg` / `keySpec` / `keyFg` and the brand accent).
const systemKeyboard: Record<"dark" | "light", Palette> = {
  light: {
    background: "#D1D4DB",
    key: "#FFFFFF",
    foreground: "#000000",
    accent: "#2C7A4B",
    action: "#ABB0BB",
    actionForeground: "#000000",
  },
  dark: {
    background: "#2B2B2D",
    key: "#6B6B6E",
    foreground: "#FFFFFF",
    accent: "#5FBF84",
    action: "#464648",
    actionForeground: "#FFFFFF",
  },
};

// `native`（原生，只在 iOS 提供）与 iOS 的 `SystemKeyboardTokens` 同值：灰白键面、柔和水杉绿强调色、圆角 6，不跟季节。
const nativeKeyboard: Record<"dark" | "light", Palette> = {
  light: {
    background: "#E2E3E8",
    key: "#FFFFFF",
    foreground: "#000000",
    accent: "#32664B",
    action: "#32664B",
    actionForeground: "#FFFFFF",
  },
  dark: {
    background: "#222223",
    key: "#464646",
    foreground: "#FFFFFF",
    accent: "#8AC4A2",
    action: "#8AC4A2",
    actionForeground: "#000000",
  },
};

/** 全局主题的键盘外观：内置主题用目录里的调色板（功能键画 `function_key`）；`native` 用系统键盘的取值；`system`、没有设计的 `custom` 和不认识的 id 用系统预览。 */
export function keyboardThemeLook(skin: string, theme: "dark" | "light"): KeyboardThemeLook {
  const entry = themeEntry(skin);
  const keyboard = entry.keyboard;
  const native = entry.id === "native";
  return {
    palette: keyboard
      ? {
          background: keyboard.background,
          key: keyboard.key,
          foreground: keyboard.text,
          accent: keyboard.accent,
          action: keyboard.function_key,
          actionForeground: keyboard.text,
        }
      : native
        ? nativeKeyboard[theme]
        : systemKeyboard[theme],
    cornerRadius: native ? 6 : 8,
    borderWidth: 0,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  };
}

/** One resolved look for the live keyboard surface and its SVG preview. */
export function resolveKeyboardSkinLook(
  theme: "dark" | "light",
  skin: GlobalTheme,
  customDesign?: TouchKeyboardSkinDesign,
) {
  const custom = skin === "custom" ? customDesign : undefined;
  const option = keyboardThemeLook(skin, theme);
  return {
    custom,
    palette: custom
      ? {
          background: skinColor(custom.background),
          key: skinColor(custom.keyBackground),
          foreground: skinColor(custom.keyForeground),
          accent: skinColor(custom.accent),
          action: skinColor(custom.actionBackground),
          actionForeground: skinColor(readableSkinText(custom.actionBackground)),
        }
      : option.palette,
    cornerRadius: custom?.cornerRadius ?? option.cornerRadius,
    borderWidth: custom?.borderWidth ?? option.borderWidth,
    shadowOpacity: custom?.shadow ?? option.shadowOpacity,
    shadowRadius: custom ? 2 : option.shadowRadius,
    shadowOffset: custom ? 1 : option.shadowOffset,
    monospaced: custom?.monospaced ?? option.monospaced,
    pattern: custom?.pattern ?? option.pattern,
    keyShape: custom?.keyShape ?? "rounded",
    keyMaterial: custom?.keyMaterial ?? "flat",
    keyOpacity: custom?.keyOpacity ?? 1,
  };
}

export type ResolvedKeyboardSkinLook = ReturnType<typeof resolveKeyboardSkinLook>;
