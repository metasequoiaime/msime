#include "input/InputKeyPolicy.h"
#include <cassert>

int main() {
  using msime::windows::should_learn_entered_english_word;
  using msime::windows::normalize_numpad_digit_key;
  using msime::windows::is_backend_independent_reset_key;
  assert(normalize_numpad_digit_key(0x60) == '0');
  assert(normalize_numpad_digit_key(0x69) == '9');
  assert(normalize_numpad_digit_key(0x41) == 0x41);
  assert(is_backend_independent_reset_key(0x1B));
  assert(is_backend_independent_reset_key(0xA1));
  assert(!is_backend_independent_reset_key(0xA2));
  using msime::windows::is_segment_backspace_key;
  using msime::windows::is_segment_caret_key;
  using msime::windows::kModifierControl;
  using msime::windows::kModifierShift;
  using msime::windows::kVirtualKeyBackspace;
  using msime::windows::kVirtualKeyLeft;
  using msime::windows::kVirtualKeyRight;
  assert(is_segment_backspace_key(kVirtualKeyBackspace, kModifierControl));
  assert(!is_segment_backspace_key(kVirtualKeyBackspace,
                                   kModifierControl | kModifierShift));
  assert(is_segment_caret_key(kVirtualKeyLeft, kModifierControl));
  assert(is_segment_caret_key(kVirtualKeyRight, kModifierControl));
  assert(!is_segment_caret_key(kVirtualKeyRight, kModifierControl | kModifierShift));
  // 整句改字：全拼、双拼里左右键换成改字命令，Ctrl+左右换成逐个字母的光标命令；其他方案原样。
  using msime::windows::sentence_edit_command;
  static_assert(sentence_edit_command(4, true) == 17);
  static_assert(sentence_edit_command(5, true) == 18);
  static_assert(sentence_edit_command(13, true) == 4);
  static_assert(sentence_edit_command(14, true) == 5);
  static_assert(sentence_edit_command(12, true) == 12);
  static_assert(sentence_edit_command(4, false) == 4);
  static_assert(sentence_edit_command(13, false) == 13);
  using msime::windows::should_send_composition_reply;
  assert(should_send_composition_reply(false, false, false, false, false,
                                        true));
  assert(!should_send_composition_reply(false, false, false, false, false,
                                         false));
  using msime::windows::is_english_mode_toggle_key;
  using msime::windows::kModifierAlt;
  assert(is_english_mode_toggle_key('E', kModifierControl | kModifierShift));
  assert(!is_english_mode_toggle_key('E', kModifierControl | kModifierShift | kModifierAlt));
  assert(!is_english_mode_toggle_key('E', kModifierControl));
  assert(!is_english_mode_toggle_key('E', kModifierShift));
  assert(!is_english_mode_toggle_key('F', kModifierControl | kModifierShift));
  assert(should_learn_entered_english_word(false, false, true, false));
  assert(should_learn_entered_english_word(true, false, true, true));
  assert(should_learn_entered_english_word(false, true, true, true));
  assert(should_learn_entered_english_word(false, true, false, true));
  assert(!should_learn_entered_english_word(false, false, true, true));
  assert(!should_learn_entered_english_word(false, false, false, false));
}
