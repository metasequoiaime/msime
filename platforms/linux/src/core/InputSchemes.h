#pragma once

#include "InputSchemeTraits.h"

#include <nlohmann/json.hpp>

#include <array>
#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>

namespace msime::linux_host {

// Every preferences scheme id in Engine order: the index is the number a view reports as `scheme`.
inline constexpr std::array<std::string_view, 10> kInputSchemeIds = {
    "quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"};

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
  const bool cantonese = std::filesystem::is_regular_file(root / "cantonese.db", error);
  const bool zhuyin = std::filesystem::is_regular_file(root / "zhuyin.db", error);
  const bool stroke = std::filesystem::is_regular_file(root / "stroke.db", error);
  return {cantonese, zhuyin, stroke};
}

// Whether a scheme can run with these dictionaries: every known scheme but Cantonese, Zhuyin and Stroke needs no data beyond the resource set.
inline bool input_scheme_available(std::string_view id, LanguageDictionaryAvailability dictionaries) {
  if (id == "cantonese") return dictionaries.cantonese;
  if (id == "zhuyin") return dictionaries.zhuyin;
  if (id == "stroke") return dictionaries.stroke;
  return scheme_number(id) >= 0;
}

// The scheme the Engine actually runs for this preferences scheme, mirroring host-api's `effective_scheme`: a scheme that cannot run here gives way to the last Chinese scheme when that one can, and to quanpin otherwise. The menus check this one and the indicator shows its mode, so neither claims a scheme the user is not typing in.
inline std::string effective_input_scheme(std::string_view id, std::string_view last_chinese_scheme,
                                          LanguageDictionaryAvailability dictionaries) {
  if (input_scheme_available(id, dictionaries)) return std::string(id);
  const int last = scheme_number(last_chinese_scheme);
  if (last >= 0 && scheme::IsChinese(last) && input_scheme_available(last_chinese_scheme, dictionaries))
    return std::string(last_chinese_scheme);
  return "quanpin";
}

} // namespace msime::linux_host
