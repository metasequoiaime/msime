import { expect, test } from "vitest";
import { isVoicePolishEnabled } from "../../../../packages/ui/src/settings/voice-input-defaults";

test("enables voice polish when either preference flag is true", () => {
  expect(isVoicePolishEnabled({ polish_text: true })).toBe(true);
  expect(isVoicePolishEnabled({ polish_enabled: true })).toBe(true);
  expect(isVoicePolishEnabled({ polish_text: true, polish_enabled: false })).toBe(true);
});

test("keeps voice polish disabled when neither preference flag is true", () => {
  expect(isVoicePolishEnabled({})).toBe(false);
  expect(isVoicePolishEnabled({ polish_text: false, polish_enabled: false })).toBe(false);
});
