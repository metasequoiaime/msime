import { expect, test, vi } from "vitest";
import { createSettingsDraftActions, type Preferences } from "@msime/ui";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

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

test("updates custom theme preferences while preserving the rest of the draft", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const draft = {
    scheme: "quanpin",
    custom_theme: { base: "system" },
  } as unknown as Preferences;

  actions.onCustomThemeChange({ candidate_skin: "synthetic-skin" });
  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const next = updater(draft);

  expect(next.custom_theme).toEqual({ base: "system", candidate_skin: "synthetic-skin" });
  expect(next.scheme).toBe("quanpin");
});

test("replaces the draft for translation settings changes", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const next = { scheme: "wubi" } as unknown as Preferences;

  actions.onTranslationChange(next);

  expect(setDraft).toHaveBeenCalledWith(next);
});

test("merges shared preference patches into the current draft", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const draft = { scheme: "quanpin", learning: true } as unknown as Preferences;

  actions.onPreferencesChange({ learning: false });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft)).toMatchObject({ scheme: "quanpin", learning: false });
});

test("selects a custom keyboard through the shared draft action", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const design = { ...defaultTouchKeyboardSkinDesign, key_radius: 11 };
  const draft = {
    scheme: "quanpin",
    global_theme: "night",
    custom_theme: { base: "night", candidate_skin: "community-skin" },
  } as unknown as Preferences;

  actions.onCustomKeyboardChange(design);

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft).custom_theme).toMatchObject({
    base: "night",
    candidate_skin: null,
    keyboard: design,
  });
});

test("uses the latest custom keyboard draft through the shared action", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const design = { ...defaultTouchKeyboardSkinDesign, key_radius: 13 };
  const draft = {
    scheme: "quanpin",
    global_theme: "custom",
    custom_theme: { base: "night", keyboard: design },
  } as unknown as Preferences;

  actions.onUseCustomKeyboard();

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft).custom_theme?.keyboard).toEqual(design);
});

test("updates a candidate color through the shared draft action", () => {
  const setDraft = vi.fn();
  const actions = createSettingsDraftActions({ setDraft });
  const draft = {
    scheme: "quanpin",
    global_theme: "night",
    custom_theme: { base: "night", candidate_skin: "community-skin" },
  } as unknown as Preferences;

  actions.onCandidateColorChange("surface", "#123456");

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater(draft).custom_theme).toMatchObject({
    base: "night",
    candidate_skin: null,
    candidate_colors: { surface: "#123456" },
  });
});
