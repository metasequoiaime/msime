import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import type { HostPlatform } from "../core/host-capabilities";
import type { Preferences } from "../core/preferences-model";
import { useCandidatePreviewTheme } from "../candidate/candidate-preview-theme";
import {
  hostDrawnCustomTheme,
  settingsThemePreferences,
  type SettingsThemePreferencesSource,
} from "./settings-theme-preferences";
import type { GlobalTheme } from "../theme/global-theme";

export type SettingsThemePreviewSource = SettingsThemePreferencesSource &
  Pick<Preferences, "candidate_theme" | "toolbar_theme" | "screen_keyboard_theme">;

export interface UseSettingsThemePreviewsOptions {
  draft?: SettingsThemePreviewSource;
  platform?: HostPlatform;
  surfaceThemesFollow?: boolean;
  showCandidateNumberColor: boolean;
  showCandidateBorderColor: boolean;
}

export interface SettingsThemePreviews {
  themeMode: ReturnType<typeof settingsThemePreferences>["themeMode"];
  settingsTheme: ReturnType<typeof settingsThemePreferences>["settingsTheme"];
  globalTheme: GlobalTheme;
  customColors: ReturnType<typeof settingsThemePreferences>["customColors"];
  customTouchKeyboardSkin: ReturnType<typeof settingsThemePreferences>["customTouchKeyboardSkin"];
  skinPreviewThemes: Partial<Record<GlobalTheme, "light" | "dark">>;
  setSkinPreviewThemes: Dispatch<SetStateAction<Partial<Record<GlobalTheme, "light" | "dark">>>>;
  candidatePreviewTheme: "dark" | "light";
  toolbarPreviewTheme: "dark" | "light";
  keyboardPreviewTheme: "dark" | "light";
  previewCustomTheme: ReturnType<typeof hostDrawnCustomTheme>;
  customKeyboardSelected: boolean;
}

/** 集中管理设置页各表面的主题预览和皮肤卡片的明暗覆盖。 */
export function useSettingsThemePreviews({
  draft,
  platform,
  surfaceThemesFollow = false,
  showCandidateNumberColor,
  showCandidateBorderColor,
}: UseSettingsThemePreviewsOptions): SettingsThemePreviews {
  const { themeMode, settingsTheme, globalTheme, customColors, customTouchKeyboardSkin } =
    settingsThemePreferences(draft, platform, surfaceThemesFollow);
  const [skinPreviewThemes, setSkinPreviewThemes] = useState<
    Partial<Record<GlobalTheme, "light" | "dark">>
  >({});
  const previewCustomTheme = hostDrawnCustomTheme(
    draft?.custom_theme,
    showCandidateNumberColor,
    showCandidateBorderColor,
  );
  const candidatePreviewTheme = useCandidatePreviewTheme(themeMode, draft?.candidate_theme);
  const toolbarPreviewTheme = useCandidatePreviewTheme(
    themeMode,
    surfaceThemesFollow ? undefined : draft?.toolbar_theme,
  );
  const keyboardPreviewTheme = useCandidatePreviewTheme(
    themeMode,
    surfaceThemesFollow ? undefined : draft?.screen_keyboard_theme,
  );
  const customKeyboardSelected = globalTheme === "custom" && Boolean(draft?.custom_theme?.keyboard);

  useEffect(() => setSkinPreviewThemes({}), [candidatePreviewTheme]);

  return {
    themeMode,
    settingsTheme,
    globalTheme,
    customColors,
    customTouchKeyboardSkin,
    skinPreviewThemes,
    setSkinPreviewThemes,
    candidatePreviewTheme,
    toolbarPreviewTheme,
    keyboardPreviewTheme,
    previewCustomTheme,
    customKeyboardSelected,
  };
}
