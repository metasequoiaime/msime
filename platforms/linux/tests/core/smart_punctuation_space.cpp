#include "../src/core/SmartPunctuationSpace.h"
#include "../../../../shared/contracts/punctuation/policy.h"

#include <cassert>
#include <string>
#include <vector>

using msime::linux_host::ascii_mark_from_text;
using msime::linux_host::ascii_mark_text;
using msime::linux_host::chinese_punctuation_mark;
using msime::linux_host::english_mode_output;
using msime::linux_host::EnglishPunctuationState;
using msime::linux_host::is_auto_paired_opening_key;
using msime::linux_host::is_smart_punctuation_key;
using msime::linux_host::is_space_conversion_key;
using msime::linux_host::repeat_conversion_matches_document;
using msime::linux_host::smart_punctuation_ascii_mark;
using msime::linux_host::space_conversion_ascii_text;
using msime::linux_host::space_conversion_matches_document;

int main() {
  const std::vector<std::string> after_letter{"好", "，"};
  // The mark is still the one that was committed, still after the same
  // character: this is the mark the user just typed.
  assert(space_conversion_matches_document("，", "好", after_letter));
  // Same mark, different neighbour - the caret moved to another comma in the
  // document, and rewriting it would edit text the user already accepted.
  assert(!space_conversion_matches_document("，", "刚", after_letter));
  // A different mark in front of the caret is not this key's mark.
  assert(!space_conversion_matches_document("。", "好", after_letter));

  // No fingerprint could be taken - the mark opened the document, or the host
  // published no readable text in front of the caret. There is then nothing for
  // the readback to disagree with, and the mark match alone decides, which is
  // the rule the source settled on rather than disabling the feature in those
  // hosts.
  const std::vector<std::string> alone{"，"};
  assert(space_conversion_matches_document("，", "", alone));
  assert(space_conversion_matches_document("，", "", after_letter));
  // A fingerprint was recorded, so a mark that now opens the document is not
  // the one that was armed.
  assert(!space_conversion_matches_document("，", "好", alone));

  // Nothing in front of the caret leaves no mark to rewrite.
  assert(!space_conversion_matches_document("，", "好", {}));
  assert(!space_conversion_matches_document("，", "", {}));
  // A key with no Chinese mark never arms, and never matches if it somehow did.
  assert(!space_conversion_matches_document("", "好", after_letter));

  // The three keys smart punctuation routes, and only those.
  assert(is_smart_punctuation_key(',') && is_smart_punctuation_key('.') &&
         is_smart_punctuation_key(':'));
  assert(!is_smart_punctuation_key(';') && !is_smart_punctuation_key('!') &&
         !is_smart_punctuation_key('a') && !is_smart_punctuation_key(' '));

  // Direct smart routing remains the narrow three-key policy.
  assert(chinese_punctuation_mark(',') == "，");
  assert(chinese_punctuation_mark('.') == "。");
  assert(chinese_punctuation_mark(':') == "：");
  assert(chinese_punctuation_mark('<') == "《");
  assert(chinese_punctuation_mark('a').empty());
  assert(chinese_punctuation_mark(' ').empty());

  // Space conversion follows the complete Windows mapping. Both halves of a
  // quote map to the same ASCII key; braces and corner brackets are not part of
  // the product contract. `、` 的 ASCII 键随顿号开关走，不在这张静态表里。
  const auto marks = msime::input::punctuation_marks_from_view("curly", "solid", true, false);
  const std::vector<std::pair<std::string_view, char>> conversions{
      {"。", '.'}, {"，", ','}, {"！", '!'}, {"？", '?'}, {"；", ';'},
      {"：", ':'}, {"“", '"'},  {"”", '"'},  {"‘", '\''},
      {"’", '\''}, {"【", '['}, {"】", ']'}, {"《", '<'}, {"》", '>'},
      {"（", '('}, {"）", ')'},
  };
  for (const auto &[chinese, ascii] : conversions) {
    assert(smart_punctuation_ascii_mark(chinese, 0, marks) == ascii);
    assert(is_space_conversion_key(ascii, marks));
    assert(space_conversion_ascii_text(chinese, 0, marks) == std::string(1, ascii));
  }
  assert(smart_punctuation_ascii_mark("〈", 0, marks) == 0);
  assert(smart_punctuation_ascii_mark("｛", 0, marks) == 0);
  assert(!is_space_conversion_key('{', marks));
  assert(space_conversion_ascii_text("x", 0, marks).empty());

  // 顿号空格反查随当前开着的键判（契约 enumerationComma 段）：按下的键开着时转回它，
  // 否则 `\` 优先、`/` 其次；关掉 `/` 后 `/` 不再是顿号来源，`\` 才是。
  assert(is_space_conversion_key('\\', marks));
  assert(!is_space_conversion_key('/', marks));
  assert(smart_punctuation_ascii_mark("、", '\\', marks) == '\\');
  assert(smart_punctuation_ascii_mark("、", '/', marks) == '\\');
  assert(smart_punctuation_ascii_mark("、", 0, marks) == '\\');
  assert(space_conversion_ascii_text("、", 0, marks) == "\\");
  const auto slash_only = msime::input::punctuation_marks_from_view("curly", "solid", false, true);
  assert(!is_space_conversion_key('\\', slash_only));
  assert(is_space_conversion_key('/', slash_only));
  assert(smart_punctuation_ascii_mark("、", '/', slash_only) == '/');
  assert(smart_punctuation_ascii_mark("、", '\\', slash_only) == '/');
  assert(smart_punctuation_ascii_mark("、", 0, slash_only) == '/');
  const auto both_keys = msime::input::punctuation_marks_from_view("curly", "solid", true, true);
  assert(smart_punctuation_ascii_mark("、", '/', both_keys) == '/');
  assert(smart_punctuation_ascii_mark("、", '\\', both_keys) == '\\');
  assert(smart_punctuation_ascii_mark("、", 0, both_keys) == '\\');
  // `/` 不再是顿号的正查键：关着时按 `/` 上屏的 ASCII 不能武装重复手势。
  assert(chinese_punctuation_mark('/').empty());
  assert(is_auto_paired_opening_key('"'));
  assert(is_auto_paired_opening_key('\''));
  assert(is_auto_paired_opening_key('('));
  assert(is_auto_paired_opening_key('['));
  assert(is_auto_paired_opening_key('<'));
  assert(!is_auto_paired_opening_key(')'));
  assert(!is_auto_paired_opening_key('/'));

  // Half width leaves the key alone; full width moves printable ASCII one block
  // up, which is what the host would have committed in the first place.
  assert(ascii_mark_text(',', false) == ",");
  assert(ascii_mark_text('.', false) == ".");
  assert(ascii_mark_text('.', true) == "\uff0e");
  assert(ascii_mark_text(':', true) == "\uff1a");
  // Ordinary repeat routing still recognises the width Engine committed.
  assert(ascii_mark_text(',', true) == chinese_punctuation_mark(','));
  assert(ascii_mark_text('.', true) != chinese_punctuation_mark('.'));
  // Nothing outside printable ASCII has a fullwidth form to offer.
  assert(ascii_mark_text('\n', true) == std::string(1, '\n'));

  // Recognising the host's own commit, so the repeat gesture can arm without a
  // second list of keys beside the table.
  assert(ascii_mark_from_text(",", false) == ',');
  assert(ascii_mark_from_text("．", true) == '.');
  assert(ascii_mark_from_text("a", false) == 0);
  assert(ascii_mark_from_text("", false) == 0);
  // A halfwidth comma is not what a fullwidth host committed, and the fullwidth
  // comma is - the same codepoint as the Chinese one, which is why the repeat
  // gesture is guarded by the key and the window rather than by the glyph alone.
  assert(ascii_mark_from_text(",", true) == 0);
  assert(ascii_mark_from_text("，", true) == ',');
  // Space conversion never consults the character-width mode.
  assert(space_conversion_ascii_text("，", 0, marks) == ",");
  assert(space_conversion_ascii_text("。", 0, marks) == ".");

  // The repeat check asks one question of the document: is that ASCII mark, in
  // the width the host committed it, still in front of the caret.
  assert(repeat_conversion_matches_document(',', false, {"a", ","}));
  assert(!repeat_conversion_matches_document(',', false, {"a", "."}));
  assert(!repeat_conversion_matches_document(',', false, {}));
  assert(repeat_conversion_matches_document('.', true, {"．"}));
  assert(!repeat_conversion_matches_document('.', true, {"."}));

  // English mode: plain keys pass through, fullwidth widens printable ASCII, and the Chinese punctuation lock converts marks ahead of width.
  EnglishPunctuationState english;
  assert(english_mode_output(U'a', false, false, false, english, {}).empty());
  assert(english_mode_output(U',', false, false, false, english, {}).empty());
  assert(english_mode_output(U'a', false, false, true, english, {}) == "ａ");
  assert(english_mode_output(U' ', false, false, true, english, {}) == "\u3000");
  assert(english_mode_output(U'1', true, false, true, english, {}) == "１");
  assert(english_mode_output(U'\t', false, true, true, english, {}).empty());
  assert(english_mode_output(U'\u00e9', false, true, true, english, {}).empty());
  assert(english_mode_output(U',', false, true, false, english, {}) == "，");
  assert(english_mode_output(U',', false, true, true, english, {}) == "，");
  assert(english_mode_output(U'a', false, true, false, english, {}).empty());
  assert(english_mode_output(U'a', false, true, true, english, {}) == "ａ");
  assert(english_mode_output(U'\\', false, true, false, english, {}) == "、");
  assert(english_mode_output(U'/', false, true, false, english, {}).empty());
  assert(english_mode_output(U'^', false, true, false, english, {}) == "……");
  assert(english_mode_output(U'$', false, true, false, english, {}) == "¥");
  assert(english_mode_output(U'$', false, true, true, english, {}) == "￥");
  assert(english_mode_output(U'$', false, false, false, english, {}).empty());
  assert(english_mode_output(U'$', false, false, true, english, {}) == "＄");
  // The keypad keeps its ASCII mark, then width applies.
  assert(english_mode_output(U'.', true, true, false, english, {}).empty());
  assert(english_mode_output(U'.', true, true, true, english, {}) == "．");
  // Quotes alternate and book titles nest, as in Engine's policy.
  assert(english_mode_output(U'"', false, true, false, english, {}) == "“");
  assert(english_mode_output(U'"', false, true, false, english, {}) == "”");
  assert(english_mode_output(U'\'', false, true, false, english, {}) == "‘");
  assert(english_mode_output(U'\'', false, true, false, english, {}) == "’");
  // 契约第 3 版：`{` `}` 出直角引号，交替状态与弯引号分开。
  assert(english_mode_output(U'{', false, true, false, english, {}) == "「");
  assert(english_mode_output(U'{', false, true, false, english, {}) == "」");
  assert(english_mode_output(U'}', false, true, false, english, {}) == "『");
  assert(english_mode_output(U'}', false, true, false, english, {}) == "』");
  assert(english_mode_output(U'<', false, true, false, english, {}) == "《");
  assert(english_mode_output(U'<', false, true, false, english, {}) == "〈");
  assert(english_mode_output(U'>', false, true, false, english, {}) == "〉");
  assert(english_mode_output(U'>', false, true, false, english, {}) == "》");
  assert(english_mode_output(U'>', false, true, false, english, {}) == "》");
  assert(english_mode_output(U'<', false, true, false, english, {}) == "《");
  // 直角引号与空心括号按 View 报告的样式出字，交替和缺省样式一样；样式不影响全角和其他键。
  const auto corner = msime::input::punctuation_marks_from_view("corner", "hollow");
  assert(corner.corner_quotes && corner.hollow_brackets);
  const auto defaults = msime::input::punctuation_marks_from_view("curly", "unknown");
  assert(!defaults.corner_quotes && !defaults.hollow_brackets);
  // 顿号键直接消费 View 的已解析开关，英文模式固定中文标点也必须保持一致。
  for (int mask = 0; mask < 4; ++mask) {
    const auto marks = msime::input::punctuation_marks_from_view("curly", "solid", mask & 1, mask & 2);
    EnglishPunctuationState enumeration;
    assert(english_mode_output(U'\\', false, true, false, enumeration, marks) == ((mask & 1) ? "、" : ""));
    assert(english_mode_output(U'/', false, true, false, enumeration, marks) == ((mask & 2) ? "、" : ""));
    assert(english_mode_output(U'/', false, false, false, enumeration, marks).empty());
    assert(english_mode_output(U'/', true, true, false, enumeration, marks).empty());
    if (!(mask & 1)) assert(english_mode_output(U'\\', false, true, true, enumeration, marks) == "＼");
  }
  EnglishPunctuationState styled;
  assert(english_mode_output(U'\'', false, true, false, styled, corner) == "「");
  assert(english_mode_output(U'"', false, true, false, styled, corner) == "『");
  assert(english_mode_output(U'\'', false, true, false, styled, corner) == "」");
  assert(english_mode_output(U'"', false, true, false, styled, corner) == "』");
  // `{` `}` 出的本来就是直角引号，样式不换它们。
  assert(english_mode_output(U'{', false, true, false, styled, corner) == "「");
  assert(english_mode_output(U'}', false, true, false, styled, corner) == "『");
  assert(english_mode_output(U'[', false, true, false, styled, corner) == "〖");
  assert(english_mode_output(U']', false, true, false, styled, corner) == "〗");
  assert(english_mode_output(U',', false, true, false, styled, corner) == "，");
  assert(english_mode_output(U'\'', false, false, true, styled, corner) == "＇");
  assert(english_mode_output(U'[', false, false, false, styled, corner).empty());

  // 空格转换的反查认直角引号和空心括号，转回按下的那个键。
  assert(smart_punctuation_ascii_mark("「", 0, marks) == '\'');
  assert(smart_punctuation_ascii_mark("」", 0, marks) == '\'');
  assert(smart_punctuation_ascii_mark("『", 0, marks) == '"');
  assert(smart_punctuation_ascii_mark("』", 0, marks) == '"');
  assert(smart_punctuation_ascii_mark("〖", 0, marks) == '[');
  assert(smart_punctuation_ascii_mark("〗", 0, marks) == ']');
  assert(space_conversion_ascii_text("「", 0, marks) == "'");
  // 正向表仍给缺省样式的字。
  assert(chinese_punctuation_mark('\'') == "‘");
  assert(chinese_punctuation_mark('[') == "【");

  // 英文模式的手抄表与生成的契约 policy.h 一致：契约加字改字时这张表必须跟，否则错位。
  namespace contract = metasequoia::punctuation_contract;
  const auto default_marks =
      msime::input::punctuation_marks_from_view("curly", "solid", true, false);
  for (const auto &row : contract::simple) {
    EnglishPunctuationState state;
    // `$` 是宿主有意的宽度分叉：半角写 `¥`（U+00A5）、全角才写契约的 `￥`（U+FFE5）。
    const auto expected = row.input == '$' ? "¥" : row.output;
    assert(english_mode_output(row.input, false, true, false, state, default_marks) == expected);
  }
  {
    EnglishPunctuationState state;
    assert(english_mode_output('$', false, true, true, state, default_marks) == "￥");
  }
  for (const auto &row : contract::alternating) {
    EnglishPunctuationState state;
    assert(english_mode_output(row.input, false, true, false, state, default_marks) == row.opening);
    assert(english_mode_output(row.input, false, true, false, state, default_marks) == row.closing);
  }
  {
    // 嵌套书名号：同一个 state 开两层再关两层，各层的字由契约的 nested_* 常量钉住。
    EnglishPunctuationState state;
    assert(english_mode_output(contract::nested_opening_input, false, true, false, state, default_marks) ==
           contract::nested_opening);
    assert(english_mode_output(contract::nested_opening_input, false, true, false, state, default_marks) ==
           contract::nested_opening_inner);
    assert(english_mode_output(contract::nested_closing_input, false, true, false, state, default_marks) ==
           contract::nested_closing_inner);
    assert(english_mode_output(contract::nested_closing_input, false, true, false, state, default_marks) ==
           contract::nested_closing);
  }
  {
    const auto styled_marks =
        msime::input::punctuation_marks_from_view("corner", "hollow", true, true);
    for (const auto &row : contract::corner_quotes) {
      EnglishPunctuationState state;
      assert(english_mode_output(row.input, false, true, false, state, styled_marks) == row.opening);
      assert(english_mode_output(row.input, false, true, false, state, styled_marks) == row.closing);
    }
    for (const auto &row : contract::hollow_brackets) {
      EnglishPunctuationState state;
      assert(english_mode_output(row.input, false, true, false, state, styled_marks) == row.output);
    }
    // 顿号两键都在 enumeration_comma 段：`/` 键只有开着时才出顿号。
    EnglishPunctuationState backslash_state, slash_state;
    assert(english_mode_output('\\', false, true, false, backslash_state, styled_marks) == "、");
    assert(english_mode_output('/', false, true, false, slash_state, styled_marks) == "、");
    EnglishPunctuationState off_state;
    assert(english_mode_output('/', false, true, false, off_state, default_marks).empty());
  }
  return 0;
}
