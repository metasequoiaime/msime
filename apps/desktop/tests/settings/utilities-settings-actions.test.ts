import { expect, test, vi } from "vitest";
import { createUtilitiesSettingsActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
};

test("merges local-mode changes into the current draft", () => {
  const setDraft = vi.fn();
  const actions = createUtilitiesSettingsActions({ draft, setDraft });
  const localModes = {
    unicode: true,
    date_time: false,
    quick_phrase: true,
    emoji: true,
    kaomoji: false,
    super_jianpin: true,
    temporary_english: true,
    temporary_japanese: false,
  };

  actions.onLocalModesChange(localModes);

  expect(setDraft).toHaveBeenCalledWith(expect.objectContaining({ local_modes: localModes }));
});

test("does not update before preferences have loaded", () => {
  const setDraft = vi.fn();
  const actions = createUtilitiesSettingsActions({ draft: undefined, setDraft });

  actions.onLocalModesChange({
    unicode: false,
    date_time: false,
    quick_phrase: false,
    emoji: false,
    kaomoji: false,
    super_jianpin: false,
    temporary_english: false,
    temporary_japanese: false,
  });

  expect(setDraft).not.toHaveBeenCalled();
});
