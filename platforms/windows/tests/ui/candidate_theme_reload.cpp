#include "../../src/candidate/CandidateAppearance.h"
#include <cassert>

int main() {
  using namespace msime::windows;
  for (bool system_dark : {false, true}) {
    for (const char *global : {"dark", "light", "system"}) {
      for (const char *surface : {"follow", "dark", "light"}) {
        const nlohmann::json values{{"theme", global},
                                    {"candidate_theme", surface}};
        const bool expected =
            std::string(surface) == "dark" ||
            (std::string(surface) == "follow" &&
             (std::string(global) == "dark" ||
              (std::string(global) == "system" && system_dark)));
        assert(candidate_theme_dark(values, system_dark) == expected);
        assert(candidate_appearance("state", values, system_dark)
                   .at("dark_theme") == expected);
      }
    }
  }
  // The request for one surface: the stored global theme (system when none is stored), that surface's own mode, the layout being drawn and the absolute skin root.
  const auto root = std::filesystem::temp_directory_path() / "msime-skins";
  const nlohmann::json custom{
      {"base", "system"},
      {"candidate_skin", "sample"},
      {"candidate_colors", {{"accent", "#07C160"}}}};
  const auto stored = candidate_theme_values(
      {{"theme", "system"},
       {"global_theme", "custom"},
       {"custom_theme",
        {{"base", "system"},
         {"candidate_skin", "sample"},
         {"candidate_colors", {{"accent", "#07C160"}}},
         {"keyboard", {{"background", "photo.jpg"}}}}},
       {"unrelated", "synthetic"}});
  // The custom keyboard (which may carry a photo) colours nothing here, so it is not copied across threads.
  assert(stored.size() == 3 && stored.at("custom_theme") == custom);
  const auto request = candidate_theme_request(stored, true, false, root);
  assert(request.at("global_theme") == "custom" && request.at("dark") == true &&
         request.at("layout") == "vertical" &&
         request.at("custom_theme") == custom &&
         request.at("skins_directory") == root.u8string());
  assert(request.size() == 5);
  const auto fallback = candidate_theme_request(nlohmann::json::object(), false,
                                                true, std::filesystem::path());
  assert(fallback == nlohmann::json({{"global_theme", "system"},
                                     {"dark", false},
                                     {"layout", "horizontal"}}));
  // The shared layer refuses a relative root, so one is never sent.
  assert(!candidate_theme_request(stored, false, false, "relative/skins")
              .contains("skins_directory"));

  // A system answer leaves every slot to the native tokens of the surface's own mode.
  const auto native = candidate_theme_resolution(
      {{"id", "system"},
       {"source", "system"},
       {"appearance", nullptr},
       {"candidate", nullptr},
       {"keyboard", nullptr},
       {"candidate_skin", nullptr}});
  assert(native && !native->dark && native->candidate_skin.empty());
  assert(candidate_theme_palette(*native, true).surface ==
         candidate_native_palette(true).surface);
  assert(candidate_theme_palette(*native, false).surface ==
         candidate_native_palette(false).surface);
  // An empty resolution is the fallback when the shared layer refuses a request, and draws the same.
  assert(candidate_theme_palette(CandidateThemeResolution{}, true).text ==
         candidate_native_palette(true).text);

  // A built-in theme fixes its mode and sets its slots; null slots keep the native token of that mode.
  const auto paper = candidate_theme_resolution(
      {{"id", "paper"},
       {"source", "builtin"},
       {"appearance", "light"},
       {"candidate",
        {{"surface", "#F7F1E3"},
         {"border", "#0000001A"},
         {"text", "#2B2B2B"},
         {"number", nullptr},
         {"secondary", "#6B6B6B"},
         {"accent", "#8A5A2B"},
         {"selected", nullptr},
         {"selected_text", nullptr},
         {"selected_number", nullptr},
         {"hover", "#0000000F"},
         {"show_selected_bar", true}}},
       {"keyboard", nullptr},
       {"candidate_skin", nullptr}});
  assert(paper && paper->dark && !*paper->dark);
  // The card is drawn light even when the surface asked for dark.
  const auto papered = candidate_theme_palette(*paper, true);
  assert(papered.surface == candidate_rgb(0xF7F1E3) &&
         papered.text == candidate_rgb(0x2B2B2B) &&
         papered.accent == candidate_rgb(0x8A5A2B));
  assert(papered.number == candidate_light_palette().number);
  assert(papered.selected == papered.hover &&
         papered.selected_text == papered.accent && papered.show_selected_bar);
  assert(papered.menu_fill == papered.surface);

  // A custom theme that draws a package names it, and the assets follow that name.
  auto packaged = nlohmann::json{{"appearance", "dark"},
                                 {"candidate", {{"show_selected_bar", false}}},
                                 {"candidate_skin", "sample"}};
  const auto named = candidate_theme_resolution(packaged);
  assert(named && named->dark && *named->dark &&
         named->candidate_skin == "sample" &&
         !candidate_theme_palette(*named, false).show_selected_bar);

  // A shape this reader does not recognise is refused as a whole.
  for (const auto &hostile : {
           nlohmann::json(7),
           nlohmann::json{{"appearance", "sepia"}},
           nlohmann::json{{"candidate", "flat"}},
           nlohmann::json{{"candidate", {{"text", 12}}}},
           nlohmann::json{{"candidate", {{"text", std::string(100, 'x')}}}},
           nlohmann::json{{"candidate", {{"show_selected_bar", "yes"}}}},
           nlohmann::json{{"candidate_skin", "../escape"}},
           nlohmann::json{{"candidate_skin", 3}},
       })
    assert(!candidate_theme_resolution(hostile));
  // An unparsable colour string keeps the native token rather than drawing something invented.
  const auto garbled = candidate_theme_resolution(
      {{"candidate", {{"surface", "not a colour"}}}});
  assert(garbled && candidate_theme_palette(*garbled, true).surface ==
                        candidate_native_palette(true).surface);

  // Only the display projection crosses the preference/UI thread boundary, and only the latest one.
  CandidateThemeMailbox mailbox;
  assert(!mailbox.take());
  mailbox.publish({{"global_theme", "ink"}});
  mailbox.publish({{"candidate_theme", "light"},
                   {"unrelated", "synthetic"},
                   {"global_theme", std::string(100, 'x')}});
  const auto latest = mailbox.take();
  assert(latest && latest->size() == 1 &&
         latest->at("candidate_theme") == "light");
  assert(!mailbox.take());

  // The tray menu names the selected theme from the shared catalog, and names nothing when the catalog cannot say.
  const nlohmann::json catalog{
      {"ok", true},
      {"value",
       {{"default", "system"},
        {"themes",
         {{{"id", "system"}, {"title", "System"}},
          {{"id", "night"}, {"title", "Night"}},
          {{"id", "ink"}, {"title", 7}},
          {{"id", "long"}, {"title", std::string(65, 'x')}},
          "garbled"}}}}};
  assert(theme_catalog_title(catalog, "night") == "Night");
  assert(theme_catalog_title(catalog, "system") == "System");
  assert(theme_catalog_title(catalog, "paper").empty());
  assert(theme_catalog_title(catalog, "ink").empty());
  assert(theme_catalog_title(catalog, "long").empty());
  assert(theme_catalog_title({{"ok", false}, {"value", catalog.at("value")}},
                             "night")
             .empty());
  assert(theme_catalog_title({{"ok", true}, {"value", {{"themes", 3}}}}, "night")
             .empty());
  assert(theme_catalog_title(nlohmann::json::array(), "night").empty());
  assert(theme_catalog_title(nlohmann::json(), "night").empty());
}
