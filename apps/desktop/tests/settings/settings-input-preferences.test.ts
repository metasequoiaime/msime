import { expect, test } from "vitest";
import { settingsInputPreferences } from "@msime/ui";

test("fills absent input preferences with the shared defaults", () => {
  const values = settingsInputPreferences();

  expect(values.wordCharacter).toEqual({ enabled: true, keys: "brackets" });
  expect(values.keybindings.switch_language_shift).toBe(true);
  expect(values.frequency.mode).toBe("promote");
  expect(values.mixedInput.minimum_prefix).toBe(5);
  expect(values.fuzzyPinyin).toEqual({ enabled: false, rules: [] });
  expect(values.localModes.temporary_japanese).toBe(true);
  expect(values.navigation.page_up_down).toBe(true);
  expect(values.numberRowSelection).toBe(true);
});

test("keeps explicit input choices, including disabled switches", () => {
  const values = settingsInputPreferences({
    word_character: { enabled: false, keys: "minus_equal" },
    fuzzy_pinyin: { enabled: true, rules: ["z-zh"] },
    number_row_selection: false,
  });

  expect(values.wordCharacter).toEqual({ enabled: false, keys: "minus_equal" });
  expect(values.fuzzyPinyin).toEqual({ enabled: true, rules: ["z-zh"] });
  expect(values.numberRowSelection).toBe(false);
  expect(values.frequency.mode).toBe("promote");
});
