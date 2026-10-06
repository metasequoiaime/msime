#pragma once

#include <algorithm>
#include <cctype>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace msime::linux_host {

// Windows draws its own candidate window and reads candidate_font_family, the fallback chain and
// candidate_font_size straight into it. Neither Linux host draws the list: IBus hands it to the
// desktop panel and Fcitx5 to its classic UI, and both of those take one Pango font description.
// So the Linux answer to the same three settings is to write that description where the panel
// reads it, rather than to draw a second candidate window next to the one the desktop already has.
struct CandidateFont {
  std::string family;
  std::vector<std::string> fallbacks;
  int size_px = 0;
  // candidate_english_font. Windows draws candidate text in this face first and falls back for the glyphs it lacks, and macOS and Android name it ahead of the primary family; a Pango family list resolves per glyph the same way, so the Linux answer is to put it first. Empty when unset, which leaves the description exactly as it was before the setting existed.
  std::string english_family{};
};

inline constexpr const char *kDefaultCandidateFontFamily = "Noto Sans SC";
inline constexpr int kDefaultCandidateFontSize = 18;

inline const std::vector<std::string> &default_candidate_fallback_fonts() {
  static const std::vector<std::string> fonts{"Noto Sans SC", "Microsoft YaHei"};
  return fonts;
}

// Reads the shared font preferences the way the preference store defaults them, so a document written before a key existed describes the same font as one that spells the default out.
inline CandidateFont read_candidate_font(const nlohmann::json &preferences) {
  CandidateFont font{kDefaultCandidateFontFamily, default_candidate_fallback_fonts(),
                     kDefaultCandidateFontSize};
  if (!preferences.is_object()) return font;
  if (const auto family = preferences.find("candidate_font_family");
      family != preferences.end() && family->is_string())
    font.family = family->get<std::string>();
  if (const auto fallbacks = preferences.find("candidate_fallback_fonts");
      fallbacks != preferences.end() && fallbacks->is_array()) {
    font.fallbacks.clear();
    for (const auto &fallback : *fallbacks)
      if (fallback.is_string()) font.fallbacks.push_back(fallback.get<std::string>());
  }
  if (const auto size = preferences.find("candidate_font_size");
      size != preferences.end() && size->is_number_integer())
    font.size_px = size->get<int>();
  // Optional and absent by default; null is how the store writes "not chosen".
  if (const auto english = preferences.find("candidate_english_font");
      english != preferences.end() && english->is_string())
    font.english_family = english->get<std::string>();
  return font;
}

inline std::string trimmed_font_family(const std::string &value) {
  auto begin = value.begin();
  auto end = value.end();
  while (begin != end && std::isspace(static_cast<unsigned char>(*begin))) ++begin;
  while (end != begin && std::isspace(static_cast<unsigned char>(*(end - 1)))) --end;
  std::string family(begin, end);
  // A comma separates families in a Pango description; one inside a name would split it in two.
  family.erase(std::remove(family.begin(), family.end(), ','), family.end());
  return family;
}

// The description lists the English family when one is chosen, then the primary family and the fallbacks, without repeats, and ends the family list with a comma so that a name ending in a word Pango knows as a style ("Bold", "Light") stays part of the name. The size is in pixels, the unit the shared preference is written in.
inline std::string candidate_pango_font(const CandidateFont &font) {
  std::vector<std::string> families;
  families.reserve(font.fallbacks.size() + 2);
  auto add = [&](const std::string &value) {
    auto family = trimmed_font_family(value);
    if (!family.empty() && std::find(families.begin(), families.end(), family) == families.end())
      families.push_back(std::move(family));
  };
  add(font.english_family);
  add(font.family);
  for (const auto &fallback : font.fallbacks) add(fallback);
  std::string description;
  for (const auto &family : families) {
    if (!description.empty()) description += ", ";
    description += family;
  }
  if (!description.empty()) description += ",";
  const int size = font.size_px >= 12 && font.size_px <= 32 ? font.size_px : kDefaultCandidateFontSize;
  if (!description.empty()) description += ' ';
  description += std::to_string(size) + "px";
  return description;
}

// The default has no English family, so a document that never set one still counts as untouched and leaves the desktop's panel font alone, while choosing one counts as a change like any other font choice.
inline bool candidate_font_is_default(const CandidateFont &font) {
  return candidate_pango_font(font) ==
         candidate_pango_font({kDefaultCandidateFontFamily, default_candidate_fallback_fonts(),
                               kDefaultCandidateFontSize, std::string{}});
}

// The panel font belongs to the whole desktop, not to this input method, so the host only writes
// it in answer to the user choosing a font. Untouched defaults leave whatever the desktop already
// has; after that, every change the user makes is written once, and an unchanged value is not
// written again on every preference refresh.
class CandidateFontSync {
public:
  std::optional<std::string> next(const CandidateFont &font) {
    auto description = candidate_pango_font(font);
    if (!last_ && candidate_font_is_default(font)) {
      last_ = description;
      return std::nullopt;
    }
    if (last_ == description) return std::nullopt;
    last_ = description;
    return description;
  }

private:
  std::optional<std::string> last_;
};

}  // namespace msime::linux_host
