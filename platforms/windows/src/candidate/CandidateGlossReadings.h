#pragma once
#include <cstdint>
#include <functional>
#include <string>
#include <string_view>
#include <unordered_map>
#include <vector>

namespace msime::windows {
// 候选释义每一行后面画的读音和整句的逐词拆解，只用于显示，从不上屏。规则移植自 macOS 的 platforms/macos/src/candidate/CandidatePronunciation.h：释义每种目标语言一行，每行按它的第一个词读；英文走共享的离线读音表（msime_client_pronunciation_request），日文行读罗马字，macOS 的来自系统分词器，Windows 的来自微软日语输入法的 IFELanguage（JapaneseReader.cpp），没有它时只读纯假名的行。

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

// 第 `index` 行释义按哪种语言读："en"、"ja"，或空表示不读。有假名就是日文；拉丁字母只在英文目标那一行才算英文（法文的 chat 也是纯字母，当英文读会教错音）；只有汉字的行只在日文目标那一行才算日文。
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
  return target == "ja" ? "ja" : std::string{};
}

// 整句候选：2 到 32 个汉字，和 msime_client_gloss_breakdown_request 的共享规则一致。
inline bool gloss_sentence_candidate(std::string_view text) {
  size_t count = 0;
  for (size_t index = 0; index < text.size();) {
    const auto value = gloss_reading_detail::next_code_point(text, index);
    const bool han = (value >= 0x3400 && value <= 0x4DBF) || (value >= 0x4E00 && value <= 0x9FFF) ||
                     (value >= 0xF900 && value <= 0xFAFF) || (value >= 0x20000 && value <= 0x2FA1F);
    if (!han)
      return false;
    ++count;
  }
  return count >= 2 && count <= 32;
}

// 一个候选需要共享读音表回答的英文文本：英文候选是它自己（它那一行英文目标的释义是中文），其他候选是它的英文释义行，整行发过去，共享那边取第一个词。
inline std::vector<std::string> gloss_english_texts(const std::string &candidate_text,
                                                    const std::vector<std::string> &lines,
                                                    const std::vector<std::string> &targets) {
  std::vector<std::string> texts;
  bool any = false;
  for (const auto &line : lines)
    any = any || !line.empty();
  if (!any)
    return texts;
  if (gloss_plain_english(candidate_text)) {
    texts.push_back(candidate_text);
    return texts;
  }
  for (size_t index = 0; index < lines.size(); ++index)
    if (gloss_line_language(lines[index], index, targets) == "en")
      texts.push_back(lines[index]);
  return texts;
}

// 一个候选每行释义的读音，和释义逐行对应；没有一行有读音时返回空。`english` 给出发给共享读音表的文本对应的 "/…/"，查不到返回空串；`japanese` 给出日文行第一个词的罗马字，读不全时返回空串，不给时日文行不标读音。
inline std::vector<std::string>
gloss_pronunciation_lines(const std::string &candidate_text, const std::vector<std::string> &lines,
                          const std::vector<std::string> &targets,
                          const std::function<std::string(const std::string &)> &english,
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
      reading = english(candidate_text);
    else if (const auto language = gloss_line_language(line, index, targets); language == "en")
      reading = english(line);
    else if (language == "ja" && japanese)
      reading = japanese(gloss_first_term(line));
    any = any || !reading.empty();
    readings.push_back(std::move(reading));
  }
  if (!any)
    readings.clear();
  return readings;
}

// 候选窗画出来的释义：每行释义后面空两格跟它的读音，逐词拆解另起一行放在最后。读音和拆解都是 "\n" 分行的纯文本，和释义同一种颜色。
inline std::string candidate_gloss_display(const std::vector<std::string> &lines,
                                           const std::vector<std::string> &readings,
                                           const std::string &breakdown) {
  std::string display;
  for (size_t index = 0; index < lines.size(); ++index) {
    if (index)
      display.push_back('\n');
    display.append(lines[index]);
    if (!lines[index].empty() && index < readings.size() && !readings[index].empty()) {
      display.append("  ");
      display.append(readings[index]);
    }
  }
  if (!breakdown.empty()) {
    if (!display.empty())
      display.push_back('\n');
    display.append(breakdown);
  }
  return display;
}
// 翻译工作线程为一个候选算好的读音和拆解，按候选文字存。translation 是算读音时那条释义（U+2028 分行）：候选眼下的释义与它相同才挂读音，免得旧读音跟到新释义后面；拆解只看候选文字。
struct CandidateReading {
  std::string translation;
  std::string pronunciation;
  std::string breakdown;
};
using CandidateReadings = std::unordered_map<std::string, CandidateReading>;
} // namespace msime::windows
