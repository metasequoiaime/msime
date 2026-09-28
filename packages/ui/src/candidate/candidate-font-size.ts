import type { CSSProperties } from "react";

// Shared preferences and pinned Windows appearance controls both use 12–32.
export const candidateFontSizes = Array.from({ length: 21 }, (_, index) => index + 12);
function candidateFontSizeValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isInteger(value) && value >= 12 && value <= 32
    ? value
    : fallback;
}
export function candidateFontSize(value: unknown): number {
  return candidateFontSizeValue(value, 18);
}
export function candidatePreeditFontSize(value: unknown): number {
  return candidateFontSizeValue(value, 15);
}
export function candidateFontStyle(preferences: {
  candidate_font_size?: number;
  candidate_preedit_font_size?: number;
}): CSSProperties {
  return {
    "--appearance-font-size": `${candidateFontSize(preferences.candidate_font_size)}px`,
    "--appearance-preedit-font-size": `${candidatePreeditFontSize(preferences.candidate_preedit_font_size)}px`,
  } as CSSProperties;
}
