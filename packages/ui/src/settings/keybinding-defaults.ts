import type { KeybindingPreferences } from "./keybinding-preferences";

export const defaultKeybindings: KeybindingPreferences = {
  switch_language_shift: true,
  switch_language_shift_requires_editable: false,
  switch_language_ctrl: false,
  switch_language_ctrl_space: true,
  switch_language_ctrl_alt_space: true,
  toggle_character_set_ctrl_shift_f: true,
  toggle_fullwidth_option_shift_h: true,
  switch_language_caps_lock: false,
  toggle_japanese_ctrl_shift_j: false,
  trilingual_cycle: false,
  toggle_english_candidates: "Ctrl+Shift+KeyE",
  toggle_full_width: "Ctrl+Shift+Space",
  toggle_punctuation: "Ctrl+Period",
};

/** `client-core` 偏好中 `number_row_selection` 的默认值：显示候选时用 1 到 9 选择候选。 */
export const defaultNumberRowSelection = true;
