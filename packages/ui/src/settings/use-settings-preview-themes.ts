import { useEffect, type Dispatch, type SetStateAction } from "react";
import type { Preferences } from "../index";
import { useCandidatePreviewTheme } from "../candidate/candidate-preview-theme";
import type { SurfaceTheme, ThemeMode } from "./theme-settings-section";

export type SettingsSkinPreviewThemes = Partial<
  Record<NonNullable<Preferences["candidate_skin"]>, "light" | "dark">
>;

export interface UseSettingsPreviewThemesOptions {
  themeMode: ThemeMode;
  candidateTheme?: SurfaceTheme;
  toolbarTheme?: SurfaceTheme;
  screenKeyboardTheme?: SurfaceTheme;
  setSkinPreviewThemes: Dispatch<SetStateAction<SettingsSkinPreviewThemes>>;
}

/** Resolves settings surface preview themes and resets skin previews when the candidate theme changes. */
export function useSettingsPreviewThemes({
  themeMode,
  candidateTheme,
  toolbarTheme,
  screenKeyboardTheme,
  setSkinPreviewThemes,
}: UseSettingsPreviewThemesOptions) {
  const candidatePreviewTheme = useCandidatePreviewTheme(themeMode, candidateTheme);
  const toolbarPreviewTheme = useCandidatePreviewTheme(themeMode, toolbarTheme);
  const keyboardPreviewTheme = useCandidatePreviewTheme(themeMode, screenKeyboardTheme);

  useEffect(() => setSkinPreviewThemes({}), [candidatePreviewTheme, setSkinPreviewThemes]);

  return { candidatePreviewTheme, toolbarPreviewTheme, keyboardPreviewTheme } as const;
}
