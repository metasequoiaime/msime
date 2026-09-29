import { expect, test } from "vitest";
import {
  isVoicePolishEnabled,
  voiceAsrTokenLabel,
} from "../../../../packages/ui/src/settings/voice-input-defaults";

test("enables voice polish when either preference flag is true", () => {
  expect(isVoicePolishEnabled({ polish_text: true })).toBe(true);
  expect(isVoicePolishEnabled({ polish_enabled: true })).toBe(true);
  expect(isVoicePolishEnabled({ polish_text: true, polish_enabled: false })).toBe(true);
});

test("keeps voice polish disabled when neither preference flag is true", () => {
  expect(isVoicePolishEnabled({})).toBe(false);
  expect(isVoicePolishEnabled({ polish_text: false, polish_enabled: false })).toBe(false);
});

test("uses the Doubao API key label only for its non-legacy authentication", () => {
  expect(voiceAsrTokenLabel("doubao", "api_key")).toBe("Doubao API Key");
  expect(voiceAsrTokenLabel("doubao", "legacy")).toBe("识别 API Token");
  expect(voiceAsrTokenLabel("openai", "api_key")).toBe("识别 API Token");
});
