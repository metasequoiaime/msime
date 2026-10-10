// 候选释义读音和逐词拆解的显示规则，和 macOS 的 CandidatePronunciation.h 逐条对应。
#include "CandidateGlossReadings.h"
#include <cstdio>
#include <cstdlib>
#include <map>

using namespace msime::windows;

namespace {
void require(bool value, const char *what) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    std::exit(EXIT_FAILURE);
  }
}
} // namespace

int main() {
  // 每行按第一个分隔符前的词读，去掉两端空白。
  require(gloss_first_term("  hello ; hi") == "hello", "ascii separator");
  require(gloss_first_term("你好，您好") == "你好", "fullwidth comma");
  require(gloss_first_term("to like/to love") == "to like", "slash");
  require(gloss_first_term("こんにちは、やあ") == "こんにちは", "ideographic comma");
  require(gloss_first_term("\xE3\x80\x80word\xC2\xA0") == "word", "unicode spaces");
  require(gloss_first_term("").empty() && gloss_first_term(" ; x").empty(), "empty term");

  require(gloss_plain_english("don't stop") && gloss_plain_english("well-being"), "english");
  require(!gloss_plain_english("café") && !gloss_plain_english("123") && !gloss_plain_english(" "),
          "not english");

  // 有假名就是日文；拉丁字母只在英文目标那一行是英文；只有汉字的行只在日文目标那一行是日文。
  const std::vector<std::string> targets{"en", "ja"};
  require(gloss_line_language("hello; hi", 0, targets) == "en", "english line");
  require(gloss_line_language("chat", 0, {"fr"}).empty(), "french is not english");
  require(gloss_line_language("こんにちは", 0, {"en"}) == "ja", "kana anywhere");
  require(gloss_line_language("今日", 1, targets) == "ja", "kanji on the japanese target");
  require(gloss_line_language("今日", 0, targets).empty(), "kanji on the english target");

  require(gloss_sentence_candidate("我喜欢你"), "sentence");
  require(!gloss_sentence_candidate("我"), "single character");
  require(!gloss_sentence_candidate("我a"), "mixed");
  require(!gloss_sentence_candidate(""), "empty");
  std::string long_sentence;
  for (int index = 0; index < 33; ++index)
    long_sentence += "字";
  require(!gloss_sentence_candidate(long_sentence), "too long");
  long_sentence.resize(32 * 3);
  require(gloss_sentence_candidate(long_sentence), "32 characters");

  // 英文候选读它自己；中文候选读它的英文释义行；没有释义时什么都不问。
  require(gloss_english_texts("hello", {"你好"}, {"en"}) == std::vector<std::string>{"hello"},
          "english candidate");
  require(gloss_english_texts("你好", {"hello; hi", "こんにちは"}, targets) ==
              std::vector<std::string>{"hello; hi"},
          "english gloss line");
  require(gloss_english_texts("hello", {""}, {"en"}).empty(), "no gloss");

  const std::map<std::string, std::string> table{{"hello; hi", "/həˈləʊ/"}, {"hello", "/həˈləʊ/"}};
  const auto english = [&](const std::string &text) {
    const auto found = table.find(text);
    return found == table.end() ? std::string{} : found->second;
  };
  const auto readings =
      gloss_pronunciation_lines("你好", {"hello; hi", "こんにちは"}, targets, english);
  require(readings.size() == 2 && readings[0] == "/həˈləʊ/" && readings[1].empty(),
          "per-line readings");
  // 给了日文读法时，日文行按第一个词读成罗马字；读不出来的行留空。
  const auto japanese = [](const std::string &term) {
    return term == "こんにちは" ? std::string("konnichiwa") : std::string{};
  };
  const auto with_romaji =
      gloss_pronunciation_lines("你好", {"hello; hi", "こんにちは、やあ"}, targets, english, japanese);
  require(with_romaji.size() == 2 && with_romaji[0] == "/həˈləʊ/" && with_romaji[1] == "konnichiwa",
          "japanese line reading");
  require(gloss_pronunciation_lines("猫", {"猫"}, {"ja"}, english, japanese).empty(),
          "unreadable japanese line");
  require(gloss_pronunciation_lines("你好", {"bonjour"}, {"fr"}, english).empty(), "no reading");
  require(gloss_pronunciation_lines("hello", {"你好"}, {"en"}, english) ==
              std::vector<std::string>{"/həˈləʊ/"},
          "english candidate reading");

  // 画出来的释义：每行后面空两格跟读音，拆解另起最后一行。
  require(candidate_gloss_display({"hello; hi", "こんにちは"}, readings, "") ==
              "hello; hi  /həˈləʊ/\nこんにちは",
          "display with readings");
  require(candidate_gloss_display({}, {}, "我 I · 喜欢 to like · 你 you") ==
              "我 I · 喜欢 to like · 你 you",
          "breakdown alone");
  require(candidate_gloss_display({"", "aimer"}, {}, "我 I · 你 you") ==
              "\naimer\n我 I · 你 you",
          "empty first line kept");
}
