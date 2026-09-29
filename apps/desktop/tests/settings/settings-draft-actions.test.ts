import { expect, test, vi } from "vitest";
import { createSettingsDraftActions, type Preferences } from "@msime/ui";

test("updates nested AI and voice preferences while preserving the rest of the draft", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const draft = {
    scheme: "quanpin",
    ai_assistant: undefined,
    voice_input: undefined,
    chinese_punctuation: true,
  } as unknown as Preferences;

  actions.onAiChange({ enabled: true });
  const aiUpdater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const withAi = aiUpdater(draft);
  expect(withAi.ai_assistant?.enabled).toBe(true);
  expect(withAi.scheme).toBe("quanpin");

  actions.onVoiceChange({ language: "en-US" });
  const voiceUpdater = setDraft.mock.calls[1][0] as (value: Preferences) => Preferences;
  expect(voiceUpdater(draft).voice_input?.language).toBe("en-US");
});

test("replaces the draft for translation settings changes", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const next = { scheme: "wubi" } as unknown as Preferences;

  actions.onTranslationChange(next);

  expect(setDraft).toHaveBeenCalledWith(next);
});
