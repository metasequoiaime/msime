#pragma once
#include <cstdint>
#include <functional>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

namespace msime::windows {
// 候选释义每一行后面画的读音，只用于显示，从不上屏。规则移植自 macOS 的 platforms/macos/src/candidate/CandidatePronunciation.h：释义每种目标语言一行，每行按它的第一个词读；英文、法文、西班牙文、俄文、德文和韩文走共享的离线读音表（msime_client_pronunciation_request，每种语言一张表），日文行读罗马字，macOS 的来自系统分词器，Windows 的来自微软日语输入法的 IFELanguage（JapaneseReader.cpp），没有它时只读纯假名的行。

namespace gloss_reading_detail {
// 从 `index` 处解出一个 UTF-8 码位并前移；不合法的字节当作 U+FFFD 前移一个字节。
inline char32_t next_code_point(std::string_view text, size_t &index) {
  const auto byte = static_cast<unsigned char>(text[index]);
  size_t length = byte < 0x80 ? 1 : (byte >> 5) == 0x6 ? 2 : (byte >> 4) == 0xE ? 3 : (byte >> 3) == 0x1E ? 4 : 0;
  if (length == 0 || index + length > text.size()) {
    ++index;
    return 0xFFFD;
  }
  char32_t value = length == 1 ? byte : length == 2 ? (byte & 0x1F) : length == 3 ? (byte & 0x0F) : (byte & 0x07);
  for (size_t offset = 1; offset < length; ++offset) {
    const auto continuation = static_cast<unsigned char>(text[index + offset]);
    if ((continuation & 0xC0) != 0x80) {
      ++index;
      return 0xFFFD;
    }
    value = (value << 6) | (continuation & 0x3F);
  }
  index += length;
  return value;
}
// ; ； , ， 、 /
inline bool separator(char32_t value) {
  return value == U';' || value == U'\uFF1B' || value == U',' || value == U'\uFF0C' ||
         value == U'\u3001' || value == U'/';
}
// 空格、制表符、不换行空格和全角空格。
inline bool space(char32_t value) {
  return value == U' ' || value == U'\t' || value == U'\u00A0' || value == U'\u3000';
}
} // namespace gloss_reading_detail

// 一行释义在第一个分隔符（; ； , ， 、 /）之前的部分，也就是这一行教的那个词，去掉两端空白。
inline std::string gloss_first_term(std::string_view line) {
  using namespace gloss_reading_detail;
  size_t end = line.size();
  for (size_t index = 0; index < line.size();) {
    const size_t start = index;
    if (separator(next_code_point(line, index))) {
      end = start;
      break;
    }
  }
  const auto term = line.substr(0, end);
  size_t first = term.size();
  for (size_t index = 0; index < term.size();) {
    const size_t start = index;
    if (!space(next_code_point(term, index))) {
      first = start;
      break;
    }
  }
  size_t last = first;
  for (size_t index = first; index < term.size();)
    if (!space(next_code_point(term, index)))
      last = index;
  return std::string(term.substr(first, last - first));
}

inline bool gloss_contains_kana(std::string_view text) {
  for (size_t index = 0; index < text.size();) {
    const auto value = gloss_reading_detail::next_code_point(text, index);
    if (value >= 0x3041 && value <= 0x30FF)
      return true;
  }
  return false;
}

// 纯英文：ASCII 字母加上单词里会有的空格、连字符和撇号，至少一个字母。
inline bool gloss_plain_english(std::string_view text) {
  bool letter = false;
  for (const char character : text) {
    if ((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z'))
      letter = true;
    else if (character != ' ' && character != '-' && character != '\'')
      return false;
  }
  return letter;
}

// 共享层除英文外还有读音表的目标语言（target-pronunciations/<语言>-phonetic.db）；日文由本机读罗马字。
inline bool gloss_shared_reading_target(std::string_view target) {
  return target == "fr" || target == "es" || target == "ru" || target == "de" || target == "ko";
}

// 第 `index` 行释义按哪种语言读："en"、"ja"、"fr"、"es"、"ru"、"de"、"ko"，或空表示不读。有假名就是日文；拉丁字母只在英文目标那一行才算英文（法文的 chat 也是纯字母，当英文读会教错音）；只有汉字的行只在日文目标那一行才算日文；法、西、俄、德、韩目标那一行就是那种语言，它的表里只有那种语言的词，不是的行查不到，不标读音。
inline std::string gloss_line_language(std::string_view line, size_t index,
                                       const std::vector<std::string> &targets) {
  const auto term = gloss_first_term(line);
  if (term.empty())
    return {};
  if (gloss_contains_kana(term))
    return "ja";
  const std::string target = index < targets.size() ? targets[index] : std::string{};
  if (target == "en")
    return gloss_plain_english(term) ? "en" : std::string{};
  if (gloss_shared_reading_target(target))
    return target;
  return target == "ja" ? "ja" : std::string{};
}

// 一个候选需要共享读音表回答的 (语言, 文本)：英文候选是它自己（英文；它那一行英文目标的释义是中文），其他候选是它的英文、法、西、俄、德、韩释义行，整行发过去，共享那边取第一个词。日文行由本机读，不在里面。
inline std::vector<std::pair<std::string, std::string>>
gloss_shared_items(const std::string &candidate_text, const std::vector<std::string> &lines,
                   const std::vector<std::string> &targets) {
  std::vector<std::pair<std::string, std::string>> items;
  bool any = false;
  for (const auto &line : lines)
    any = any || !line.empty();
  if (!any)
    return items;
  if (gloss_plain_english(candidate_text)) {
    items.emplace_back("en", candidate_text);
    return items;
  }
  for (size_t index = 0; index < lines.size(); ++index)
    if (auto language = gloss_line_language(lines[index], index, targets);
        !language.empty() && language != "ja")
      items.emplace_back(std::move(language), lines[index]);
  return items;
}

// 一个候选每行释义的读音，和释义逐行对应；没有一行有读音时返回空。`shared` 给出发给共享读音表的 (语言, 文本) 的回答，查不到返回空串；`japanese` 给出日文行第一个词的罗马字，读不全时返回空串，不给时日文行不标读音。
inline std::vector<std::string>
gloss_pronunciation_lines(const std::string &candidate_text, const std::vector<std::string> &lines,
                          const std::vector<std::string> &targets,
                          const std::function<std::string(const std::string &, const std::string &)> &shared,
                          const std::function<std::string(const std::string &)> &japanese = {}) {
  std::vector<std::string> readings;
  readings.reserve(lines.size());
  bool any = false;
  const bool english_candidate = gloss_plain_english(candidate_text);
  for (size_t index = 0; index < lines.size(); ++index) {
    const auto &line = lines[index];
    std::string reading;
    const bool english_line = index < targets.size() ? targets[index] == "en" : index == 0;
    if (english_candidate && !line.empty() && english_line)
      reading = shared("en", candidate_text);
    else if (const auto language = gloss_line_language(line, index, targets); language == "ja") {
      if (japanese)
        reading = japanese(gloss_first_term(line));
    } else if (!language.empty())
      reading = shared(language, line);
    any = any || !reading.empty();
    readings.push_back(std::move(reading));
  }
  if (!any)
    readings.clear();
  return readings;
}

// 候选窗画出来的释义：每行释义，它的读音在它下面单独一行，和 macOS 的 MSIMECandidateGlossDisplay 一样。读音不再接在释义行尾：英文释义加上 IPA 常常比候选本身宽一倍，横排时整格跟着这一行变宽，读音的末尾紧挨下一格的释义，像压在相邻候选上；格子被挤窄时这一行又在音标中间折开。单独一行后格宽取释义和读音里较宽的一行，多出的一行按已有的多行释义排（CandidateItemWidths::translation_lines），没有读音的页高度不变；竖排一行释义加它的读音仍在候选右边（candidate_translation_beside）。读音是 "\n" 分行的纯文本，候选窗按释义色的 0.72 倍透明度画（macOS 的 MSIMECandidatePronunciationOpacity）；reading_ranges 不为空时记下每段读音在结果里的位置（UTF-8 字节的起点和长度），line_starts 不为空时记下每行释义在结果里的起点，Tab 预选的释义列据此画下划线（candidate_gloss_column_range）。
inline std::string candidate_gloss_display(const std::vector<std::string> &lines,
                                           const std::vector<std::string> &readings,
                                           std::vector<std::pair<size_t, size_t>> *reading_ranges = nullptr,
                                           std::vector<size_t> *line_starts = nullptr) {
  std::string display;
  for (size_t index = 0; index < lines.size(); ++index) {
    if (index)
      display.push_back('\n');
    if (line_starts)
      line_starts->push_back(display.size());
    display.append(lines[index]);
    if (!lines[index].empty() && index < readings.size() && !readings[index].empty()) {
      display.push_back('\n');
      if (reading_ranges)
        reading_ranges->emplace_back(display.size(), readings[index].size());
      display.append(readings[index]);
    }
  }
  return display;
}
// 翻译工作线程为一个候选算好的读音，按候选文字存。translation 是算读音时那条释义（U+2028 分行）：候选眼下的释义与它相同才挂读音，免得旧读音跟到新释义后面。
struct CandidateReading {
  std::string translation;
  std::string pronunciation;
};
using CandidateReadings = std::unordered_map<std::string, CandidateReading>;
} // namespace msime::windows
