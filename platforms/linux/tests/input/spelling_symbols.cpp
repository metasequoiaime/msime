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
  // Shift and the number row pick a Unicode candidate even where the chord types the digit (AZERTY), and only then: a bare digit, a key off the row, a symbol or no list to pick from leaves the digit input.
  using msime::linux_host::shifted_number_row_picks;
  assert(shifted_number_row_picks(unicode, U'1', true, true, true));
  assert(!shifted_number_row_picks(unicode, U'1', false, true, true));
  assert(!shifted_number_row_picks(unicode, U'1', true, false, true));
  assert(!shifted_number_row_picks(unicode, U'1', true, true, false));
  assert(!shifted_number_row_picks(unicode, U'!', true, true, true));
  assert(!shifted_number_row_picks(expression, U'%', true, true, true));

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
  // 藏文威利转写：空闲时撇号（achung 开头的音节）和斜杠（单独输出垂符）是拼写，组字时再加上叠写加号、消歧句点和连字符；数字和空格不是拼写，交给宿主自己的规则。
  const Json tibetan_idle = {{"scheme", 8}, {"local_mode", "none"}, {"spelling_symbols", "'/"}};
  assert(engine_spelling(tibetan_idle, U'\''));
  assert(engine_spelling(tibetan_idle, U'/'));
  assert(!engine_spelling(tibetan_idle, U'+'));
  const Json tibetan_composing = {{"scheme", 8}, {"local_mode", "none"}, {"spelling_symbols", "'+-./"}};
  for (char32_t symbol : {U'\'', U'+', U'-', U'.', U'/'})
    assert(engine_spelling(tibetan_composing, symbol));
  assert(!engine_spelling(tibetan_composing, U','));
  assert(!spelling_digits(tibetan_composing));
  assert(!spelling_space(tibetan_composing));
  // 笔画只用字母 h s p n z 和通配符 x 拼写，它们走字母路径：方案不列任何拼写符号，所以组字时数字行仍然选词，标点仍然是标点。
  const Json stroke = {{"scheme", 9}, {"local_mode", "none"}, {"editing_text", "hx"}, {"spelling_symbols", ""}};
  for (char32_t character : {U'1', U'0', U'\'', U',', U'x', U'*'})
    assert(!engine_spelling(stroke, character));
  assert(!spelling_digits(stroke));
  assert(!spelling_space(stroke));
  // Quanpin's idle mode-entry keys stay on the punctuation route, and the dedicated English mode keeps no scheme rules.
  assert(!engine_spelling(Json{{"scheme", 0}, {"local_mode", "none"}, {"spelling_symbols", "/@"}}, U'/'));
  assert(!engine_spelling(Json{{"scheme", 6}, {"dedicated_english", true}, {"spelling_symbols", "1"}}, U'1'));
  // 组字原文是网址触发词时，全拼、双拼只列出打开网址模式的键：它是拼写，不是翻页键或中文标点；没列出的键和空闲时的 "/" 仍走宿主自己的规则。
  const Json url_entry = {{"scheme", 0}, {"local_mode", "none"}, {"editing_text", "www"}, {"spelling_symbols", "."}};
  assert(engine_spelling(url_entry, U'.'));
  assert(!engine_spelling(url_entry, U','));
  assert(!engine_spelling(url_entry, U'1'));
  assert(!spelling_digits(url_entry));
  const Json url_scheme = {{"scheme", 1}, {"local_mode", "none"}, {"editing_text", "https"}, {"spelling_symbols", ":"}};
  assert(engine_spelling(url_scheme, U':'));
  assert(!engine_spelling(url_scheme, U'.'));
  // 普通组字不列符号，句号仍是中文标点；没有组字时列出的键（空 editing_text）不算组字拼写。
  assert(!engine_spelling(Json{{"scheme", 0}, {"local_mode", "none"}, {"editing_text", "zhong"}, {"spelling_symbols", ""}}, U'.'));
  assert(!engine_spelling(Json{{"scheme", 0}, {"local_mode", "none"}, {"editing_text", ""}, {"spelling_symbols", "/@"}}, U'/'));
  // 网址模式本身是本地模式：数字和网址符号都是输入，数字行不再选词。
  const Json url_mode = {{"scheme", 0}, {"local_mode", "url"}, {"editing_text", "www."}, {"spelling_symbols", "0123456789-._~:/?#[]@!$&'()*+,;=%^"}};
  for (char32_t character : U"0123456789-._~:/?#[]@!$&'()*+,;=%^")
    if (character != 0) assert(engine_spelling(url_mode, character));
  for (char32_t character : {U'"', U'<', U'>', U'\\', U'{', U'}', U'|', U'`', U' '})
    assert(!engine_spelling(url_mode, character));
  assert(spelling_digits(url_mode));
  assert(!spelling_space(url_mode));
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
