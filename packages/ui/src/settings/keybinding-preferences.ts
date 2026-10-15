export type KeybindingPreferences = {
  switch_language_shift: boolean;
  switch_language_ctrl: boolean;
  switch_language_ctrl_space?: boolean;
  switch_language_ctrl_alt_space: boolean;
  toggle_character_set_ctrl_shift_f: boolean;
  toggle_fullwidth_option_shift_h: boolean;
  /** 单按 Caps Lock 切换中英文，长按照常是大写锁定。缺省为关。 */
  switch_language_caps_lock?: boolean;
  /** Ctrl+Shift+J 在中文和日语之间切换。缺省为关。 */
  toggle_japanese_ctrl_shift_j?: boolean;
};
