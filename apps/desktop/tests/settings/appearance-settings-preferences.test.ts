import { expect, test } from "vitest";
import { appearanceSettingsPreferences, type Preferences } from "@msime/ui";

function preferences(candidate_english_font?: string): Preferences {
  return {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    candidate_english_font,
  };
}

test("fills the Windows candidate font default", () => {
  const draft = { ...preferences(), theme: "dark" as const };
  expect(appearanceSettingsPreferences(draft, true)).toMatchObject({
    candidate_english_font: "Segoe UI",
    theme: "dark",
  });
});

test("preserves explicit and non-Windows candidate fonts", () => {
  expect(
    appearanceSettingsPreferences(preferences("Synthetic Font"), true).candidate_english_font,
  ).toBe("Synthetic Font");
  expect(
    appearanceSettingsPreferences(preferences(), false).candidate_english_font,
  ).toBeUndefined();
});
