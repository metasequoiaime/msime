#include "PreviewConfig.h"
#include <iostream>

using namespace msime::windows;
// A bare `return 1` told us only that something in three hundred assertions
// broke, which is how this test stayed red unnoticed: the pipe-only preset
// skips it, so nobody read the exit code. Report the line instead.
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Preview configuration test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
int main() {
  try {
    const auto root = std::filesystem::temp_directory_path();
    nlohmann::json document{{"format_version", 1},
                            {"resources", (root / "resources").u8string()},
                            {"state_root", (root / "state").u8string()},
                            {"pipe_namespace", "fixture-12"},
                            {"preedit_style", "pinyin"}};
    const auto good = PreviewConfig::parse(document.dump());
    require(good.style == TsfPreeditStyle::Pinyin);
    require(good.floating_toolbar_enabled);
    require(!good.navigation.minus_equal && !good.navigation.comma_period &&
            !good.navigation.brackets && !good.navigation.tab &&
            !good.navigation.page_up_down && !good.navigation.arrows &&
            !good.navigation.mouse_wheel &&
            good.word_character == WordCharacterBinding::Disabled);
    const auto names = good.pipe_names();
    require(names[0] == L"\\\\.\\pipe\\msime-client-preview-fixture-12-0" &&
            names[0] != names[1] && names[1] != names[2]);
    // Takes the caller's line, or every rejection failure would report the one
    // line inside this lambda and name none of the ~60 documents it checks.
    auto reject_at = [&](nlohmann::json bad, int line) {
      bool rejected = false;
      try {
        PreviewConfig::parse(bad.dump());
      } catch (...) {
        rejected = true;
      }
      if (!rejected)
        require_failed(line);
    };
#define reject(bad) reject_at((bad), __LINE__)
    for (const char *field : {"resources", "state_root", "preedit_style",
                              "format_version", "pipe_namespace"}) {
      auto bad = document;
      bad.erase(field);
      reject(bad);
    }
    for (const std::string &token :
         {std::string{}, std::string("../server"), std::string("pipe\\name"),
          std::string(49, 'a'), std::string("a\0b", 3)}) {
      auto bad = document;
      bad["pipe_namespace"] = token;
      reject(bad);
    }
    auto bad = document;
    bad["state_root"] = "relative";
    reject(bad);
    bad = document;
    bad["resources"] = "relative";
    reject(bad);
    bad = document;
    bad["extra"] = true;
    reject(bad);
    bad = document;
    bad["format_version"] = 2;
    reject(bad);
    bad["format_version"] = 1.0;
    reject(bad);
    bad = document;
    bad["preedit_style"] = "unknown";
    reject(bad);
    bad = document;
    bad["resources"] = std::string(16385, 'x');
    reject(bad);
    document["preedit_style"] = "local";
    require(PreviewConfig::parse(document.dump()).style ==
            TsfPreeditStyle::Local);
    document["floating_toolbar_enabled"] = false;
    require(!PreviewConfig::parse(document.dump()).floating_toolbar_enabled);
    document["floating_toolbar_enabled"] = 1;
    reject(document);
    document["floating_toolbar_enabled"] = true;
    const nlohmann::json toolbar_items{
        {"character_set", false}, {"punctuation", true}, {"fullwidth", false},
        {"emoji", true}, {"screen_keyboard", true}, {"settings", false}};
    document["floating_toolbar_items"] = toolbar_items;
    document["floating_toolbar_x"] = 120;
    document["floating_toolbar_y"] = -40;
    const auto configured_position = PreviewConfig::parse(document.dump());
    require(configured_position.floating_toolbar_x == 120 &&
            configured_position.floating_toolbar_y == -40);
    auto invalid_position = document;
    invalid_position["floating_toolbar_x"] = 32768;
    reject(invalid_position);
    invalid_position = document;
    invalid_position["floating_toolbar_y"] = -32769;
    reject(invalid_position);
    const auto configured_items = PreviewConfig::parse(document.dump());
    require(!configured_items.floating_toolbar_items[0] &&
            configured_items.floating_toolbar_items[1] &&
            !configured_items.floating_toolbar_items[2] &&
            configured_items.floating_toolbar_items[3] &&
            configured_items.floating_toolbar_items[4] &&
            !configured_items.floating_toolbar_items[5]);
    document["floating_toolbar_items"]["extra"] = true;
    reject(document);
    document["floating_toolbar_items"] = toolbar_items;
    const nlohmann::json bindings{
        {"minus_equal", false},        {"comma_period", false},
        {"brackets", false},           {"tab", false},
        {"page_up_down", false},       {"arrows", false},
        {"word_character", "disabled"}};
    for (const char *name : {"minus_equal", "comma_period", "brackets", "tab",
                             "page_up_down", "arrows", "mouse_wheel"}) {
      auto config = document;
      config["key_bindings"] = bindings;
      config["key_bindings"][name] = true;
      const auto loaded = PreviewConfig::parse(config.dump());
      require(loaded.navigation.minus_equal ==
              (std::string(name) == "minus_equal"));
      require(loaded.navigation.comma_period ==
              (std::string(name) == "comma_period"));
      require(loaded.navigation.brackets == (std::string(name) == "brackets"));
      require(loaded.navigation.tab == (std::string(name) == "tab"));
      require(loaded.navigation.page_up_down ==
              (std::string(name) == "page_up_down"));
      require(loaded.navigation.arrows == (std::string(name) == "arrows"));
      require(loaded.navigation.mouse_wheel ==
              (std::string(name) == "mouse_wheel"));
      config["key_bindings"][name] = 1;
      reject(config);
      config["key_bindings"].erase(name);
      // mouse_wheel arrived after the other six, so the parser accepts a
      // binding block written before it existed and reads it as off. The rest
      // are mandatory: a missing one means a partial block, not a default.
      if (std::string(name) == "mouse_wheel")
        require(!PreviewConfig::parse(config.dump()).navigation.mouse_wheel);
      else
        reject(config);
    }
    document["key_bindings"] = bindings;
    for (const auto &[name, expected] :
         std::array<std::pair<const char *, WordCharacterBinding>, 3>{
             {{"disabled", WordCharacterBinding::Disabled},
              {"brackets", WordCharacterBinding::Brackets},
              {"minus_equal", WordCharacterBinding::MinusEqual}}}) {
      document["key_bindings"]["word_character"] = name;
      require(PreviewConfig::parse(document.dump()).word_character == expected);
    }
    document["key_bindings"]["word_character"] = "unknown";
    reject(document);
    document["key_bindings"] = bindings;
    document["key_bindings"]["extra"] = false;
    reject(document);
    document["key_bindings"] = nullptr;
    reject(document);
    document.erase("key_bindings");
    // Appearance is optional; without it the presenters keep their built-ins.
    require(good.skin_directory.empty() && good.dark_theme);
    const auto skins = (root / "skins").u8string();
    document["appearance"] = {{"skin_directory", skins},
                              {"dark_theme", false}};
    const auto themed = PreviewConfig::parse(document.dump());
    require(themed.skin_directory == std::filesystem::u8path(skins) &&
            !themed.dark_theme);
    require(themed.horizontal_candidates); // The shipped default is one row.
    // Supplementary faces and the candidate preedit line are shared settings the
    // window accepts but that nothing used to fill in, so both were inert here.
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_fallback_fonts",
                               nlohmann::json::array({"Microsoft YaHei", "Segoe UI Emoji"})}};
    const auto fonts = PreviewConfig::parse(document.dump());
    require(fonts.candidate_fallback_fonts.size() == 2 &&
            fonts.candidate_fallback_fonts[0] == "Microsoft YaHei" &&
            fonts.candidate_fallback_fonts[1] == "Segoe UI Emoji");
    require(fonts.candidate_show_preedit); // Showing the reading is the default.
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_fallback_fonts",
                               nlohmann::json::array({"", "Segoe UI"})}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_fallback_fonts", nlohmann::json::array({1})}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_fallback_fonts", "Segoe UI"}};
    reject(document);

    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_preedit_style", "empty"}};
    require(!PreviewConfig::parse(document.dump()).candidate_show_preedit);
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_preedit_style", "pinyin"}};
    require(PreviewConfig::parse(document.dump()).candidate_show_preedit);
    // An unknown value must be refused rather than silently picking a layout.
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_preedit_style", "raw"}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins},
                              {"candidate_preedit_style", 1}};
    reject(document);

    document["appearance"] = {{"skin_directory", skins}, {"layout", "vertical"}};
    require(!PreviewConfig::parse(document.dump()).horizontal_candidates);
    document["appearance"] = {{"skin_directory", skins}, {"layout", "horizontal"}};
    require(PreviewConfig::parse(document.dump()).horizontal_candidates);
    document["appearance"] = {{"skin_directory", skins}, {"layout", "grid"}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins}, {"layout", 1}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins}};
    const auto rooted = PreviewConfig::parse(document.dump());
    require(rooted.dark_theme);
    // A skin root has to be absolute and named, like every other preview path.
    document["appearance"] = {{"skin_directory", "skins"}};
    reject(document);
    document["appearance"] = nlohmann::json::object();
    reject(document);
    // The skin selection and the per-colour overrides are gone: the global theme in the stored preferences colours the card, so a document still naming them is refused rather than silently ignored.
    for (const char *retired :
         {"skin", "candidate_text_color", "candidate_number_color",
          "candidate_surface_color", "candidate_border_color",
          "candidate_selected_color", "candidate_hover_color",
          "candidate_accent_color"}) {
      document["appearance"] = {{"skin_directory", skins}, {retired, "#123456"}};
      reject(document);
    }
    document["appearance"] = {{"skin_directory", skins}, {"dark_theme", "no"}};
    reject(document);
    document["appearance"] = {{"skin_directory", skins},
                              {"dark_theme", true},
                              {"layout", "vertical"},
                              {"extra", 1}};
    reject(document);
    document["appearance"] = skins;
    reject(document);
    document.erase("appearance");
    std::cout
        << "Preview configuration: isolated names and strict fields passed\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Preview configuration test failed with an unknown error\n";
    return 1;
  }
}
