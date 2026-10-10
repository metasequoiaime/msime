// 日文释义行的罗马字，和 macOS 的 MSIMEJapaneseRomaji 同样逐词读、助词按发音读、读不全就不标。
#include "JapaneseRomaji.h"
#include <cstdio>
#include <cstdlib>

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
  // 平文式：し chi つ ふ じ，拗音，促音，拨音，长音。
  require(kana_romaji(u"ねこ") == "neko", "plain");
  require(kana_romaji(u"しちつふじ") == "shichitsufuji", "hepburn consonants");
  require(kana_romaji(u"きょう") == "kyou", "youon");
  require(kana_romaji(u"しゃしん") == "shashin", "sh youon and final n");
  require(kana_romaji(u"ちゃ") == "cha" && kana_romaji(u"じゅ") == "ju", "ch and j youon");
  require(kana_romaji(u"きって") == "kitte", "sokuon");
  require(kana_romaji(u"まっちゃ") == "matcha", "sokuon before ch");
  require(kana_romaji(u"きんえん") == "kin'en" && kana_romaji(u"こんや") == "kon'ya", "n before vowel and y");
  require(kana_romaji(u"しんぶん") == "shinbun", "n before consonant");
  require(kana_romaji(u"とうきょう") == "toukyou", "long vowels spelled out");
  // 片假名按同样的规则，ー 重复前一个元音，外来音按常见写法。
  require(kana_romaji(u"コーヒー") == "koohii", "katakana long mark");
  require(kana_romaji(u"ファイル") == "fairu" && kana_romaji(u"パーティー") == "paatii", "loanword sounds");
  require(kana_romaji(u"ウィキ") == "wiki" && kana_romaji(u"チェック") == "chekku", "wi and che");
  require(kana_romaji(u"ヴァイオリン") == "vaiorin", "va");
  require(kana_romaji(u"ジョン・スミス") == "jon sumisu", "middle dot");
  // ASCII 字母和数字照抄；读不出来的字（汉字）让整串为空。
  require(kana_romaji(u"Tシャツ") == "Tshatsu", "ascii passes through");
  require(kana_romaji(u"猫").empty() && kana_romaji(u"ねこ猫").empty(), "kanji is unreadable");
  require(kana_romaji(u"").empty(), "empty");

  require(japanese_kana_only(u"ありがとう") && japanese_kana_only(u"コーヒー"), "kana only");
  require(!japanese_kana_only(u"今日は") && !japanese_kana_only(u"ー") && !japanese_kana_only(u""), "not kana only");

  // 逐词读，词间一个空格，助词按发音；标点不读。
  const std::vector<JapaneseWord> sentence = {{u"今日", u"きょう"}, {u"は", u"は"},     {u"天気", u"てんき"},
                                              {u"が", u"が"},       {u"いい", u"いい"}, {u"です", u"です"},
                                              {u"ね", u"ね"},       {u"。", u"。"}};
  require(japanese_romaji(sentence) == "kyou wa tenki ga ii desu ne", "sentence");
  require(japanese_romaji({{u"こんにちは", u"こんにちは"}}) == "konnichiwa", "greeting");
  require(japanese_romaji({{u"こんばんは", u"こんばんは"}}) == "konbanwa", "evening greeting");
  require(japanese_romaji({{u"学校", u"がっこう"}, {u"へ", u"へ"}}) == "gakkou e", "particle he");
  require(japanese_romaji({{u"本", u"ほん"}, {u"を", u"を"}}) == "hon o", "particle wo");
  // 一个词读不出来，整行不标。
  require(japanese_romaji({{u"猫", u"ねこ"}, {u"犬", u"犬"}}).empty(), "partial reading is not shown");
  require(japanese_romaji({}).empty() && japanese_romaji({{u"、", u"、"}}).empty(), "nothing to read");

  // IFELanguage 反查的结果：写法取原文（pwchComp），读音取输出（pwchOutput）。取反了的话写法是假名、读音是汉字，含汉字的行一律读不出来。
  {
    const std::u16string comp = u"今日は天気がいいですね";
    const std::u16string output = u"きょうはてんきがいいですね";
    const std::vector<JapaneseMorphWord> morphs = {{0, 3, 0, 2}, {3, 1, 2, 1},  {4, 3, 3, 2},  {7, 1, 5, 1},
                                                   {8, 2, 6, 2}, {10, 2, 8, 2}, {12, 1, 10, 1}};
    const auto words = japanese_reverse_words(comp, output, morphs);
    require(words.size() == morphs.size() && words.front().surface == u"今日" && words.front().reading == u"きょう",
            "reverse conversion: surface from comp, reading from output");
    require(japanese_romaji(words) == "kyou wa tenki ga ii desu ne", "reverse conversion sentence");
    // 越界的段整行作废。
    require(japanese_reverse_words(comp, output, {{0, 3, 10, 2}}).empty(), "comp out of range");
    require(japanese_reverse_words(comp, output, {{12, 2, 0, 1}}).empty(), "output out of range");
    require(japanese_reverse_words(comp, output, {{0, 3, 0, 0}}).empty(), "empty surface");
  }

  require(japanese_utf16("今日は") == u"今日は", "utf8 to utf16");
  require(japanese_utf16("\xF0\x9F\x98\x80") == u"\U0001F600", "supplementary plane");
  require(japanese_utf16("\xE4\xBB").empty(), "truncated utf8");
  std::puts("Japanese romaji: Hepburn per word, particles as spoken");
  return 0;
}
