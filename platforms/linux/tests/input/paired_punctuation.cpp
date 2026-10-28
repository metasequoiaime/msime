#include "../src/candidates/PairedPunctuation.h"

#include <cassert>
#include <optional>
#include <string>

int main() {
  using msime::linux_host::PairedPunctuationTracker;
  using msime::linux_host::PairedPunctuationModifier;
  using msime::linux_host::paired_closing_modifiers_allowed;
  using msime::linux_host::paired_closing_for_key;
  using msime::linux_host::paired_punctuation_excluded_client;

  PairedPunctuationTracker tracker;
  tracker.push("）");
  tracker.push("】");
  assert(tracker.size() == 2);
  assert(tracker.matches("】"));
  assert(!tracker.consume("）", "）", true));
  assert(tracker.empty());

  tracker.push("｝");
  assert(!tracker.consume("｝", "}", true));
  assert(tracker.empty());
  tracker.push("”");
  assert(tracker.consume("”", "”", true));
  assert(tracker.empty());
  tracker.push("〉");
  assert(!tracker.consume("〉", "", false));
  assert(tracker.empty());

  for (std::size_t index = 0; index < PairedPunctuationTracker::kMaxDepth + 3;
       ++index)
    tracker.push("）");
  assert(tracker.size() == PairedPunctuationTracker::kMaxDepth);

  tracker.clear();
  assert(paired_closing_for_key('"', false, tracker) == "”");
  assert(paired_closing_for_key('\'', false, tracker) == "’");
  assert(paired_closing_for_key(')', false, tracker) == "）");
  assert(paired_closing_for_key(']', false, tracker) == "】");
  assert(paired_closing_for_key('}', false, tracker) == "}");
  assert(paired_closing_for_key('}', true, tracker) == "｝");
  // 契约第 5 版：中文标点下 `{` 补出的是」，栈顶是」时 `}` 跨过它，全角开关不影响。
  tracker.push("」");
  assert(paired_closing_for_key('}', false, tracker) == "」");
  assert(paired_closing_for_key('}', true, tracker) == "」");
  assert(tracker.consume(*paired_closing_for_key('}', true, tracker), "」", true));
  assert(tracker.empty());
  assert(!paired_closing_for_key(',', false, tracker));
  // 直角引号与空心括号：栈顶是这个键的另一组后半个时跨过它。
  tracker.push("」");
  assert(paired_closing_for_key('\'', false, tracker) == "」");
  assert(paired_closing_for_key('"', false, tracker) == "”");
  tracker.push("』");
  assert(paired_closing_for_key('"', false, tracker) == "』");
  tracker.push("〗");
  assert(paired_closing_for_key(']', false, tracker) == "〗");
  assert(tracker.consume("〗", "〗", true));
  assert(tracker.consume(*paired_closing_for_key('"', false, tracker), "』", true));
  assert(tracker.consume(*paired_closing_for_key('\'', false, tracker), "」", true));
  assert(tracker.empty());
  for (std::size_t index = 0; index < PairedPunctuationTracker::kMaxDepth + 3;
       ++index)
    tracker.push("）");
  assert(paired_punctuation_excluded_client("/usr/bin/scalc"));
  assert(paired_punctuation_excluded_client("scalc.bin"));
  assert(paired_punctuation_excluded_client("GNUMERIC"));
  assert(!paired_punctuation_excluded_client("soffice.bin"));
  assert(!paired_punctuation_excluded_client("org.gnome.TextEditor"));

  constexpr auto control =
      static_cast<unsigned>(PairedPunctuationModifier::Control);
  constexpr auto alt = static_cast<unsigned>(PairedPunctuationModifier::Alt);
  constexpr auto super =
      static_cast<unsigned>(PairedPunctuationModifier::Super);
  assert(paired_closing_modifiers_allowed(0));
  assert(paired_closing_modifiers_allowed(
      static_cast<unsigned>(PairedPunctuationModifier::Shift)));
  assert(!paired_closing_modifiers_allowed(control));
  assert(!paired_closing_modifiers_allowed(alt | super));

  tracker.push("）");
  assert(!tracker.consume("）", "）", true, false));
  assert(tracker.matches("）"));
  assert(tracker.consume("）", "）", true));
  {
    using msime::linux_host::normalize_punctuation_pair;
    using msime::linux_host::paired_closing_from_text;
    using Mode = msime::linux_host::PunctuationPairMode;
    std::string text = "（";
    assert(normalize_punctuation_pair(text, Mode::Bracket) && text == "（）");
    assert(paired_closing_from_text(text) == std::optional<std::string>("）"));
    text = "你好《";
    assert(normalize_punctuation_pair(text, Mode::Bracket) && text == "你好《》");
    // 契约第 5 版：`{` 在中文标点下出「，补成「」，跨过时认」。
    text = "「";
    assert(normalize_punctuation_pair(text, Mode::Brace) && text == "「」");
    assert(paired_closing_from_text(text) == std::optional<std::string>("」"));
    text = "你好「";
    assert(normalize_punctuation_pair(text, Mode::Brace) && text == "你好「」");
    // `}` 出的」不是 `{` 的前半个，不补。
    text = "」";
    assert(!normalize_punctuation_pair(text, Mode::Brace) && text == "」");
    // 引擎写 ASCII 标点时（专用英文）`{` 仍补成花括号。
    text = "{";
    assert(normalize_punctuation_pair(text, Mode::Brace) && text == "{}");
    text = "｛";
    assert(normalize_punctuation_pair(text, Mode::Brace) && text == "｛｝");
    // Engine alternates quote halves; either one becomes a complete pair.
    text = "”";
    assert(normalize_punctuation_pair(text, Mode::DoubleQuote) && text == "“”");
    text = "‘";
    assert(normalize_punctuation_pair(text, Mode::SingleQuote) && text == "‘’");
    // 直角引号：引擎写出哪一组的哪一半，就补成那一组完整的一对，并能从文本取回后半个。
    text = "「";
    assert(normalize_punctuation_pair(text, Mode::SingleQuote) && text == "「」");
    assert(paired_closing_from_text(text) == std::optional<std::string>("」"));
    text = "你好』";
    assert(normalize_punctuation_pair(text, Mode::DoubleQuote) && text == "你好『』");
    assert(paired_closing_from_text(text) == std::optional<std::string>("』"));
    text = "」";
    assert(!normalize_punctuation_pair(text, Mode::DoubleQuote) && text == "」");
    text = "〖";
    assert(normalize_punctuation_pair(text, Mode::Bracket) && text == "〖〗");
    assert(paired_closing_from_text(text) == std::optional<std::string>("〗"));
    text = "(";
    assert(!normalize_punctuation_pair(text, Mode::Bracket) && text == "(");
    assert(!normalize_punctuation_pair(text, Mode::Unpaired));
  }
  return 0;
}
