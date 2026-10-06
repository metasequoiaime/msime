import { useEffect, useMemo, useState } from "react";
import {
  defaultCandidateFallbackFonts,
  defaultCandidateFontFamily,
  validFontFamily,
  type CandidateFontPreferences,
} from "./candidate-font-family";
import { useAsyncGeneration } from "../settings/use-async-generation";

export type FontFamilyResolver = (names: string[]) => Promise<string[]>;

// Resolution affects presentation only. Never write aliases back to preferences.
export function useResolvedCandidateFonts<T extends CandidateFontPreferences>(
  preferences: T,
  resolve?: FontFamilyResolver,
): T {
  const names = [
    preferences.candidate_english_font ??
      preferences.candidate_font_family ??
      defaultCandidateFontFamily,
    ...(preferences.candidate_english_font == null
      ? []
      : [preferences.candidate_font_family ?? defaultCandidateFontFamily]),
    ...(preferences.candidate_fallback_fonts ?? defaultCandidateFallbackFonts),
  ];
  const encoded = JSON.stringify(names);
  const request = useMemo(
    () => ({ names: JSON.parse(encoded) as string[], resolve }),
    [encoded, resolve],
  );
  const [result, setResult] = useState<{ request: typeof request; names: string[] }>();
  const generation = useAsyncGeneration(request);
  useEffect(() => {
    const requestGeneration = generation.current;
    if (request.resolve && request.names.length <= 33 && request.names.every(validFontFamily)) {
      void (async () => {
        try {
          const resolved: unknown = await request.resolve!(request.names);
          if (
            generation.current === requestGeneration &&
            Array.isArray(resolved) &&
            resolved.length === request.names.length &&
            resolved.every((name) => validFontFamily(name) && !/[\x00-\x1f\x7f]/.test(name))
          ) {
            setResult({ request, names: resolved });
          }
        } catch {
          /* Unavailable fonts keep literal names. */
        }
      })();
    }
  }, [request, generation]);
  if (result?.request !== request) return preferences;
  return {
    ...preferences,
    ...(preferences.candidate_english_font == null
      ? { candidate_font_family: result.names[0], candidate_fallback_fonts: result.names.slice(1) }
      : {
          candidate_english_font: result.names[0],
          candidate_fallback_fonts: result.names.slice(2),
        }),
  };
}
