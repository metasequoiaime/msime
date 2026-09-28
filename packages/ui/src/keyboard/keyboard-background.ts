import type { CSSProperties } from "react";
import { clamp } from "../core/number";
import { touchKeyboardSkinOption, type TouchKeyboardSkin } from "./screen-keyboard-preview";
import { keyboardRgba } from "./keyboard-colors";
import { skinColor, type TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";

export function keyboardBackgroundStyle(
  skin: TouchKeyboardSkin,
  customDesign: TouchKeyboardSkinDesign | undefined,
  palette: { background: string; accent: string },
): CSSProperties {
  const custom = skin === "custom" && customDesign ? customDesign : undefined;
  const option = touchKeyboardSkinOption(skin);
  const pattern = custom?.pattern ?? option.pattern;
  const patternOpacity = custom?.patternOpacity ?? 0.15;
  const images: string[] = [];
  const sizes: string[] = [];
  const positions: string[] = [];
  const add = (image: string, size: string, position = "0 0") => {
    images.push(image);
    sizes.push(size);
    positions.push(position);
  };
  const accent = keyboardRgba(palette.accent, patternOpacity);
  if (pattern === 1) add(`radial-gradient(circle, ${accent} 1px, transparent 1.5px)`, "16px 16px");
  else if (pattern === 2)
    add(
      `linear-gradient(${accent} 1px, transparent 1px), linear-gradient(90deg, ${accent} 1px, transparent 1px)`,
      "20px 20px",
    );
  else if (pattern === 3)
    add(
      `repeating-linear-gradient(155deg, transparent 0 18px, ${accent} 18px 20px, transparent 20px 38px)`,
      "48px 48px",
    );
  if (custom?.gradientEnd !== undefined) {
    const direction = custom.gradientHorizontal ? "90deg" : "180deg";
    add(
      `linear-gradient(${direction}, ${palette.background}, ${skinColor(custom.gradientEnd)})`,
      "cover",
    );
  }
  const photo =
    typeof custom?.photo === "string" &&
    /^[A-Za-z0-9+/=]+$/.test(custom.photo) &&
    custom.photo.length <= 682668
      ? custom.photo
      : undefined;
  if (photo) {
    const shade = clamp(custom?.photoShade ?? 0.25, 0, 0.8);
    const position = clamp(custom?.photoPosition ?? 0.5, 0, 1) * 100;
    add(`url("data:image/jpeg;base64,${photo}")`, "cover", `${position}% ${position}%`);
    add(`linear-gradient(rgba(0, 0, 0, ${shade}), rgba(0, 0, 0, ${shade}))`, "cover");
  }
  return images.length
    ? {
        backgroundImage: images.reverse().join(", "),
        backgroundSize: sizes.reverse().join(", "),
        backgroundPosition: positions.reverse().join(", "),
      }
    : {};
}
