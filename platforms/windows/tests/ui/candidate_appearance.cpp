#include "CandidateAppearance.h"
#include "PreviewConfig.h"
#include <iostream>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate appearance test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    const auto root = std::filesystem::temp_directory_path();
    const auto state = root / "state";
    // Everything the appearance block is assembled from must survive
    // PreviewConfig, so each case below is fed straight back through it.
    auto loads = [&](const nlohmann::json &appearance) {
      nlohmann::json document{
          {"format_version", 1},
          {"resources", (root / "resources").u8string()},
          {"state_root", state.u8string()},
          {"pipe_namespace", "production"},
          {"preedit_style", "local"},
          {"appearance", appearance}};
      return PreviewConfig::parse(document.dump());
    };

    // A store with nothing set still yields a loadable block, and the skin root
    // is the state directory's skins folder.
    const auto empty = candidate_appearance(state, nlohmann::json::object(), false);
    const auto defaults = loads(empty);
    require(defaults.skin_directory == state / "skins");
    require(defaults.dark_theme);          // The shipped card is dark.
    require(!defaults.horizontal_candidates); // The stored default is vertical.
    require(defaults.candidate_show_preedit);
    require(defaults.candidate_font == "Segoe UI");

    // The user's choices reach the card.
    nlohmann::json preferences{
        {"theme", "light"},
        {"candidate_layout", "horizontal"},
        {"candidate_preedit_style", "empty"},
        {"global_theme", "paper"},
        {"candidate_font_size", 22},
        {"candidate_preedit_font_size", 18},
        {"candidate_font_family", "Microsoft YaHei"},
        {"candidate_english_font", "Synthetic Latin"},
        {"candidate_fallback_fonts",
         nlohmann::json::array({"Segoe UI Emoji", "Noto Color Emoji"})}};
    const auto appearance = candidate_appearance(state, preferences, false);
    const auto configured = loads(appearance);
    require(!configured.dark_theme);
    require(configured.horizontal_candidates);
    require(!configured.candidate_show_preedit);
    require(configured.candidate_font_size == 22);
    require(configured.candidate_preedit_font_size == 18);
    require(configured.candidate_font == "Synthetic Latin");
    require(configured.candidate_fallback_fonts.size() == 2 &&
            configured.candidate_fallback_fonts[0] == "Segoe UI Emoji");
    // Colours are resolved from the global theme at run time, so the appearance block names no skin and no colour.
    require(!appearance.contains("skin") && !appearance.contains("global_theme"));
    for (const auto &entry : appearance.items())
      require(entry.key().find("color") == std::string::npos);

    // "system" is the only value that defers to Windows, and it defers both ways.
    require(!candidate_appearance(state, {{"theme", "system"}}, false)
                 .at("dark_theme")
                 .get<bool>());
    require(candidate_appearance(state, {{"theme", "system"}}, true)
                .at("dark_theme")
                .get<bool>());
    // An explicit choice ignores the system entirely.
    require(!candidate_appearance(state, {{"theme", "light"}}, true)
                 .at("dark_theme")
                 .get<bool>());
    require(candidate_appearance(state, {{"theme", "dark"}}, false)
                .at("dark_theme")
                .get<bool>());

    // The point of the filtering: a stored value PreviewConfig would reject is
    // dropped, so it costs the user a font rather than their IME.
    const nlohmann::json hostile{
        {"candidate_font_size", 900},
        {"candidate_preedit_font_size", 0},
        {"candidate_text_color", std::string(64, 'x')},
        {"candidate_number_color", std::string("rgb(0,0,0)\nInjected")},
        {"candidate_english_font", std::string(400, 'y')},
        {"candidate_skin", std::string(200, 'z')},
        {"candidate_fallback_fonts",
         nlohmann::json::array({"", std::string(400, 'w'), 7, "Segoe UI"})},
        {"theme", 12},
        {"candidate_layout", nullptr}};
    const auto filtered = candidate_appearance(state, hostile, false);
    require(!filtered.contains("candidate_font_size"));
    require(!filtered.contains("candidate_preedit_font_size"));
    require(!filtered.contains("candidate_text_color"));
    require(!filtered.contains("candidate_number_color"));
    require(!filtered.contains("candidate_font"));
    require(!filtered.contains("candidate_skin"));
    require(!filtered.contains("candidate_fallback_fonts"));
    // Non-string theme/layout fall back rather than propagating a bad type.
    require(filtered.at("dark_theme").get<bool>());
    require(filtered.at("layout") == "vertical");
    const auto survived = loads(filtered); // The whole point: it still loads.
    require(survived.candidate_font == "Segoe UI");

    // Only the three known inline-preedit values are passed through, and the
    // stored "raw" is PreviewConfig's "local".
    require(tsf_preedit_style(nlohmann::json::object()) == "local");
    require(tsf_preedit_style({{"tsf_preedit_style", "raw"}}) == "local");
    require(tsf_preedit_style({{"tsf_preedit_style", "pinyin"}}) == "pinyin");
    require(tsf_preedit_style({{"tsf_preedit_style", "empty"}}) == "empty");
    require(tsf_preedit_style({{"tsf_preedit_style", "nonsense"}}) == "local");
    require(tsf_preedit_style({{"tsf_preedit_style", 3}}) == "local");

    // The floating toolbar reads the same store, with the same rule: a value
    // PreviewConfig would refuse is dropped rather than copied through.
    auto toolbar_document = [&](const nlohmann::json &preferences) {
      nlohmann::json document{
          {"format_version", 1},
          {"resources", (root / "resources").u8string()},
          {"state_root", state.u8string()},
          {"pipe_namespace", "production"},
          {"preedit_style", "local"}};
      apply_floating_toolbar(document, preferences);
      return document;
    };
    // No stored block leaves the document untouched, so the defaults stand.
    require(!toolbar_document(nlohmann::json::object())
                 .contains("floating_toolbar_enabled"));
    require(!toolbar_document({{"floating_toolbar", 7}})
                 .contains("floating_toolbar_enabled"));

    const nlohmann::json full{
        {"floating_toolbar",
         {{"enabled", false},
          {"scale_percent", 125},
          {"font_size", 20},
          {"character_set", true},
          {"punctuation", false},
          {"fullwidth", true},
          {"emoji", false},
          {"screen_keyboard", true},
          {"settings", false}}}};
    const auto bar = PreviewConfig::parse(toolbar_document(full).dump());
    require(!bar.floating_toolbar_enabled);
    // Stored as a percentage, carried as a multiplier.
    require(bar.floating_toolbar_scale > 1.249 &&
            bar.floating_toolbar_scale < 1.251);
    require(bar.floating_toolbar_font_size == 20);
    require(bar.floating_toolbar_items[0] && !bar.floating_toolbar_items[1] &&
            bar.floating_toolbar_items[2] && !bar.floating_toolbar_items[3] &&
            bar.floating_toolbar_items[4] && !bar.floating_toolbar_items[5]);

    // Out-of-range scale and size are dropped, and the document still loads.
    auto hostile_toolbar = full;
    hostile_toolbar["floating_toolbar"]["scale_percent"] = 900;
    hostile_toolbar["floating_toolbar"]["font_size"] = 2;
    const auto dropped = toolbar_document(hostile_toolbar);
    require(!dropped.contains("floating_toolbar_scale"));
    require(!dropped.contains("floating_toolbar_font_size"));
    require(PreviewConfig::parse(dropped.dump()).floating_toolbar_font_size == 24);

    // PreviewConfig demands exactly six item keys, so a partial block must be
    // omitted entirely rather than sent and refused.
    auto partial = full;
    partial["floating_toolbar"].erase("emoji");
    const auto without_items = toolbar_document(partial);
    require(!without_items.contains("floating_toolbar_items"));
    require(without_items.contains("floating_toolbar_enabled"));
    PreviewConfig::parse(without_items.dump()); // Still loads.
    // A non-boolean item is the same case: six good keys or none.
    auto mistyped = full;
    mistyped["floating_toolbar"]["emoji"] = "yes";
    require(!toolbar_document(mistyped).contains("floating_toolbar_items"));

    std::cout << "Candidate appearance: stored preferences reach the card\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Candidate appearance test failed with an unknown error\n";
    return 1;
  }
}
