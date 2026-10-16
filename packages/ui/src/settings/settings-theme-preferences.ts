import type { HostPlatform } from "../core/host-capabilities";
import {
  offeredGlobalTheme,
  type CustomCandidateColors,
  type CustomTheme,
  type GlobalTheme,
} from "../theme/global-theme";
import {
  defaultTouchKeyboardSkinDesign,
  type TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";
import type { Preferences } from "../core/preferences-model";
import type { SurfaceTheme, ThemeMode } from "./theme-preferences";

export interface SettingsThemePreferences {
  themeMode: ThemeMode;
  settingsTheme: SurfaceTheme;
  globalTheme: GlobalTheme;
  customColors: CustomCandidateColors;
  customTouchKeyboardSkin: TouchKeyboardSkinDesign;
}

export type SettingsThemePreferencesSource = Pick<
  Preferences,
  "theme" | "settings_theme" | "global_theme" | "custom_theme"
>;

/** 设置页和预览用到的主题取值，缺省的补上默认值。`platform` 不提供的全局主题（iOS 以外收到的 `native`）按 `system` 处理，与宿主实际画的一致。`surfaceThemesFollow` 为真（`HostCapabilities.surface_themes_follow`，macOS、Windows、Linux 和 HarmonyOS）时设置窗口跟随颜色模式，不读存下的 `settings_theme`。 */
export function settingsThemePreferences(
  draft?: SettingsThemePreferencesSource,
  platform?: HostPlatform,
  surfaceThemesFollow = false,
): SettingsThemePreferences {
  return {
    themeMode: draft?.theme ?? "system",
    settingsTheme: surfaceThemesFollow ? "follow" : (draft?.settings_theme ?? "follow"),
    globalTheme: offeredGlobalTheme(draft?.global_theme ?? "system", platform),
    customColors: draft?.custom_theme?.candidate_colors ?? {},
    customTouchKeyboardSkin: draft?.custom_theme?.keyboard ?? defaultTouchKeyboardSkinDesign,
  };
}

/** 预览用的自定义主题：去掉宿主不读的编号色（`showNumberColor` 为假）和边框色（`showBorderColor` 为假），让设置页预览与宿主实际画的一致。桌面宿主上这两种颜色固定跟随主题，存下的旧值仍留在草稿里，只是预览不画；结果只给预览读，不能写回草稿。 */
export function hostDrawnCustomTheme(
  custom: CustomTheme | undefined,
  showNumberColor: boolean,
  showBorderColor: boolean,
): CustomTheme | undefined {
  const colors = custom?.candidate_colors;
  if (!custom || !colors || (showNumberColor && showBorderColor)) return custom;
  const { number, border, ...drawn } = colors;
  return {
    ...custom,
    candidate_colors: {
      ...drawn,
      ...(showNumberColor ? { number } : {}),
      ...(showBorderColor ? { border } : {}),
    },
  };
}
