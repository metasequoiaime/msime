#include "../src/core/LocalModeSwitches.h"
#include "../src/core/SpellingSymbols.h"

#include <cassert>

int main() {
  using msime::linux_host::local_mode_spelling;
  using msime::linux_host::spelling_digits;
  using msime::linux_host::spelling_symbol;
  using Json = nlohmann::json;

  // The expression mode spells with digits and operators: every one of them is input, and nothing else is.
  const Json expression = {{"local_mode", "expression"},
                           {"spelling_symbols", "0123456789+-*/.()%^"}};
  for (char32_t character : U"0123456789+-*/.()%^")
    if (character != 0) {
      assert(spelling_symbol(expression, character));
      assert(local_mode_spelling(expression, character));
    }
  for (char32_t character : {U'!', U'@', U'#', U'$', U'&', U',', U'=', U'[', U'a', U'V', U' '})
    assert(!local_mode_spelling(expression, character));
  assert(spelling_digits(expression));

  // Unicode spells with digits only; its "+" after U is the host's own rule, not a spelling symbol.
  const Json unicode = {{"local_mode", "unicode"}, {"spelling_symbols", "0123456789"}};
  assert(spelling_digits(unicode));
  assert(local_mode_spelling(unicode, U'7'));
  assert(!local_mode_spelling(unicode, U'+'));

  // Idle, the mode-entry keys are listed but left to the punctuation route, and digits stay candidate shortcuts.
  const Json idle = {{"local_mode", "none"}, {"spelling_symbols", "/@"}};
  assert(spelling_symbol(idle, U'/'));
  assert(spelling_symbol(idle, U'@'));
  assert(!local_mode_spelling(idle, U'/'));
  assert(!local_mode_spelling(idle, U'@'));
  assert(!spelling_digits(idle));

  // Modes that spell with letters list nothing; a view without the field (an older library, a null view before the first render) lists nothing either.
  const Json emoji = {{"local_mode", "emoji"}, {"spelling_symbols", ""}};
  assert(!local_mode_spelling(emoji, U'1'));
  assert(!spelling_digits(emoji));
  assert(!spelling_symbol(Json{{"local_mode", "expression"}}, U'1'));
  assert(!spelling_symbol(Json(nullptr), U'1'));
  assert(!local_mode_spelling(Json(nullptr), U'1'));
  assert(!spelling_symbol(Json{{"spelling_symbols", 12}}, U'1'));

  // Outside a local mode, a scheme that opens none spells with every symbol it lists: Zhuyin's keyboard keys idle, while composing (Space is the first tone) and with its list open, where "0" is still a key and 1 to 9 pick candidates.
  using msime::linux_host::engine_spelling;
  using msime::linux_host::spelling_space;
  const Json zhuyin_idle = {{"scheme", 6}, {"local_mode", "none"}, {"spelling_symbols", "125890,./;-"}};
  for (char32_t character : U"125890,./;-")
    if (character != 0) assert(engine_spelling(zhuyin_idle, character));
  assert(!engine_spelling(zhuyin_idle, U'3'));
  assert(!spelling_space(zhuyin_idle));
  const Json zhuyin_composing = {{"scheme", 6}, {"local_mode", "none"}, {"spelling_symbols", "1234567890,./;- "}};
  assert(engine_spelling(zhuyin_composing, U'3'));
  assert(spelling_space(zhuyin_composing));
  assert(spelling_digits(zhuyin_composing));
  const Json zhuyin_list = {{"scheme", 6}, {"local_mode", "none"}, {"spelling_symbols", "0,./;-"}};
  assert(engine_spelling(zhuyin_list, U'0'));
  assert(!engine_spelling(zhuyin_list, U'1'));
  assert(!spelling_digits(zhuyin_list));
  assert(!spelling_space(zhuyin_list));
  // Cantonese's apostrophe and Vietnamese's VNI tone digits are spelling while composing.
  assert(engine_spelling(Json{{"scheme", 5}, {"spelling_symbols", "'"}}, U'\''));
  const Json vietnamese = {{"scheme", 7}, {"local_mode", "none"}, {"spelling_symbols", "0123456789"}};
  assert(engine_spelling(vietnamese, U'6'));
  assert(spelling_digits(vietnamese));
  // Stroke spells with the letters h s p n z and the wildcard x alone, which go through the letter path: it lists no symbols, so while it composes the number row still picks candidates and punctuation stays punctuation.
  const Json stroke = {{"scheme", 8}, {"local_mode", "none"}, {"editing_text", "hx"}, {"spelling_symbols", ""}};
  for (char32_t character : {U'1', U'0', U'\'', U',', U'x', U'*'})
    assert(!engine_spelling(stroke, character));
  assert(!spelling_digits(stroke));
  assert(!spelling_space(stroke));
  // Quanpin's idle mode-entry keys stay on the punctuation route, and the dedicated English mode keeps no scheme rules.
  assert(!engine_spelling(Json{{"scheme", 0}, {"local_mode", "none"}, {"spelling_symbols", "/@"}}, U'/'));
  assert(!engine_spelling(Json{{"scheme", 6}, {"dedicated_english", true}, {"spelling_symbols", "1"}}, U'1'));
  // A local mode spells with its own symbols in every scheme.
  assert(engine_spelling(Json{{"scheme", 0}, {"local_mode", "expression"}, {"spelling_symbols", "0123456789+-*/.()%^"}}, U'+'));
  assert(!engine_spelling(Json(nullptr), U'1'));
  assert(!spelling_space(Json(nullptr)));

  // Only printable ASCII is ever a spelling symbol: a keysym with no character, or one outside ASCII, is not.
  assert(!spelling_symbol(expression, 0));
  assert(!spelling_symbol(expression, U'·'));
  assert(!spelling_symbol(Json{{"spelling_symbols", std::string("0\x01", 2)}}, 1));

  // The status menus read a missing switch as its client-core default: the plugin modes are off.
  using msime::linux_host::local_mode_enabled_by_default;
  for (const char *mode : {"unicode", "date_time", "quick_phrase", "emoji", "kaomoji",
                           "super_jianpin", "temporary_english", "temporary_japanese"})
    assert(local_mode_enabled_by_default(mode));
  for (const char *mode : {"expression", "command", "mention"})
    assert(!local_mode_enabled_by_default(mode));
}
