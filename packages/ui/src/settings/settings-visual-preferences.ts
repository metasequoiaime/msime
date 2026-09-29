import type { Preferences } from "../index";
import { defaultFloatingToolbar } from "./floating-toolbar-defaults";
import { defaultTouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { allTouchKeyboardSchemes } from "./touch-keyboard-scheme-helpers";

export type SettingsVisualPreferencesSource = Pick<
  Preferences,
  | "input_mode_hud"
  | "floating_toolbar"
  | "theme"
  | "settings_theme"
  | "touch_keyboard_schemes"
  | "touch_keyboard_skin"
  | "custom_touch_keyboard_skin"
  | "touch_key_spacing_tenths"
  | "touch_row_spacing_tenths"
  | "touch_keyboard_height_adjustment"
>;

export interface SettingsVisualPreferences {
  inputModeHUD: boolean;
  floatingToolbar: NonNullable<Preferences["floating_toolbar"]>;
  themeMode: NonNullable<Preferences["theme"]>;
  settingsTheme: NonNullable<Preferences["settings_theme"]>;
  touchKeyboardSchemes: NonNullable<Preferences["touch_keyboard_schemes"]>;
  touchKeyboardSkin: NonNullable<Preferences["touch_keyboard_skin"]>;
  customTouchKeyboardSkin: NonNullable<Preferences["custom_touch_keyboard_skin"]>;
  touchKeySpacingTenths: number;
  touchRowSpacingTenths: number;
  touchKeyboardHeightAdjustment: number;
}

/** Resolves visual and touch-keyboard preferences with the shared UI defaults. */
export function settingsVisualPreferences(
  draft?: SettingsVisualPreferencesSource,
): SettingsVisualPreferences {
  return {
    inputModeHUD: draft?.input_mode_hud ?? true,
    floatingToolbar: { ...defaultFloatingToolbar, ...draft?.floating_toolbar },
    themeMode: draft?.theme ?? "system",
    settingsTheme: draft?.settings_theme ?? "follow",
    touchKeyboardSchemes: draft?.touch_keyboard_schemes ?? {
      enabled: allTouchKeyboardSchemes,
    },
    touchKeyboardSkin: draft?.touch_keyboard_skin ?? "forest",
    customTouchKeyboardSkin: draft?.custom_touch_keyboard_skin ?? defaultTouchKeyboardSkinDesign,
    touchKeySpacingTenths: draft?.touch_key_spacing_tenths ?? 60,
    touchRowSpacingTenths: draft?.touch_row_spacing_tenths ?? 70,
    touchKeyboardHeightAdjustment: draft?.touch_keyboard_height_adjustment ?? 0,
  };
}
