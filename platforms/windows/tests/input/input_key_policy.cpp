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
  // Shift+左右和 Ctrl+左右一样是分段光标键；两个修饰键一起按、带 Alt、或者不带修饰键都不是。Shift+退格不是分段删除。
  using msime::windows::kModifierAlt;
  static_assert(is_segment_caret_key(kVirtualKeyLeft, kModifierShift));
  static_assert(is_segment_caret_key(kVirtualKeyRight, kModifierShift));
  static_assert(!is_segment_caret_key(kVirtualKeyLeft, kModifierShift | kModifierAlt));
  static_assert(!is_segment_caret_key(kVirtualKeyLeft, kModifierControl | kModifierAlt));
  static_assert(!is_segment_caret_key(kVirtualKeyLeft, 0));
  static_assert(!is_segment_caret_key(kVirtualKeyBackspace, kModifierShift));
  static_assert(!is_segment_backspace_key(kVirtualKeyBackspace, kModifierShift));
  // 整句改字：全拼、双拼里不带修饰键的左右键换成改字命令；分段命令（Shift/Ctrl+左右）在所有方案里都原样发出，不再换成逐个字母的光标命令；其他方案原样。
  using msime::windows::sentence_edit_command;
  static_assert(sentence_edit_command(4, true) == 17);
  static_assert(sentence_edit_command(5, true) == 18);
  static_assert(sentence_edit_command(13, true) == 13);
  static_assert(sentence_edit_command(14, true) == 14);
  static_assert(sentence_edit_command(12, true) == 12);
  static_assert(sentence_edit_command(4, false) == 4);
  static_assert(sentence_edit_command(13, false) == 13);
  static_assert(sentence_edit_command(14, false) == 14);
  using msime::windows::should_send_composition_reply;
  assert(should_send_composition_reply(false, false, false, false, false,
                                        true));
  assert(!should_send_composition_reply(false, false, false, false, false,
                                         false));
  using msime::windows::is_english_mode_toggle_key;
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
