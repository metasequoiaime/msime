import type { CustomCandidateColors, GlobalTheme } from "../theme/global-theme";
import {
  defaultTouchKeyboardSkinDesign,
  type TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";
import type { Preferences } from "../index";

export interface SettingsThemePreferences {
  themeMode: NonNullable<Preferences["theme"]>;
  settingsTheme: NonNullable<Preferences["settings_theme"]>;
  globalTheme: GlobalTheme;
  customColors: CustomCandidateColors;
  customTouchKeyboardSkin: TouchKeyboardSkinDesign;
}

/** Resolves the theme defaults used by settings pages and their previews. */
export function settingsThemePreferences(draft?: Preferences): SettingsThemePreferences {
  return {
    themeMode: draft?.theme ?? "system",
    settingsTheme: draft?.settings_theme ?? "follow",
    globalTheme: draft?.global_theme ?? "system",
    customColors: draft?.custom_theme?.candidate_colors ?? {},
    customTouchKeyboardSkin: draft?.custom_theme?.keyboard ?? defaultTouchKeyboardSkinDesign,
  };
}
