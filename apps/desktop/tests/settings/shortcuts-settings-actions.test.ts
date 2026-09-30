import { expect, test, vi } from "vitest";
import { createShortcutsSettingsActions, type Preferences } from "@msime/ui";

const draft: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
  keybindings: {
    switch_language_shift: true,
    switch_language_ctrl: false,
    switch_language_ctrl_alt_space: false,
    toggle_character_set_ctrl_shift_f: true,
    toggle_fullwidth_option_shift_h: false,
  },
};

test("updates shortcut preferences while preserving existing bindings", () => {
  const setDraft = vi.fn();
  const actions = createShortcutsSettingsActions({
    setDraft,
  });

  actions.onKeybindingsChange({ switch_language_ctrl: true });
  actions.onInputModeHUDChange(true);
  actions.onNumberRowSelectionChange(false);

  const keybindingsUpdater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  const hudUpdater = setDraft.mock.calls[1][0] as (value: Preferences) => Preferences;
  const numberUpdater = setDraft.mock.calls[2][0] as (value: Preferences) => Preferences;
  expect(keybindingsUpdater(draft).keybindings).toMatchObject({
    switch_language_shift: true,
    switch_language_ctrl: true,
  });
  expect(hudUpdater(draft)).toMatchObject({ input_mode_hud: true });
  expect(numberUpdater(draft)).toMatchObject({ number_row_selection: false });
});

test("applies shortcut patches to the latest draft", () => {
  const setDraft = vi.fn();
  const actions = createShortcutsSettingsActions({ setDraft });

  actions.onKeybindingsChange({ switch_language_ctrl: true });

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater({ ...draft, candidate_page_size: 9 }).candidate_page_size).toBe(9);
});
