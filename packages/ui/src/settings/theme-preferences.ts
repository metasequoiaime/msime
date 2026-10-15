export type ThemeMode = "dark" | "light" | "system";
export type SurfaceTheme = "follow" | "dark" | "light";

/** 可以在颜色模式之上单独设置明暗的各个界面。颜色模式本身（`theme`）是「主题」页「明暗」组的分段控件，不属于这些界面。 */
export type ThemePreferences = {
  settings_theme?: SurfaceTheme;
  screen_keyboard_theme?: SurfaceTheme;
  candidate_theme?: SurfaceTheme;
  toolbar_theme?: SurfaceTheme;
  menu_theme?: SurfaceTheme;
  emoji_theme?: SurfaceTheme;
  handwriting_theme?: SurfaceTheme;
  voice_theme?: SurfaceTheme;
};

export type ThemePreferenceKey = keyof ThemePreferences;
