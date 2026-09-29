#pragma once
#include "EditPolicy.h"
#include "NavigationPolicy.h"
#include "WordCharacterPolicy.h"
#include <algorithm>
#include <array>
#include <filesystem>
#include <string_view>
#include <nlohmann/json.hpp>

namespace msime::windows {
struct PreviewConfig {
  std::filesystem::path resources;
  std::filesystem::path state_root;
  std::string pipe_namespace;
  TsfPreeditStyle style;
  NavigationBindings navigation{};
  bool explicit_key_bindings = false;
  bool floating_toolbar_enabled = true;
  double floating_toolbar_scale = 1.0;
  int floating_toolbar_font_size = 24;
  std::optional<int> floating_toolbar_x;
  std::optional<int> floating_toolbar_y;
  WordCharacterBinding word_character = WordCharacterBinding::Disabled;
  // Optional appearance. Without it the presenters keep their built-in theme.
  std::filesystem::path skin_directory;
  bool dark_theme = true;
  // The shipped card lays candidates out on one row; vertical stays available.
  bool horizontal_candidates = true;
  int candidate_font_size = 16;
  int candidate_preedit_font_size = 16;
  std::string candidate_font = "Segoe UI";
  // Supplementary faces tried in order when the main font lacks a glyph.
  std::vector<std::string> candidate_fallback_fonts;
  // "pinyin" shows the reading above the list; "empty" hides that line.
  bool candidate_show_preedit = true;
  std::optional<bool> candidate_selected_bar;
  std::array<bool, 6> floating_toolbar_items{true, true, true, true, false, true};
  static PreviewConfig parse(const std::string &document) {
    if (document.size() > 16384)
      throw std::invalid_argument("Oversized preview configuration");
    const auto value = nlohmann::json::parse(document);
    if (!value.is_object() ||
        value.size() != ((value.contains("key_bindings") ? 6u : 5u) +
                         (value.contains("floating_toolbar_enabled") ? 1u : 0u) +
                         (value.contains("floating_toolbar_scale") ? 1u : 0u) +
                         (value.contains("floating_toolbar_font_size") ? 1u : 0u) +
                         (value.contains("floating_toolbar_x") ? 1u : 0u) +
                         (value.contains("floating_toolbar_y") ? 1u : 0u) +
                         (value.contains("floating_toolbar_items") ? 1u : 0u) +
                         (value.contains("appearance") ? 1u : 0u)) ||
        !value.at("format_version").is_number_integer() ||
        value.at("format_version") != 1)
      throw std::invalid_argument("Invalid preview configuration");
    PreviewConfig result;
    result.resources =
        std::filesystem::u8path(value.at("resources").get<std::string>());
    result.state_root =
        std::filesystem::u8path(value.at("state_root").get<std::string>());
    result.pipe_namespace = value.at("pipe_namespace").get<std::string>();
    result.style = TsfPreeditStyle::Local;
    if (!result.resources.is_absolute() || !result.state_root.is_absolute() ||
        result.resources.u8string().find('\0') != std::string::npos ||
        result.state_root.u8string().find('\0') != std::string::npos)
      throw std::invalid_argument("Preview paths must be absolute");
    if (result.pipe_namespace.empty() || result.pipe_namespace.size() > 48)
      throw std::invalid_argument("Invalid preview pipe namespace");
    for (unsigned char c : result.pipe_namespace)
      if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
            (c >= '0' && c <= '9') || c == '-'))
        throw std::invalid_argument("Invalid preview pipe namespace");
    const auto style = value.at("preedit_style").get<std::string>();
    if (style == "pinyin")
      result.style = TsfPreeditStyle::Pinyin;
    else if (style == "empty")
      result.style = TsfPreeditStyle::Empty;
    else if (style != "local")
      throw std::invalid_argument("Invalid preview preedit style");
    if (value.contains("appearance")) {
      const auto &appearance = value.at("appearance");
      if (!appearance.is_object() || appearance.size() > 9 ||
          !appearance.contains("skin_directory") ||
          !appearance.at("skin_directory").is_string())
        throw std::invalid_argument("Invalid preview appearance");
      // The envelope is strict, so appearance is too: an unknown key is far
      // more likely a typo than a future field, and silently ignoring it means
      // the user's setting never applies and nothing says why.
      // Colours are not here: the card, toolbar and menus are coloured by the global theme in the stored preferences.
      static constexpr std::string_view known[] = {
          "skin_directory",         "layout",
          "dark_theme",             "candidate_font_size",
          "candidate_preedit_font_size", "candidate_font",
          "candidate_selected_bar", "candidate_fallback_fonts",
          "candidate_preedit_style"};
      for (const auto &entry : appearance.items()) {
        bool recognized = false;
        for (const auto &name : known)
          if (entry.key() == name) {
            recognized = true;
            break;
          }
        if (!recognized)
          throw std::invalid_argument("Invalid preview appearance");
      }
      result.skin_directory = std::filesystem::u8path(
          appearance.at("skin_directory").get<std::string>());
      if (!result.skin_directory.is_absolute() ||
          result.skin_directory.u8string().find('\0') != std::string::npos)
        throw std::invalid_argument("Preview paths must be absolute");
      if (appearance.contains("layout")) {
        if (!appearance.at("layout").is_string())
          throw std::invalid_argument("Invalid preview appearance");
        const auto layout = appearance.at("layout").get<std::string>();
        if (layout == "vertical")
          result.horizontal_candidates = false;
        else if (layout != "horizontal")
          throw std::invalid_argument("Invalid preview appearance");
      }
      if (appearance.contains("dark_theme")) {
        if (!appearance.at("dark_theme").is_boolean())
          throw std::invalid_argument("Invalid preview appearance");
        result.dark_theme = appearance.at("dark_theme").get<bool>();
      }
      if (appearance.contains("candidate_font_size")) {
        result.candidate_font_size = appearance.at("candidate_font_size").get<int>();
        if (result.candidate_font_size < 8 || result.candidate_font_size > 48)
          throw std::invalid_argument("Invalid candidate font size");
      }
      if (appearance.contains("candidate_preedit_font_size")) {
        result.candidate_preedit_font_size = appearance.at("candidate_preedit_font_size").get<int>();
        if (result.candidate_preedit_font_size < 8 || result.candidate_preedit_font_size > 48)
          throw std::invalid_argument("Invalid candidate preedit font size");
      }
      if (appearance.contains("candidate_font")) {
        result.candidate_font = appearance.at("candidate_font").get<std::string>();
        if (result.candidate_font.empty() || result.candidate_font.size() > 128 ||
            result.candidate_font.find('\0') != std::string::npos ||
            std::any_of(result.candidate_font.begin(), result.candidate_font.end(),
                        [](unsigned char c) { return c < 0x20; }))
          throw std::invalid_argument("Invalid candidate font");
      }
      if (appearance.contains("candidate_fallback_fonts")) {
        const auto &fonts = appearance.at("candidate_fallback_fonts");
        if (!fonts.is_array() || fonts.size() > 32)
          throw std::invalid_argument("Invalid candidate fallback fonts");
        for (const auto &font : fonts) {
          if (!font.is_string())
            throw std::invalid_argument("Invalid candidate fallback fonts");
          auto name = font.get<std::string>();
          if (name.empty() || name.size() > 128 ||
              name.find('\0') != std::string::npos ||
              std::any_of(name.begin(), name.end(),
                          [](unsigned char c) { return c < 0x20; }))
            throw std::invalid_argument("Invalid candidate fallback fonts");
          result.candidate_fallback_fonts.push_back(std::move(name));
        }
      }
      if (appearance.contains("candidate_preedit_style")) {
        const auto &preedit = appearance.at("candidate_preedit_style");
        if (!preedit.is_string())
          throw std::invalid_argument("Invalid candidate preedit style");
        const auto preedit_style = preedit.get<std::string>();
        // Only the shared vocabulary is accepted; an unknown value would
        // otherwise silently pick a presentation the user did not choose.
        if (preedit_style == "pinyin")
          result.candidate_show_preedit = true;
        else if (preedit_style == "empty")
          result.candidate_show_preedit = false;
        else
          throw std::invalid_argument("Invalid candidate preedit style");
      }
      if (appearance.contains("candidate_selected_bar")) {
        if (!appearance.at("candidate_selected_bar").is_boolean())
          throw std::invalid_argument("Invalid candidate selected bar");
        result.candidate_selected_bar = appearance.at("candidate_selected_bar").get<bool>();
      }
    }
    if (value.contains("key_bindings")) {
      result.explicit_key_bindings = true;
      const auto &keys = value.at("key_bindings");
      if (!keys.is_object() || (keys.size() != 7 && keys.size() != 8) ||
          (keys.size() == 8 && !keys.contains("mouse_wheel")))
        throw std::invalid_argument("Invalid preview key bindings");
      result.navigation.minus_equal = keys.at("minus_equal").get<bool>();
      result.navigation.comma_period = keys.at("comma_period").get<bool>();
      result.navigation.brackets = keys.at("brackets").get<bool>();
      result.navigation.tab = keys.at("tab").get<bool>();
      result.navigation.page_up_down = keys.at("page_up_down").get<bool>();
      result.navigation.arrows = keys.at("arrows").get<bool>();
      result.navigation.mouse_wheel = keys.value("mouse_wheel", false);
      const auto word = keys.at("word_character").get<std::string>();
      if (word == "brackets")
        result.word_character = WordCharacterBinding::Brackets;
      else if (word == "minus_equal")
        result.word_character = WordCharacterBinding::MinusEqual;
      else if (word != "disabled")
        throw std::invalid_argument("Invalid preview word binding");
    }
    if (value.contains("floating_toolbar_enabled"))
      result.floating_toolbar_enabled = value.at("floating_toolbar_enabled").get<bool>();
    if (value.contains("floating_toolbar_scale")) {
      result.floating_toolbar_scale = value.at("floating_toolbar_scale").get<double>();
      if (result.floating_toolbar_scale < 0.75 || result.floating_toolbar_scale > 1.5)
        throw std::invalid_argument("Invalid floating toolbar scale");
    }
    if (value.contains("floating_toolbar_font_size")) {
      result.floating_toolbar_font_size = value.at("floating_toolbar_font_size").get<int>();
      if (result.floating_toolbar_font_size < 16 || result.floating_toolbar_font_size > 28)
        throw std::invalid_argument("Invalid floating toolbar font size");
    }
    for (const auto *key : {"floating_toolbar_x", "floating_toolbar_y"}) {
      if (!value.contains(key)) continue;
      const int coordinate = value.at(key).get<int>();
      if (coordinate < -32768 || coordinate > 32767)
        throw std::invalid_argument("Invalid floating toolbar position");
      if (std::string_view(key).back() == 'x') result.floating_toolbar_x = coordinate;
      else result.floating_toolbar_y = coordinate;
    }
    if (value.contains("floating_toolbar_items")) {
      const auto &items = value.at("floating_toolbar_items");
      if (!items.is_object() || items.size() != 6)
        throw std::invalid_argument("Invalid floating toolbar items");
      result.floating_toolbar_items = {
          items.at("character_set").get<bool>(), items.at("punctuation").get<bool>(),
          items.at("fullwidth").get<bool>(), items.at("emoji").get<bool>(),
          items.at("screen_keyboard").get<bool>(), items.at("settings").get<bool>()};
    }
    return result;
  }
  // The session-less language-bar endpoint. Preview builds keep it inside the
  // same namespace so two Servers on one machine never collide.
  std::wstring aux_pipe_name() const {
    const std::wstring token(pipe_namespace.begin(), pipe_namespace.end());
    return L"\\\\.\\pipe\\msime-client-preview-" + token + L"-aux";
  }
  std::array<std::wstring, 3> pipe_names() const {
    std::array<std::wstring, 3> names;
    const std::wstring token(pipe_namespace.begin(), pipe_namespace.end());
    for (size_t role = 0; role < names.size(); ++role)
      names[role] = L"\\\\.\\pipe\\msime-client-preview-" + token + L"-" +
                    std::to_wstring(role);
    return names;
  }
};
} // namespace msime::windows
