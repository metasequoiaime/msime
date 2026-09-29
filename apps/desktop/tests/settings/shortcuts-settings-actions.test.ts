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
    draft,
    keybindings: draft.keybindings!,
    setDraft,
  });

  actions.onKeybindingsChange({ switch_language_ctrl: true });
  actions.onInputModeHUDChange(true);
  actions.onNumberRowSelectionChange(false);

  expect(setDraft).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({
      keybindings: expect.objectContaining({
        switch_language_shift: true,
        switch_language_ctrl: true,
      }),
    }),
  );
  expect(setDraft).toHaveBeenNthCalledWith(2, expect.objectContaining({ input_mode_hud: true }));
  expect(setDraft).toHaveBeenNthCalledWith(
    3,
    expect.objectContaining({ number_row_selection: false }),
  );
});
