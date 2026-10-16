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

test("a theme the host does not offer reads as system", () => {
  expect(settingsThemePreferences({ global_theme: "native" }, "android").globalTheme).toBe(
    "system",
  );
  expect(settingsThemePreferences({ global_theme: "native" }).globalTheme).toBe("system");
  expect(settingsThemePreferences({ global_theme: "native" }, "ios").globalTheme).toBe("native");
  expect(settingsThemePreferences({ global_theme: "night" }, "android").globalTheme).toBe("night");
});

// 桌面宿主的设置窗口跟随颜色模式（`surface_themes_follow`），存下的设置界面主题不再起作用；宿主报告不跟随时照旧读它。
test.each(["macos", "windows", "linux", "harmony"] as const)(
  "%s, whose surfaces follow the colour mode, ignores the stored settings theme",
  (platform) => {
    const draft = { theme: "dark" as const, settings_theme: "light" as const };
    expect(settingsThemePreferences(draft, platform, true)).toMatchObject({
      themeMode: "dark",
      settingsTheme: "follow",
    });
    expect(settingsThemePreferences(draft, platform, false).settingsTheme).toBe("light");
  },
);
