#pragma once
#include "CandidateThemeSettings.h"
#include <algorithm>
#include <filesystem>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {
// Translate the stored preferences into PreviewConfig's appearance block.
//
// Every field is optional on purpose. PreviewConfig validates hard - a font size outside 8..48 or an unprintable font name throws - and that validation runs on the production launch path, so copying a bad stored value straight through would turn a cosmetic preference into a Server that refuses to start. A value that would not survive validation is left out instead, and the built-in default stands in for it.
//
// Colours are not copied: the card, the toolbar and the menus resolve the global theme from the stored preferences at run time.
//
// system_dark is resolved by the caller rather than read here, so this stays a pure function of its inputs and the registry is not consulted under test.
inline nlohmann::json candidate_appearance(const std::filesystem::path &state,
                                           const nlohmann::json &preferences,
                                           bool system_dark) {
  auto text = [&preferences](const char *name, const char *fallback) {
    return preferences.contains(name) && preferences.at(name).is_string()
               ? preferences.at(name).get<std::string>()
               : std::string(fallback);
  };
  nlohmann::json appearance{{"skin_directory", (state / "skins").u8string()}};
  appearance["dark_theme"] = candidate_theme_dark(
      candidate_theme_values(preferences), system_dark);
  appearance["layout"] =
      text("candidate_layout", "vertical") == "horizontal" ? "horizontal"
                                                           : "vertical";
  appearance["candidate_preedit_style"] =
      text("candidate_preedit_style", "pinyin") == "empty" ? "empty" : "pinyin";
  // Mirrors PreviewConfig's own bounds, so anything kept here will load.
  auto printable = [](const std::string &value, size_t limit) {
    return !value.empty() && value.size() <= limit &&
           value.find('\0') == std::string::npos &&
           std::none_of(value.begin(), value.end(),
                        [](unsigned char c) { return c < 0x20; });
  };
  for (const char *name :
       {"candidate_font_size", "candidate_preedit_font_size"}) {
    if (!preferences.contains(name) ||
        !preferences.at(name).is_number_integer())
      continue;
    const auto size = preferences.at(name).get<int64_t>();
    if (size >= 8 && size <= 48)
      appearance[name] = static_cast<int>(size);
  }
  // Windows upstream uses the English face first for all candidate text, then
  // supplementary faces for missing glyphs, independent of input mode.
  const auto family = text("candidate_english_font", "Segoe UI");
  if (printable(family, 128))
    appearance["candidate_font"] = family;
  if (preferences.contains("candidate_fallback_fonts") &&
      preferences.at("candidate_fallback_fonts").is_array()) {
    auto fonts = nlohmann::json::array();
    for (const auto &font : preferences.at("candidate_fallback_fonts")) {
      if (fonts.size() >= 32 || !font.is_string())
        break;
      const auto value = font.get<std::string>();
      if (printable(value, 128))
        fonts.push_back(value);
    }
    if (!fonts.empty())
      appearance["candidate_fallback_fonts"] = fonts;
  }
  return appearance;
}
// Copy the stored floating-toolbar preferences onto the runtime document.
//
// Same contract as candidate_appearance: PreviewConfig validates these too - a
// scale outside 0.75..1.5 or a font size outside 16..28 throws on the launch
// path - so a value that would not survive is left out rather than copied
// through. The items object is all-or-nothing because PreviewConfig requires
// exactly six keys, so a partial block would be refused outright.
inline void apply_floating_toolbar(nlohmann::json &document,
                                   const nlohmann::json &preferences) {
  if (!preferences.contains("floating_toolbar") ||
      !preferences.at("floating_toolbar").is_object())
    return;
  const auto &toolbar = preferences.at("floating_toolbar");
  if (toolbar.contains("enabled") && toolbar.at("enabled").is_boolean())
    document["floating_toolbar_enabled"] = toolbar.at("enabled").get<bool>();
  // Stored as a percentage; the document carries a multiplier.
  if (toolbar.contains("scale_percent") &&
      toolbar.at("scale_percent").is_number_integer()) {
    const auto percent = toolbar.at("scale_percent").get<int64_t>();
    if (percent >= 75 && percent <= 150)
      document["floating_toolbar_scale"] =
          static_cast<double>(percent) / 100.0;
  }
  if (toolbar.contains("font_size") &&
      toolbar.at("font_size").is_number_integer()) {
    const auto size = toolbar.at("font_size").get<int64_t>();
    if (size >= 16 && size <= 28)
      document["floating_toolbar_font_size"] = static_cast<int>(size);
  }
  static constexpr const char *names[] = {"character_set", "punctuation",
                                          "fullwidth",     "emoji",
                                          "screen_keyboard", "settings"};
  nlohmann::json items = nlohmann::json::object();
  for (const char *name : names) {
    if (!toolbar.contains(name) || !toolbar.at(name).is_boolean())
      return; // Six or none.
    items[name] = toolbar.at(name).get<bool>();
  }
  document["floating_toolbar_items"] = items;
}
// The inline preedit the TSF side draws. PreviewConfig spells the pass-through
// case "local"; the stored preference spells the same thing "raw".
inline std::string tsf_preedit_style(const nlohmann::json &preferences) {
  if (!preferences.contains("tsf_preedit_style") ||
      !preferences.at("tsf_preedit_style").is_string())
    return "local";
  const auto style = preferences.at("tsf_preedit_style").get<std::string>();
  return style == "pinyin" || style == "empty" ? style : "local";
}
} // namespace msime::windows
