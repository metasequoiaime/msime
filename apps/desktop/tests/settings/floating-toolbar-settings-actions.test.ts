import { expect, test, vi } from "vitest";
import {
  createFloatingToolbarSettingsActions,
  type FloatingToolbarPreferences,
  type Preferences,
} from "@msime/ui";

const draft = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
} as Preferences;

const floatingToolbar: FloatingToolbarPreferences = {
  enabled: true,
  english_mode: true,
  fullwidth: false,
  punctuation: true,
  character_set: false,
  emoji: true,
  handwriting: true,
  screen_keyboard: false,
  voice: true,
  settings: true,
  scale_percent: 100,
  font_size: 20,
};

test("merges floating-toolbar patches into the current draft", () => {
  const setDraft = vi.fn();
  const actions = createFloatingToolbarSettingsActions({ draft, floatingToolbar, setDraft });

  actions.onChange({ enabled: false, scale_percent: 125 });

  expect(setDraft).toHaveBeenCalledWith(
    expect.objectContaining({
      floating_toolbar: expect.objectContaining({ enabled: false, scale_percent: 125 }),
    }),
  );
});

test("does not update before preferences have loaded", () => {
  const setDraft = vi.fn();
  const actions = createFloatingToolbarSettingsActions({
    draft: undefined,
    floatingToolbar,
    setDraft,
  });

  actions.onChange({ enabled: false });

  expect(setDraft).not.toHaveBeenCalled();
});
