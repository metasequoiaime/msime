import { expect, test } from "vitest";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import { settingsThemePreferences } from "../../../../packages/ui/src/settings/settings-theme-preferences";

test("fills missing theme preferences with shared defaults", () => {
  expect(settingsThemePreferences()).toMatchObject({
    themeMode: "system",
    settingsTheme: "follow",
    globalTheme: "system",
    customColors: {},
    customTouchKeyboardSkin: defaultTouchKeyboardSkinDesign,
  });
});

test("keeps explicit theme preferences", () => {
  expect(
    settingsThemePreferences({
      theme: "dark",
      settings_theme: "light",
      global_theme: "night",
      custom_theme: { candidate_colors: { accent: "#123456" }, keyboard: null },
    }),
  ).toMatchObject({
    themeMode: "dark",
    settingsTheme: "light",
    globalTheme: "night",
    customColors: { accent: "#123456" },
    customTouchKeyboardSkin: defaultTouchKeyboardSkinDesign,
  });
});
