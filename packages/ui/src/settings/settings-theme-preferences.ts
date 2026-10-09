import {
  offeredGlobalTheme,
  type CustomCandidateColors,
  type GlobalTheme,
} from "../theme/global-theme";
import {
  defaultTouchKeyboardSkinDesign,
  type TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";
import type { HostPlatform, Preferences } from "../index";

export interface SettingsThemePreferences {
  themeMode: NonNullable<Preferences["theme"]>;
  settingsTheme: NonNullable<Preferences["settings_theme"]>;
  globalTheme: GlobalTheme;
  customColors: CustomCandidateColors;
  customTouchKeyboardSkin: TouchKeyboardSkinDesign;
}

export type SettingsThemePreferencesSource = Pick<
  Preferences,
  "theme" | "settings_theme" | "global_theme" | "custom_theme"
>;

/** 设置页和预览用到的主题取值，缺省的补上默认值。`platform` 不提供的全局主题（iOS 以外收到的 `native`）按 `system` 处理，与宿主实际画的一致。 */
export function settingsThemePreferences(
  draft?: SettingsThemePreferencesSource,
  platform?: HostPlatform,
): SettingsThemePreferences {
  return {
    themeMode: draft?.theme ?? "system",
    settingsTheme: draft?.settings_theme ?? "follow",
    globalTheme: offeredGlobalTheme(draft?.global_theme ?? "system", platform),
    customColors: draft?.custom_theme?.candidate_colors ?? {},
    customTouchKeyboardSkin: draft?.custom_theme?.keyboard ?? defaultTouchKeyboardSkinDesign,
  };
}
