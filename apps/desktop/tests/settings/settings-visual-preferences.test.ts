import { expect, test } from "vitest";
import { settingsVisualPreferences } from "@msime/ui";

test("fills visual and touch preferences with shared defaults", () => {
  const values = settingsVisualPreferences();

  expect(values.inputModeHUD).toBe(true);
  expect(values.themeMode).toBe("system");
  expect(values.settingsTheme).toBe("follow");
  expect(values.touchKeyboardSkin).toBe("forest");
  expect(values.touchKeySpacingTenths).toBe(60);
  expect(values.touchRowSpacingTenths).toBe(70);
  expect(values.touchKeyboardHeightAdjustment).toBe(0);
  expect(values.floatingToolbar.scale_percent).toBe(100);
  expect(values.touchKeyboardSchemes.enabled).toContain("quanpin");
});

test("preserves visual and touch values supplied by the draft", () => {
  const values = settingsVisualPreferences({
    input_mode_hud: false,
    theme: "dark",
    settings_theme: "light",
    touch_keyboard_skin: "custom",
    touch_key_spacing_tenths: 82,
    touch_row_spacing_tenths: 91,
    touch_keyboard_height_adjustment: -3,
  });

  expect(values.inputModeHUD).toBe(false);
  expect(values.themeMode).toBe("dark");
  expect(values.settingsTheme).toBe("light");
  expect(values.touchKeyboardSkin).toBe("custom");
  expect(values.touchKeySpacingTenths).toBe(82);
  expect(values.touchRowSpacingTenths).toBe(91);
  expect(values.touchKeyboardHeightAdjustment).toBe(-3);
});
