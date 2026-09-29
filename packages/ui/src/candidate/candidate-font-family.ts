import type { CSSProperties } from "react";
import { utf8ByteLength } from "../core/text";

export type CandidateFontPreferences = {
  candidate_font_family?: string;
  candidate_english_font?: string | null;
  candidate_fallback_fonts?: string[];
};
export const defaultCandidateEnglishFont = "Segoe UI";
export const defaultCandidateFontFamily = "Noto Sans SC";
export const defaultCandidateFallbackFonts = ["Noto Sans SC", "Microsoft YaHei"] as const;
export function validFontFamily(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    utf8ByteLength(value) <= 128 &&
    !/[\x00-\x1f\x7f-\x9f]/.test(value)
  );
}
export function validCandidateFonts(value: CandidateFontPreferences): boolean {
  return (
    validFontFamily(value.candidate_font_family ?? defaultCandidateFontFamily) &&
    (value.candidate_english_font == null ||
      (validFontFamily(value.candidate_english_font) &&
        !/[\x00-\x1f\x7f-\x9f]/.test(value.candidate_english_font))) &&
    (value.candidate_fallback_fonts?.length ?? defaultCandidateFallbackFonts.length) <= 32 &&
    (value.candidate_fallback_fonts ?? defaultCandidateFallbackFonts).every(validFontFamily)
  );
}
export function quoteFontFamily(value: string): string {
  return (
    '"' +
    value.replace(/["\\\x00-\x1f\x7f]/g, (char) => "\\" + char.charCodeAt(0).toString(16) + " ") +
    '"'
  );
}
export function candidateFamilyStyle(value: CandidateFontPreferences): CSSProperties {
  const primary = validFontFamily(value.candidate_font_family)
    ? value.candidate_font_family
    : defaultCandidateFontFamily;
  const leading = validFontFamily(value.candidate_english_font)
    ? [value.candidate_english_font]
    : [];
  const families = [
    ...leading,
    primary,
    ...(value.candidate_fallback_fonts ?? defaultCandidateFallbackFonts)
      .slice(0, 32)
      .filter(validFontFamily),
  ];
  return {
    "--appearance-font-family": families.map(quoteFontFamily).join(", ") + ", sans-serif",
  } as CSSProperties;
}
