#pragma once

// 候选释义的英文读音（音标），IBus 和 Fcitx5 两个宿主共用，只用于显示，从不上屏。规则与 macOS 的 platforms/macos/src/candidate/CandidatePronunciation.h、Windows 的 platforms/windows/src/candidate/CandidateGlossReadings.h 相同，只取英文这一半：释义每种目标语言一行，英文目标那一行按它的第一个词读，英文候选在英文目标那一行读它自己；读音来自共享层的离线读音表（msime_client_pronunciation_request）。Linux 没有可用的系统日文分词器，日文释义不标罗马音。
//
// 读音表在资源目录旁的 pronunciations/en-phonetic.db；Linux 的安装包不带它，用户在设置里打开「显示读音」后点「下载」，设置应用把它装进偏好目录下的 resource-packs/pronunciations/，宿主把 resource-packs 作为补充目录交给共享层。两处都没有时共享层回答空列表，候选窗照旧只画释义。

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <nlohmann/json.hpp>
#include <string>
#include <string_view>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace msime::linux_host {

// 一行释义在第一个分隔符（; ； , ， 、 /）之前的部分，也就是这一行教的那个词，去掉两端的空格、制表符、不换行空格和全角空格。
inline std::string gloss_first_term(std::string_view line) {
  static constexpr std::string_view separators[] = {";", "\xEF\xBC\x9B", ",", "\xEF\xBC\x8C", "\xE3\x80\x81", "/"};
  std::size_t end = line.size();
  for (const auto separator : separators) end = std::min(end, line.find(separator));
  auto term = line.substr(0, end);
  static constexpr std::string_view spaces[] = {" ", "\t", "\xC2\xA0", "\xE3\x80\x80"};
  for (bool trimmed = true; trimmed;) {
    trimmed = false;
    for (const auto space : spaces) {
      if (term.substr(0, space.size()) == space) {
        term.remove_prefix(space.size());
        trimmed = true;
      }
      if (term.size() >= space.size() && term.substr(term.size() - space.size()) == space) {
        term.remove_suffix(space.size());
        trimmed = true;
      }
    }
  }
  return std::string(term);
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

// 释义按 "\n" 分成的行，与目标语言逐行对应；空释义没有行。
inline std::vector<std::string> gloss_lines(std::string_view translation) {
  std::vector<std::string> lines;
  if (translation.empty()) return lines;
  for (std::size_t start = 0;;) {
    const auto cut = translation.find('\n', start);
    lines.emplace_back(translation.substr(start, cut == std::string_view::npos ? std::string_view::npos : cut - start));
    if (cut == std::string_view::npos) break;
    start = cut + 1;
  }
  return lines;
}

// 第 `index` 行释义是不是要按英文读：只有英文目标那一行、第一个词是纯英文时才是。法文的 chat 也是纯字母，按英文读会教错音，所以拉丁字母的行不在英文目标上就不读。
inline bool gloss_line_english(std::string_view line, std::size_t index, const std::vector<std::string> &targets) {
  if (index >= targets.size() || targets[index] != "en") return false;
  const auto term = gloss_first_term(line);
  return !term.empty() && gloss_plain_english(term);
}

// 英文候选在第 `index` 行读它自己：那一行是它的中文释义，行在英文目标上（没有目标语言时只认第一行）。
inline bool gloss_line_reads_candidate(std::string_view line, std::size_t index, const std::vector<std::string> &targets) {
  return !line.empty() && (index < targets.size() ? targets[index] == "en" : index == 0);
}

// 一个候选需要共享读音表回答的文本：英文候选是它自己，其他候选是它的英文释义行，整行发过去，共享那边取第一个词。
inline std::vector<std::string> gloss_english_texts(const std::string &candidate_text, std::string_view translation,
                                                    const std::vector<std::string> &targets) {
  std::vector<std::string> texts;
  const auto lines = gloss_lines(translation);
  if (gloss_plain_english(candidate_text)) {
    for (std::size_t index = 0; index < lines.size(); ++index)
      if (gloss_line_reads_candidate(lines[index], index, targets)) {
        texts.push_back(candidate_text);
        break;
      }
    return texts;
  }
  for (std::size_t index = 0; index < lines.size(); ++index)
    if (gloss_line_english(lines[index], index, targets)) texts.push_back(lines[index]);
  return texts;
}

// 一个候选每行释义的读音，按 "\n" 与释义逐行对应；没有一行有读音时是空串。`english` 给出发给共享读音表的文本对应的 "/…/"，查不到时返回空串。
inline std::string gloss_pronunciation(const std::string &candidate_text, std::string_view translation,
                                       const std::vector<std::string> &targets,
                                       const std::function<std::string(const std::string &)> &english) {
  const auto lines = gloss_lines(translation);
  const bool english_candidate = gloss_plain_english(candidate_text);
  std::string joined;
  bool any = false;
  for (std::size_t index = 0; index < lines.size(); ++index) {
    std::string reading;
    if (english_candidate) {
      if (gloss_line_reads_candidate(lines[index], index, targets)) reading = english(candidate_text);
    } else if (gloss_line_english(lines[index], index, targets)) {
      reading = english(lines[index]);
    }
    if (index) joined.push_back('\n');
    joined += reading;
    any = any || !reading.empty();
  }
  return any ? joined : std::string{};
}

// 读音表的补充目录：偏好目录下的 resource-packs，设置应用把资源包装在这里（`<state_root>/resource-packs/<id>/`）。偏好目录不是绝对路径时没有补充目录，共享层会拒绝相对路径。
inline std::string pronunciation_supplementary_root(const std::string &preferences_directory) {
  if (preferences_directory.empty() || preferences_directory.front() != '/') return {};
  auto root = preferences_directory;
  while (root.size() > 1 && root.back() == '/') root.pop_back();
  return root + (root == "/" ? "" : "/") + "resource-packs";
}

// 一批翻译回答（[{text, translation}]）里要问读音表的文本，去掉重复，保持先后顺序。
inline std::vector<std::string> translation_pronunciation_texts(const nlohmann::json &translations,
                                                                const std::vector<std::string> &targets) {
  std::vector<std::string> texts;
  if (!translations.is_array()) return texts;
  std::unordered_set<std::string> seen;
  for (const auto &item : translations) {
    if (!item.is_object()) continue;
    const auto text = item.find("text"), translation = item.find("translation");
    if (text == item.end() || !text->is_string() || translation == item.end() || !translation->is_string()) continue;
    for (auto &english : gloss_english_texts(text->get<std::string>(), translation->get_ref<const std::string &>(), targets))
      if (seen.insert(english).second) texts.push_back(std::move(english));
  }
  return texts;
}

// msime_client_pronunciation_request 的请求文档。
inline std::string pronunciation_request(std::uint64_t generation, const std::vector<std::string> &texts,
                                         const std::string &supplementary_root) {
  auto items = nlohmann::json::array();
  for (const auto &text : texts) items.push_back({{"text", text}, {"language", "en"}});
  nlohmann::json request{{"generation", generation}, {"items", std::move(items)}};
  if (!supplementary_root.empty()) request["supplementary_resources"] = supplementary_root;
  return request.dump();
}

// 读音表的回答（响应的 value：{generation, pronunciations:[{text, language, pronunciation}]}）变成 {文本: 读音}。问过的每个文本都有一项，没有音标的记空串，同一个词不再反复去问；回答里不是问过的文本的项不收。
inline nlohmann::json read_pronunciations(const nlohmann::json &value, const std::vector<std::string> &texts) {
  auto answers = nlohmann::json::object();
  std::unordered_set<std::string> asked(texts.begin(), texts.end());
  for (const auto &text : texts) answers[text] = "";
  if (!value.is_object()) return answers;
  const auto entries = value.find("pronunciations");
  if (entries == value.end() || !entries->is_array()) return answers;
  for (const auto &entry : *entries) {
    if (!entry.is_object()) continue;
    const auto text = entry.find("text"), pronunciation = entry.find("pronunciation");
    if (text == entry.end() || !text->is_string() || pronunciation == entry.end() || !pronunciation->is_string()) continue;
    if (!asked.count(text->get_ref<const std::string &>())) continue;
    answers[text->get<std::string>()] = *pronunciation;
  }
  return answers;
}

// 在翻译工作线程上为一批翻译回答查读音：`ask` 发出请求文档、交回响应的 value，失败时交回 null。没有要问的文本时不发请求。
inline nlohmann::json lookup_translation_pronunciations(
    const nlohmann::json &translations, const std::vector<std::string> &targets, std::uint64_t generation,
    const std::string &supplementary_root, const std::function<nlohmann::json(const std::string &)> &ask) {
  const auto texts = translation_pronunciation_texts(translations, targets);
  if (texts.empty()) return nlohmann::json::object();
  return read_pronunciations(ask(pronunciation_request(generation, texts, supplementary_root)), texts);
}

// 宿主记下的读音，按发给读音表的文本存，空串表示问过、没有音标。条目超过上限时整个清掉重来，和 macOS 的 _englishPronunciations 一样。焦点切换时宿主也清掉它：用户刚下载读音表时，之前记下的空串要重新问。
class CandidatePronunciations {
public:
  static constexpr std::size_t kCapacity = 4096;
  void store(const nlohmann::json &answers) {
    if (!answers.is_object()) return;
    if (entries_.size() + answers.size() > kCapacity) entries_.clear();
    for (const auto &item : answers.items())
      if (item.value().is_string()) entries_[item.key()] = item.value().get<std::string>();
  }
  std::string find(const std::string &text) const {
    const auto found = entries_.find(text);
    return found == entries_.end() ? std::string{} : found->second;
  }
  void clear() { entries_.clear(); }
  std::size_t size() const { return entries_.size(); }

  // 一个候选的读音（"\n" 分行，与释义逐行对应）；targets 为空表示没有打开「显示读音」。
  std::string candidate(const std::string &text, std::string_view translation,
                        const std::vector<std::string> &targets) const {
    if (targets.empty() || translation.empty()) return {};
    return gloss_pronunciation(text, translation, targets, [this](const std::string &english) { return find(english); });
  }

private:
  std::unordered_map<std::string, std::string> entries_;
};

// 候选窗画出来的释义：每行释义下面单独一行是它的读音，和 macOS 的 MSIMECandidateGlossDisplay 一样。读音接在行尾时横排的格子会被它撑宽一倍，读音的末尾贴着下一格的释义；单独一行后格宽取两者中较宽的一行。readings 是每段读音在 text 里的位置（UTF-8 字节的起点和长度），按释义色减淡画；lines 记下每行释义在原释义和 text 里的起点，释义里共享层标出的词性、分隔符区段按它换算到 text 里。
struct CandidateGlossReadings {
  std::string text;
  std::vector<std::pair<std::size_t, std::size_t>> readings;
  std::vector<std::pair<std::size_t, std::size_t>> lines;
  // 原释义里第 `offset` 个字节在 text 里的位置。
  std::size_t position(std::size_t offset) const {
    for (auto line = lines.rbegin(); line != lines.rend(); ++line)
      if (offset >= line->first) return line->second + (offset - line->first);
    return offset;
  }
};
inline CandidateGlossReadings candidate_gloss_readings(std::string_view translation, std::string_view pronunciation) {
  CandidateGlossReadings result;
  const auto lines = gloss_lines(translation);
  const auto readings = gloss_lines(pronunciation);
  std::size_t source = 0;
  for (std::size_t index = 0; index < lines.size(); ++index) {
    if (index) result.text.push_back('\n');
    result.lines.emplace_back(source, result.text.size());
    result.text += lines[index];
    source += lines[index].size() + 1;
    if (!lines[index].empty() && index < readings.size() && !readings[index].empty()) {
      result.text.push_back('\n');
      result.readings.emplace_back(result.text.size(), readings[index].size());
      result.text += readings[index];
    }
  }
  return result;
}

// IBus 的候选行只有一行纯文字：读音接在它那一行释义后面，隔两个空格。
inline std::string candidate_gloss_inline_readings(std::string_view translation, std::string_view pronunciation) {
  const auto lines = gloss_lines(translation);
  const auto readings = gloss_lines(pronunciation);
  std::string text;
  for (std::size_t index = 0; index < lines.size(); ++index) {
    if (index) text.push_back('\n');
    text += lines[index];
    if (!lines[index].empty() && index < readings.size() && !readings[index].empty()) text += "  " + readings[index];
  }
  return text;
}

} // namespace msime::linux_host
