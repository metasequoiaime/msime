export type KeybindingPreferences = {
  switch_language_shift: boolean;
  /** macOS 单按 Shift 仅在可编辑焦点生效，需辅助功能权限，缺省为关。 */
  switch_language_shift_requires_editable?: boolean;
  switch_language_ctrl: boolean;
  switch_language_ctrl_space?: boolean;
  switch_language_ctrl_alt_space: boolean;
  toggle_character_set_ctrl_shift_f: boolean;
  toggle_fullwidth_option_shift_h: boolean;
  /** 单按 Caps Lock 切换中英文，长按照常是大写锁定。缺省为关。 */
  switch_language_caps_lock?: boolean;
  /** Ctrl+Shift+J 在中文和日语之间切换。缺省为关。 */
  toggle_japanese_ctrl_shift_j?: boolean;
  /** 中日英循环切换：打开后「切换中英文」这一组快捷键按 中 → 日 → 英 → 中 转圈。缺省为关。 */
  trilingual_cycle?: boolean;
  /** 中英切换（英文候选模式）热键，写法见 client-core 的 KeyChord（如 `Ctrl+Shift+KeyE`）；空串表示不绑定，缺省为出厂值。 */
  toggle_english_candidates?: string;
  /** 全角 / 半角切换热键（如 `Ctrl+Shift+Space`）；空串表示不绑定，缺省为出厂值。 */
  toggle_full_width?: string;
  /** 中英文标点切换热键（如 `Ctrl+Period`）；空串表示不绑定，缺省为出厂值。 */
  toggle_punctuation?: string;
  /** 额外的语音开始／结束热键，缺省或空串表示不绑定。 */
  toggle_voice_input?: string;
};
