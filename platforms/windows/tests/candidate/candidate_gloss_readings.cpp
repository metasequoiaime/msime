// 候选释义读音的显示规则，和 macOS 的 CandidatePronunciation.h 逐条对应。
#include "CandidateGlossReadings.h"
#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>
#include <utility>
#include <vector>

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
  require(gloss_line_language("chat", 0, {"fr"}) == "fr", "french is read as french, not english");
  require(gloss_line_language("chat", 0, {"fr", "en"}) == "fr", "only the english target's line is english");
  require(gloss_line_language("кошка", 1, {"en", "ru"}) == "ru", "a line follows its own target");
  require(gloss_line_language("chat", 0, {"zh"}).empty(), "a target without a table is not read");
  require(gloss_line_language("こんにちは", 0, {"en"}) == "ja", "kana anywhere");
  require(gloss_line_language("今日", 1, targets) == "ja", "kanji on the japanese target");
  require(gloss_line_language("今日", 0, targets).empty(), "kanji on the english target");

  // 英文候选读它自己；中文候选读它的英文释义行；没有释义时什么都不问。
  using Items = std::vector<std::pair<std::string, std::string>>;
  require(gloss_shared_items("hello", {"你好"}, {"en"}) == Items{{"en", "hello"}}, "english candidate");
  require(gloss_shared_items("你好", {"hello; hi", "こんにちは"}, targets) == Items{{"en", "hello; hi"}},
          "english gloss line");
  require(gloss_shared_items("猫", {"chat", "猫", "Katze"}, {"fr", "ja", "de"}) ==
              Items{{"fr", "chat"}, {"de", "Katze"}},
          "other target lines go in their language; japanese is read locally");
  require(gloss_shared_items("hello", {""}, {"en"}).empty(), "no gloss");

  const std::map<std::pair<std::string, std::string>, std::string> table{
      {{"en", "hello; hi"}, "/həˈləʊ/"}, {{"en", "hello"}, "/həˈləʊ/"}, {{"fr", "chat"}, "/ʃa/"}};
  const auto english = [&](const std::string &language, const std::string &text) {
    const auto found = table.find({language, text});
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
  require(gloss_pronunciation_lines("猫", {"chat"}, {"fr"}, english) == std::vector<std::string>{"/ʃa/"},
          "french line reading");
  require(gloss_pronunciation_lines("猫", {"chat"}, {"en"}, english).empty(), "a french answer is not english");
  require(gloss_pronunciation_lines("hello", {"你好"}, {"en"}, english) ==
              std::vector<std::string>{"/həˈləʊ/"},
          "english candidate reading");

  // 画出来的释义：读音在它那一行释义下面单独一行，和 macOS 的 MSIMECandidateGlossDisplay 一样；读音的位置和每行释义的起点都报出来。
  std::vector<std::pair<size_t, size_t>> ranges;
  std::vector<size_t> starts;
  require(candidate_gloss_display({"hello; hi", "こんにちは"}, readings, &ranges, &starts) ==
              "hello; hi\n/həˈləʊ/\nこんにちは",
          "reading on its own line");
  require(ranges.size() == 1 && ranges[0].first == std::string("hello; hi\n").size() &&
              ranges[0].second == std::string("/həˈləʊ/").size(),
          "reading range");
  require(starts == std::vector<size_t>{0, std::string("hello; hi\n/həˈləʊ/\n").size()},
          "second gloss line starts after the reading");
  // 两行都有读音：每条读音跟在自己那一行下面。
  ranges.clear();
  starts.clear();
  require(candidate_gloss_display({"hello", "こんにちは"}, {"/həˈləʊ/", "konnichiwa"}, &ranges, &starts) ==
              "hello\n/həˈləʊ/\nこんにちは\nkonnichiwa",
          "both readings");
  require(ranges.size() == 2 && ranges[1].first == std::string("hello\n/həˈləʊ/\nこんにちは\n").size(),
          "second reading range");
  require(starts.size() == 2 && starts[1] == std::string("hello\n/həˈləʊ/\n").size(), "both line starts");
  // 空的释义行不生读音行，第二种目标语言照样排在第二行；没有读音时就是释义各行。
  starts.clear();
  require(candidate_gloss_display({"", "aimer"}, {"/x/", ""}, nullptr, &starts) == "\naimer",
          "empty first line kept without a reading line");
  require(starts == std::vector<size_t>{0, 1}, "empty line start");
  require(candidate_gloss_display({"hello", "bonjour"}, {}) == "hello\nbonjour", "no readings");
}
