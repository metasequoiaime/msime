#pragma once
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
// 日文释义行的罗马字，对应 macOS CandidatePronunciation.h 的 MSIMEJapaneseRomaji：逐词读，词与词之间一个空格（今日は天気がいいですね 读作 kyou wa tenki ga ii desu ne），助词 は、へ、を 和问候语末尾的 は 按实际发音读作 wa、e、o；有一个词读不出来就整行不标，半个读音不显示。macOS 的分词和读音来自系统分词器；Windows 的来自微软日语输入法的 IFELanguage（JapaneseReader.cpp），它给出每个词的平假名读音，这里把读音按平文式（Hepburn）转成罗马字。长音不加长音符号，コーヒー 写作 koohii，与 kyou、toukyou 的写法一致。纯计算，不依赖 Windows 头文件，主机上的单测直接包含。

// 一个词：它在释义里的写法和它的假名读音（UTF-16，与 IFELanguage 给的一样）。
struct JapaneseWord {
  std::u16string surface;
  std::u16string reading;
};

namespace japanese_romaji_detail {
inline bool hiragana(char16_t c) { return c >= u'ぁ' && c <= u'ゖ'; }
inline bool katakana(char16_t c) { return c >= u'ァ' && c <= u'ヶ'; }
// 片假名换成对应的平假名，其他不变。
inline char16_t fold(char16_t c) { return katakana(c) ? static_cast<char16_t>(c - 0x60) : c; }

// 一个平假名单独读的罗马字；小写的 ゃ ゅ ょ ぁ ぃ ぅ ぇ ぉ 和 っ 由调用方按前后文处理，不在这里。
inline const char *syllable(char16_t c) {
  switch (c) {
  case u'あ': return "a";   case u'い': return "i";   case u'う': return "u";   case u'え': return "e";   case u'お': return "o";
  case u'か': return "ka";  case u'き': return "ki";  case u'く': return "ku";  case u'け': return "ke";  case u'こ': return "ko";
  case u'が': return "ga";  case u'ぎ': return "gi";  case u'ぐ': return "gu";  case u'げ': return "ge";  case u'ご': return "go";
  case u'さ': return "sa";  case u'し': return "shi"; case u'す': return "su";  case u'せ': return "se";  case u'そ': return "so";
  case u'ざ': return "za";  case u'じ': return "ji";  case u'ず': return "zu";  case u'ぜ': return "ze";  case u'ぞ': return "zo";
  case u'た': return "ta";  case u'ち': return "chi"; case u'つ': return "tsu"; case u'て': return "te";  case u'と': return "to";
  case u'だ': return "da";  case u'ぢ': return "ji";  case u'づ': return "zu";  case u'で': return "de";  case u'ど': return "do";
  case u'な': return "na";  case u'に': return "ni";  case u'ぬ': return "nu";  case u'ね': return "ne";  case u'の': return "no";
  case u'は': return "ha";  case u'ひ': return "hi";  case u'ふ': return "fu";  case u'へ': return "he";  case u'ほ': return "ho";
  case u'ば': return "ba";  case u'び': return "bi";  case u'ぶ': return "bu";  case u'べ': return "be";  case u'ぼ': return "bo";
  case u'ぱ': return "pa";  case u'ぴ': return "pi";  case u'ぷ': return "pu";  case u'ぺ': return "pe";  case u'ぽ': return "po";
  case u'ま': return "ma";  case u'み': return "mi";  case u'む': return "mu";  case u'め': return "me";  case u'も': return "mo";
  case u'や': return "ya";  case u'ゆ': return "yu";  case u'よ': return "yo";
  case u'ら': return "ra";  case u'り': return "ri";  case u'る': return "ru";  case u'れ': return "re";  case u'ろ': return "ro";
  case u'わ': return "wa";  case u'ゐ': return "i";   case u'ゑ': return "e";   case u'を': return "o";   case u'ゔ': return "vu";
  case u'ゎ': return "wa";  case u'ゕ': return "ka";  case u'ゖ': return "ke";
  case u'ぁ': return "a";   case u'ぃ': return "i";   case u'ぅ': return "u";   case u'ぇ': return "e";   case u'ぉ': return "o";
  case u'ゃ': return "ya";  case u'ゅ': return "yu";  case u'ょ': return "yo";
  default: return nullptr;
  }
}

inline bool small_y(char16_t c) { return c == u'ゃ' || c == u'ゅ' || c == u'ょ'; }
inline bool small_vowel(char16_t c) { return c == u'ぁ' || c == u'ぃ' || c == u'ぅ' || c == u'ぇ' || c == u'ぉ'; }
inline bool vowel(char c) { return c == 'a' || c == 'i' || c == 'u' || c == 'e' || c == 'o'; }
inline bool ends_with(const std::string &text, std::string_view suffix) {
  return text.size() >= suffix.size() && text.compare(text.size() - suffix.size(), suffix.size(), suffix) == 0;
}

// 一个音节和跟在后面的小写假名合成的读法：きゃ kya、しゃ sha、ちぇ che、ふぁ fa、てぃ ti、うぃ wi 这些。合不成时返回空串，由调用方把小写假名单独读出来。
inline std::string combine(const std::string &base, char16_t small) {
  const std::string tail = syllable(small);
  if (small_y(small)) {
    // し ち じ ぢ 后面直接接元音（sha、cho、ju），其他 い 段音换成 y（kya、nyu），ふ て で 也能接（fyu、tyu、dyu）。
    if (base == "shi" || base == "chi" || base == "ji")
      return base.substr(0, base.size() - 1) + tail.substr(1);
    if (base.size() >= 2 && ends_with(base, "i"))
      return base.substr(0, base.size() - 1) + tail;
    if (base == "fu" || base == "te" || base == "de")
      return base.substr(0, 1) + tail;
    return {};
  }
  const std::string v = tail;
  if (base == "fu" || base == "vu" || base == "tsu")
    return base.substr(0, base.size() - 1) + v;
  if ((base == "te" || base == "de") && v == "i")
    return base.substr(0, 1) + v;
  if ((base == "to" || base == "do") && v == "u")
    return base.substr(0, 1) + v;
  if (base == "u" && (v == "i" || v == "e" || v == "o"))
    return "w" + v;
  if ((base == "shi" || base == "chi" || base == "ji") && v == "e")
    return base.substr(0, base.size() - 1) + v;
  if (base == "i" && v == "e")
    return "ye";
  if (base == "ku" && (v == "a" || v == "i" || v == "e" || v == "o"))
    return "kw" + v;
  if (base == "gu" && (v == "a" || v == "i" || v == "e" || v == "o"))
    return "gw" + v;
  return {};
}
} // namespace japanese_romaji_detail

// 一串假名（平假名或片假名，可以混着 ASCII 字母和数字）的平文式罗马字。ん 在元音和 y 前写作 n'，っ 双写下一个辅音（ch 前写 t），ー 重复前一个元音，・ 写作空格；其他字符读不出来，返回空串。
inline std::string kana_romaji(std::u16string_view kana) {
  using namespace japanese_romaji_detail;
  std::string out;
  bool geminate = false;
  bool pending_n = false;
  const auto append = [&](const std::string &part) {
    if (pending_n) {
      out += (vowel(part.front()) || part.front() == 'y') ? "n'" : "n";
      pending_n = false;
    }
    if (geminate) {
      if (part.compare(0, 2, "ch") == 0)
        out.push_back('t');
      else if (!vowel(part.front()))
        out.push_back(part.front());
      geminate = false;
    }
    out += part;
  };
  for (size_t index = 0; index < kana.size(); ++index) {
    const char16_t c = fold(kana[index]);
    if (c == u'ん') {
      if (pending_n)
        append("n");
      pending_n = true;
      continue;
    }
    if (c == u'っ') {
      geminate = true;
      continue;
    }
    if (c == u'ー') {
      if (pending_n || out.empty() || !vowel(out.back()))
        return {};
      out.push_back(out.back());
      continue;
    }
    if (c == u'・' || c == u' ' || c == u'　') {
      if (pending_n) {
        out.push_back('n');
        pending_n = false;
      }
      geminate = false;
      if (!out.empty() && out.back() != ' ')
        out.push_back(' ');
      continue;
    }
    if ((c >= u'a' && c <= u'z') || (c >= u'A' && c <= u'Z') || (c >= u'0' && c <= u'9')) {
      append(std::string(1, static_cast<char>(c)));
      continue;
    }
    const char *base = syllable(c);
    if (!base)
      return {};
    std::string part = base;
    if (index + 1 < kana.size()) {
      const char16_t next = fold(kana[index + 1]);
      if (!small_y(c) && !small_vowel(c) && (small_y(next) || small_vowel(next))) {
        if (auto combined = combine(part, next); !combined.empty()) {
          part = std::move(combined);
          ++index;
        }
      }
    }
    append(part);
  }
  if (pending_n)
    out.push_back('n');
  while (!out.empty() && out.back() == ' ')
    out.pop_back();
  return out;
}

// 只由假名（含 ー 和 ・）组成、至少一个假名的文字：没有日语输入法时也能直接读。
inline bool japanese_kana_only(std::u16string_view text) {
  using namespace japanese_romaji_detail;
  bool kana = false;
  for (const char16_t c : text) {
    if (hiragana(c) || katakana(c))
      kana = true;
    else if (c != u'ー' && c != u'・')
      return false;
  }
  return kana;
}

// 只有标点、符号和空白的词不读，也不算读不出来，和 macOS 分词器跳过标点一样。
inline bool japanese_word_silent(std::u16string_view surface) {
  for (const char16_t c : surface) {
    const bool ascii_punctuation = c < 0x80 && !((c >= u'a' && c <= u'z') || (c >= u'A' && c <= u'Z') ||
                                                 (c >= u'0' && c <= u'9'));
    const bool cjk_punctuation = (c >= 0x3000 && c <= 0x303F) || (c >= 0xFF01 && c <= 0xFF0F) ||
                                 (c >= 0xFF1A && c <= 0xFF20) || (c >= 0xFF3B && c <= 0xFF40) ||
                                 (c >= 0xFF5B && c <= 0xFF65) || c == u'・' || c == u' ';
    if (!ascii_punctuation && !cjk_punctuation)
      return false;
  }
  return true;
}

// 逐词的罗马字，词与词之间一个空格。助词 は、へ、を 和 こんにちは、こんばんは 按发音读；有一个词读不出来就返回空串。
inline std::string japanese_romaji(const std::vector<JapaneseWord> &words) {
  std::string out;
  for (const auto &word : words) {
    if (japanese_word_silent(word.surface))
      continue;
    std::string romaji;
    if (word.surface == u"は")
      romaji = "wa";
    else if (word.surface == u"へ")
      romaji = "e";
    else if (word.surface == u"を")
      romaji = "o";
    else if (word.surface == u"こんにちは")
      romaji = "konnichiwa";
    else if (word.surface == u"こんばんは")
      romaji = "konbanwa";
    else
      romaji = kana_romaji(word.reading);
    if (romaji.empty())
      return {};
    if (!out.empty())
      out.push_back(' ');
    out += romaji;
  }
  return out;
}

// IFELanguage 反查（FELANG_REQ_REV）结果里的一个词，即 msime.h 的 WDD：输出串 pwchOutput 是转换结果，反查时就是平假名读音，wDispPos/cchDisp 指向它；原文在 pwchComp（与 pwchRead 同一个联合），wCompPos/cchComp（与 wReadPos/cchRead 同一个联合）指向它。
struct JapaneseMorphWord {
  size_t output_position = 0;
  size_t output_length = 0;
  size_t comp_position = 0;
  size_t comp_length = 0;
};

// 把反查结果拆成词：写法取原文 comp 里的一段，读音取输出 output 里的一段。正向转换（假名转汉字）时两者正好反过来，这里只用于反查。有一段越界或为空就返回空列表，整行不标。
inline std::vector<JapaneseWord> japanese_reverse_words(std::u16string_view comp, std::u16string_view output,
                                                        const std::vector<JapaneseMorphWord> &morphs) {
  std::vector<JapaneseWord> words;
  words.reserve(morphs.size());
  for (const auto &morph : morphs) {
    if (morph.comp_length == 0 || morph.comp_position > comp.size() ||
        morph.comp_length > comp.size() - morph.comp_position || morph.output_position > output.size() ||
        morph.output_length > output.size() - morph.output_position)
      return {};
    words.push_back({std::u16string(comp.substr(morph.comp_position, morph.comp_length)),
                     std::u16string(output.substr(morph.output_position, morph.output_length))});
  }
  return words;
}

// UTF-8 和 UTF-16 之间的转换，释义行是 UTF-8，IFELanguage 收发 UTF-16。不合法的 UTF-8 转出空串。
inline std::u16string japanese_utf16(std::string_view utf8) {
  std::u16string out;
  for (size_t index = 0; index < utf8.size();) {
    const auto byte = static_cast<unsigned char>(utf8[index]);
    const size_t length = byte < 0x80 ? 1 : (byte >> 5) == 0x6 ? 2 : (byte >> 4) == 0xE ? 3 : (byte >> 3) == 0x1E ? 4 : 0;
    if (length == 0 || index + length > utf8.size())
      return {};
    char32_t value = length == 1 ? byte : length == 2 ? (byte & 0x1F) : length == 3 ? (byte & 0x0F) : (byte & 0x07);
    for (size_t offset = 1; offset < length; ++offset) {
      const auto continuation = static_cast<unsigned char>(utf8[index + offset]);
      if ((continuation & 0xC0) != 0x80)
        return {};
      value = (value << 6) | (continuation & 0x3F);
    }
    index += length;
    if (value >= 0x10000) {
      value -= 0x10000;
      out.push_back(static_cast<char16_t>(0xD800 + (value >> 10)));
      out.push_back(static_cast<char16_t>(0xDC00 + (value & 0x3FF)));
    } else {
      out.push_back(static_cast<char16_t>(value));
    }
  }
  return out;
}
} // namespace msime::windows
