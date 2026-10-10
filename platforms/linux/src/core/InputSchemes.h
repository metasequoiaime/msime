#pragma once

#include "InputSchemeTraits.h"
#include "LinuxEdition.h"

#include <nlohmann/json.hpp>

#include <array>
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>

namespace msime::linux_host {

// Every preferences scheme id in Engine order: the index is the number a view reports as `scheme`.
inline constexpr std::array<std::string_view, 10> kInputSchemeIds = {
    "quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"};

// 本版本提供的方案（版本表的 input_schemes）和回退到的默认方案。不在列表里的方案在本版本中不存在：菜单不列它，偏好里写着它时与宿主库的 effective_scheme 一样回退。
inline constexpr std::string_view kEditionInputSchemes[] = {MSIME_EDITION_INPUT_SCHEMES};
inline constexpr std::string_view kEditionDefaultScheme = MSIME_EDITION_DEFAULT_SCHEME;

inline bool edition_offers_scheme(std::string_view id) {
  for (const auto scheme : kEditionInputSchemes)
    if (scheme == id) return true;
  return false;
}

// The Engine number of a preferences scheme id, or -1 for an id this host does not know.
inline int scheme_number(std::string_view id) {
  for (std::size_t index = 0; index < kInputSchemeIds.size(); ++index)
    if (kInputSchemeIds[index] == id) return static_cast<int>(index);
  return -1;
}

// The `scheme` number of a view, or -1 when it carries none.
inline int view_scheme(const nlohmann::json &view) {
  if (!view.is_object()) return -1;
  const auto scheme = view.find("scheme");
  return scheme != view.end() && scheme->is_number_integer() ? static_cast<int>(scheme->get<int64_t>()) : -1;
}

// The view's own scheme rules hold: outside the dedicated English mode and outside every local mode, which keep their own rules in every scheme (msime_client.h). Returns the scheme number then, -1 otherwise.
inline int scheme_rules(const nlohmann::json &view) {
  const int scheme = view_scheme(view);
  if (scheme < 0) return -1;
  const auto english = view.find("dedicated_english");
  if (english != view.end() && english->is_boolean() && english->get<bool>()) return -1;
  const auto mode = view.find("local_mode");
  if (mode != view.end() && mode->is_string() && mode->get_ref<const std::string &>() != "none") return -1;
  return scheme;
}

// A composition is open under the rules of a scheme whose candidates appear only in a list the user opens (the Korean syllable, the Zhuyin conversion): editing_text is non-empty exactly while it composes. The Hanja key and F9 open the list then.
inline bool candidate_list_composition(const nlohmann::json &view) {
  const int scheme = scheme_rules(view);
  if (scheme < 0 || !scheme::OpensCandidateList(scheme)) return false;
  const auto editing = view.find("editing_text");
  return editing != view.end() && editing->is_string() && !editing->get_ref<const std::string &>().empty();
}

// That list is open: the Engine offers candidates under these rules only once it is, so a non-empty list is that list (msime_client.h). Return then chooses the highlighted row, and a digit past the end of the page is swallowed rather than typed beside the composition.
inline bool opened_candidate_list(const nlohmann::json &view) {
  if (!candidate_list_composition(view)) return false;
  const auto candidates = view.find("candidates");
  return candidates != view.end() && candidates->is_array() && !candidates->empty();
}

// 左右键是整句改字的光标键：全拼和双拼的普通组字（不在专用英文和本地模式里）。引擎进不了改字时自己退回字母光标，所以宿主不必看首选是不是整句。IBus 和 Fcitx5 都按这一条决定左右键和 Ctrl+左右发什么命令。
inline bool edits_sentence(const nlohmann::json &view) {
  const int scheme = scheme_rules(view);
  return scheme >= 0 && scheme::EditsSentence(scheme);
}

// 整句改字时行内显示的文字和光标：`phrase_prefix` 加上改好的整句，光标在焦点字前。`conversion_focus_start`、`conversion_focus_end` 按 Unicode 标量计，这里同时给出字节和标量两种单位（Fcitx5 用字节，IBus 用标量），焦点那一段 `focus_*` 同样两种单位都有。不在改字时 `active` 为假。
struct ConversionPreedit {
  bool active = false;
  std::string text;
  std::size_t caret_bytes = 0;
  std::size_t caret_scalars = 0;
  std::size_t focus_end_bytes = 0;
  std::size_t focus_end_scalars = 0;
};

inline ConversionPreedit conversion_preedit(const nlohmann::json &view) {
  ConversionPreedit result;
  if (!view.is_object()) return result;
  const auto conversion = view.find("conversion");
  if (conversion == view.end() || !conversion->is_string() || conversion->get_ref<const std::string &>().empty()) return result;
  const auto &sentence = conversion->get_ref<const std::string &>();
  std::string prefix;
  const auto phrase = view.find("phrase_prefix");
  if (phrase != view.end() && phrase->is_string()) prefix = phrase->get<std::string>();
  // 把标量下标换成字节偏移；超出范围时取末尾。
  const auto scalar_index = [&](const char *key) -> std::pair<std::size_t, std::size_t> {
    const auto value = view.find(key);
    // 解析出来的非负整数是无符号的，宿主自己构造的视图里是有符号的，两种都认；负数和非整数取末尾。
    std::size_t wanted = static_cast<std::size_t>(-1);
    if (value != view.end() && value->is_number_unsigned())
      wanted = value->get<std::size_t>();
    else if (value != view.end() && value->is_number_integer() && value->get<int64_t>() >= 0)
      wanted = static_cast<std::size_t>(value->get<int64_t>());
    std::size_t bytes = 0, scalars = 0;
    while (bytes < sentence.size() && scalars < wanted) {
      ++bytes;
      while (bytes < sentence.size() && (static_cast<unsigned char>(sentence[bytes]) & 0xC0u) == 0x80u) ++bytes;
      ++scalars;
    }
    return {bytes, scalars};
  };
  const auto [start_bytes, start_scalars] = scalar_index("conversion_focus_start");
  auto [end_bytes, end_scalars] = scalar_index("conversion_focus_end");
  if (end_bytes < start_bytes) {
    end_bytes = start_bytes;
    end_scalars = start_scalars;
  }
  std::size_t prefix_scalars = 0;
  for (unsigned char byte : prefix)
    if ((byte & 0xC0u) != 0x80u) ++prefix_scalars;
  result.active = true;
  result.text = prefix + sentence;
  result.caret_bytes = prefix.size() + start_bytes;
  result.caret_scalars = prefix_scalars + start_scalars;
  result.focus_end_bytes = prefix.size() + end_bytes;
  result.focus_end_scalars = prefix_scalars + end_scalars;
  return result;
}

// Down with no modifier while a Zhuyin conversion composes with its list closed: libchewing's key for opening the candidate list. It belongs to the input method whatever the arrow navigation binding says, because with the list closed there is no highlight for that binding to move. Once the list is open Down moves the highlight as in every other list.
inline bool zhuyin_list_down_key(const nlohmann::json &view) {
  return scheme_rules(view) == scheme::Zhuyin && candidate_list_composition(view) && !opened_candidate_list(view);
}

// Which language dictionaries the `language_dictionaries` directory named by a HostOptions document holds: Cantonese, Zhuyin and Stroke need theirs, and host-api falls back from each of them when it is missing, so offering it would select a scheme that never takes effect. Mirrors host-api's `LanguageDictionaries::serve`. This looks at the disk, so a host works it out once whenever it loads the options and keeps the result, keeping file checks off the key path and the scheme steady for the life of a session, as host-api decides it once when the session opens.
struct LanguageDictionaryAvailability {
  bool cantonese = false;
  bool zhuyin = false;
  bool stroke = false;
};

inline LanguageDictionaryAvailability language_dictionary_availability(const nlohmann::json &options) {
  if (!options.is_object()) return {};
  const auto directory = options.find("language_dictionaries");
  if (directory == options.end() || !directory->is_string() || directory->get_ref<const std::string &>().empty()) return {};
  const std::filesystem::path root = directory->get<std::string>();
  std::error_code error;
  const bool cantonese = std::filesystem::is_regular_file(root / "msime-cantonese.db", error);
  const bool zhuyin = std::filesystem::is_regular_file(root / "msime-zhuyin.db", error);
  const bool stroke = std::filesystem::is_regular_file(root / "msime-stroke.db", error);
  return {cantonese, zhuyin, stroke};
}

// Whether a scheme can run with these dictionaries: every known scheme but Cantonese, Zhuyin and Stroke needs no data beyond the resource set. 本版本不提供的方案一律不能跑。
inline bool input_scheme_available(std::string_view id, LanguageDictionaryAvailability dictionaries) {
  if (!edition_offers_scheme(id)) return false;
  if (id == "cantonese") return dictionaries.cantonese;
  if (id == "zhuyin") return dictionaries.zhuyin;
  if (id == "stroke") return dictionaries.stroke;
  return scheme_number(id) >= 0;
}

// The scheme the Engine actually runs for this preferences scheme, mirroring host-api's `effective_scheme`: a scheme that cannot run here gives way to the last Chinese scheme when that one can, and to the edition's default scheme otherwise (quanpin in full). The menus check this one and the indicator shows its mode, so neither claims a scheme the user is not typing in.
inline std::string effective_input_scheme(std::string_view id, std::string_view last_chinese_scheme,
                                          LanguageDictionaryAvailability dictionaries) {
  if (input_scheme_available(id, dictionaries)) return std::string(id);
  const int last = scheme_number(last_chinese_scheme);
  if (last >= 0 && scheme::IsChinese(last) && input_scheme_available(last_chinese_scheme, dictionaries))
    return std::string(last_chinese_scheme);
  return std::string(kEditionDefaultScheme);
}

} // namespace msime::linux_host
