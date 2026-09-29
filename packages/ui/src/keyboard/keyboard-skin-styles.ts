import type { CSSProperties } from "react";
import { clamp } from "../core/number";
import { keyboardThemeLook } from "./screen-keyboard-preview";
import { keyboardBackgroundStyle } from "./keyboard-background";
import { keyboardRgba, mixKeyboardColor, readableKeyboardText } from "./keyboard-colors";
import { skinColor, type TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";
import type { GlobalTheme } from "../theme/global-theme";

export function keyboardSkinStyles(
  theme: "dark" | "light",
  skin: GlobalTheme,
  customDesign?: TouchKeyboardSkinDesign,
): CSSProperties {
  const custom = skin === "custom" && customDesign ? customDesign : undefined;
  const option = keyboardThemeLook(skin, theme);
  const palette = custom
    ? {
        background: skinColor(custom.background),
        key: skinColor(custom.keyBackground),
        foreground: skinColor(custom.keyForeground),
        accent: skinColor(custom.accent),
        action: skinColor(custom.actionBackground),
      }
    : option.palette;
  const radius = custom?.cornerRadius ?? option.cornerRadius;
  const borderWidth = custom?.borderWidth ?? option.borderWidth;
  const shadowOpacity = custom?.shadow ?? option.shadowOpacity;
  const shadowOffset = custom ? 1 : option.shadowOffset;
  const shadowRadius = custom ? 2 : option.shadowRadius;
  const keyOpacity = custom?.keyOpacity ?? 1;
  const keyShape = custom?.keyShape ?? "rounded";
  const keyMaterial = custom?.keyMaterial ?? "flat";
  const keyRadius =
    keyShape === "capsule"
      ? "999px"
      : keyShape === "pebble"
        ? "42% 58% 48% 52% / 52% 44% 56% 48%"
        : keyShape === "ticket"
          ? `${clamp(radius, 0, 4)}px`
          : `${clamp(radius, 0, 20)}px`;
  const fontFamily =
    (custom?.monospaced ?? option.monospaced)
      ? "ui-monospace, SFMono-Regular, Consolas, monospace"
      : "inherit";
  return {
    ...keyboardBackgroundStyle(theme, skin, customDesign, palette),
    "--kb-background": palette.background,
    "--kb-text": palette.foreground,
    "--kb-heading": palette.accent,
    "--kb-key": palette.key,
    "--kb-key-fill": keyboardRgba(palette.key, keyOpacity),
    "--kb-action": palette.action,
    "--kb-action-fill": keyboardRgba(palette.action, 1),
    "--kb-action-text": readableKeyboardText(palette.action),
    "--kb-paper-line": keyboardRgba(palette.accent, 0.08),
    "--kb-hover": mixKeyboardColor(palette.key, palette.accent, 0.18),
    "--kb-active": mixKeyboardColor(palette.key, palette.accent, 0.3),
    "--kb-pressed": mixKeyboardColor(palette.key, palette.accent, 0.42),
    "--kb-key-radius": keyRadius,
    "--kb-border-width": `${clamp(borderWidth, 0, 2)}px`,
    "--kb-border-color": palette.accent,
    "--kb-shadow":
      shadowOpacity > 0
        ? `0 ${shadowOffset}px ${shadowRadius}px rgba(0, 0, 0, ${shadowOpacity})`
        : "none",
    "--kb-font-family": fontFamily,
    "--kb-key-material": keyMaterial,
  } as CSSProperties;
}
