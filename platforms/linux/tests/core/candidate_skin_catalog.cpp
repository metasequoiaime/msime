#include "../src/core/CandidateSkinCatalog.h"
#include "../src/core/GlobalTheme.h"

#include <cassert>
#include <string>
#include <vector>

using Json = nlohmann::json;
using msime::linux_host::apply_theme_choice;
using msime::linux_host::candidate_skin_corner_radius;
using msime::linux_host::candidate_skin_package;
using msime::linux_host::CandidateSkinAlign;
using msime::linux_host::current_theme_choice;
using msime::linux_host::find_theme_choice;
using msime::linux_host::parse_configured_skins;
using msime::linux_host::safe_skin_id;
using msime::linux_host::theme_choice_change;
using msime::linux_host::theme_choices;

int main() {
  // The document the desktop settings write (sync_runtime_options, host_candidate_catalog): the manifest name under `title`, the manifest base and layouts, and a palette for exactly the declared modes, empty when a mode sets no colour.
  const auto options = Json::parse(
      R"({"candidate_skin_catalog":{"packages":[)"
      R"({"id":"solarized","title":"Solarized","base":"system","layouts":["horizontal","vertical"],"candidate":{"light":{},"dark":{"surface":"#002B36"}}},)"
      R"({"id":"sakura","title":"樱花","base":"light","layouts":["vertical"],"candidate":{"light":{"surface":"#FFF0F5","show_selected_bar":false}}},)"
      R"({"id":"unsafe/id","title":"Ignored","base":"system","layouts":["vertical"]},)"
      R"({"id":"untitled","title":"","base":"system","layouts":["vertical"]},)"
      R"({"id":"baseless","title":"No base","layouts":["vertical"]},)"
      R"({"id":"layoutless","title":"No layouts","base":"system"},)"
      R"({"id":"sideways","title":"Diagonal","base":"system","layouts":["diagonal"]},)"
      R"({"id":"over-custom","title":"Over custom","base":"custom","layouts":["vertical"]},)"
      R"({"id":"solarized","title":"Twice","base":"system","layouts":["vertical"]},)"
      R"({"id":"paper","title":"重名","base":"system","layouts":["vertical"]},)"
      R"({"id":"retired","title":"Retired base","base":"fluent","layouts":["vertical"]}]}})");
  // Only what the shared layer would accept as a package is listed: a safe id, a title, a base and known layouts. The palettes are not read here, so empty ones are fine; a duplicate id keeps its first entry.
  const auto packages = parse_configured_skins(options);
  assert(packages.size() == 4);
  assert(packages[0].id == "solarized" && packages[0].title == "Solarized" && packages[0].base == "system");
  assert(packages[1].id == "sakura" && packages[1].base == "light");
  assert(packages[2].id == "paper");
  assert(packages[3].id == "retired" && packages[3].base == "fluent");
  assert(!safe_skin_id("unsafe/id"));
  assert(!safe_skin_id(""));
  assert(safe_skin_id("solarized"));
  assert(parse_configured_skins(Json::object()).empty());
  assert(parse_configured_skins(Json{{"candidate_skin_catalog", {{"packages", Json::object()}}}}).empty());

  // The package entry goes to msime_client_resolve_theme unchanged.
  const auto &catalog = options["candidate_skin_catalog"];
  assert(*candidate_skin_package(catalog, "sakura") == catalog["packages"][1]);
  assert(!candidate_skin_package(catalog, "absent"));
  assert(!candidate_skin_package(catalog, ""));
  assert(!candidate_skin_package(Json(), "sakura"));

  // The menu: the shared catalogue's themes in its order, then the packages; one named like a theme, or drawn over a base that is not a catalogue theme other than 自定义, is refused by the shared layer and left out.
  const auto theme_catalog = Json::parse(
      R"({"themes":[{"id":"system","title":"跟随系统"},{"id":"shuishan","title":"水杉"},{"id":"light","title":"浅色"},)"
      R"({"id":"paper","title":"纸白"},{"id":"night","title":"夜青"},{"id":"ink","title":"墨"},{"id":"custom","title":"自定义"}],)"
      R"("default":"system"})");
  const auto choices = theme_choices(theme_catalog, packages);
  assert(choices.size() == 9);
  assert(choices.front().id == "system" && choices.front().title == "跟随系统" && !choices.front().package_base);
  assert(choices[6].id == "custom" && !choices[6].package_base);
  assert(choices[7].id == "solarized" && choices[7].package_base == "system");
  assert(choices[8].id == "sakura" && choices[8].title == "樱花" && choices[8].package_base == "light");
  assert(!find_theme_choice(choices, "retired"));
  // Without the shared catalogue nothing is listed: the host keeps no list of its own, so it cannot tell a package's base is one the shared layer draws over either.
  assert(theme_choices(Json::object(), packages).empty());

  // The selected entry: the global theme, or the package a custom theme is drawn over.
  assert(current_theme_choice(Json::object(), choices) == "system");
  assert(current_theme_choice(Json{{"global_theme", "night"}}, choices) == "night");
  const Json over_sakura = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}};
  assert(current_theme_choice(over_sakura, choices) == "sakura");
  auto uninstalled = over_sakura;
  uninstalled["custom_theme"]["candidate_skin"] = "gone";
  assert(current_theme_choice(uninstalled, choices) == "custom");
  auto shown_elsewhere = over_sakura;
  shown_elsewhere["global_theme"] = "ink";
  assert(current_theme_choice(shown_elsewhere, choices) == "ink");

  // A built-in theme only selects itself; the custom theme is kept for when it comes back.
  const Json keyboard = {{"background", "#000000"}};
  Json preferences = {{"global_theme", "custom"},
                      {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}, {"keyboard", keyboard}}}};
  auto change = theme_choice_change(choices, "night");
  assert(change && *change == Json({{"global_theme", "night"}}));
  apply_theme_choice(preferences, *change);
  assert(preferences.at("global_theme") == "night" && preferences.at("custom_theme").at("candidate_skin") == "sakura");
  // A package selects the custom theme over it and its manifest base, as the settings page's package card does, and keeps the rest.
  change = theme_choice_change(choices, "solarized");
  assert(change && change->at("global_theme") == "custom");
  apply_theme_choice(preferences, *change);
  assert(preferences.at("custom_theme").at("candidate_skin") == "solarized");
  assert(preferences.at("custom_theme").at("base") == "system");
  assert(preferences.at("custom_theme").at("keyboard") == keyboard);
  assert(current_theme_choice(preferences, choices) == "solarized");
  // 自定义 selects the custom theme as it stands, as the settings page's 自定义 card does: the stored package, base and keyboard are not edited, so a custom theme over a listed package shows that package as chosen.
  change = theme_choice_change(choices, "custom");
  assert(change && *change == Json({{"global_theme", "custom"}}));
  const auto before_custom = preferences;
  apply_theme_choice(preferences, *change);
  assert(preferences == before_custom);
  assert(current_theme_choice(preferences, choices) == "solarized");
  Json stored_package = {{"global_theme", "ink"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}};
  apply_theme_choice(stored_package, *theme_choice_change(choices, "custom"));
  assert(stored_package == Json({{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}}));
  assert(current_theme_choice(stored_package, choices) == "sakura");
  // Without a listed package the menu shows 自定义 itself.
  assert(current_theme_choice(uninstalled, choices) == "custom");
  // A preferences document without a custom theme gets one.
  Json bare = Json::object();
  apply_theme_choice(bare, *theme_choice_change(choices, "sakura"));
  assert(bare.at("custom_theme") == Json({{"candidate_skin", "sakura"}, {"base", "light"}}));
  // Retired skin ids and anything else the menu does not list change nothing.
  assert(!theme_choice_change(choices, "willow_green"));
  assert(!theme_choice_change(choices, "unsafe/id"));

  // A decoration comes with its bounds and an absolute image path, as the shared host catalog publishes it; IBus reads the same package and simply has no use for it.
  using msime::linux_host::candidate_skin_decoration;
  using msime::linux_host::parse_skin_decoration;
  const auto decorated = nlohmann::json::parse(
      R"({"id":"sakura","title":"樱花","base":"light","layouts":["vertical"],"decoration_top_dip":24.5,"decoration_width_dip":180,)"
      R"("decoration_image":"/home/u/.local/share/msime/skins/sakura/images/ears.png"})");
  const auto decoration = parse_skin_decoration(decorated);
  assert(decoration && decoration->top_dip == 24.5 && decoration->width_dip == 180);
  assert(decoration->image == "/home/u/.local/share/msime/skins/sakura/images/ears.png");
  assert(parse_configured_skins(nlohmann::json{{"candidate_skin_catalog", {{"packages", nlohmann::json::array({decorated})}}}}).size() == 1);
  // The manifest's own bounds hold at both ends.
  auto edge = decorated;
  edge["decoration_top_dip"] = 500;
  edge["decoration_width_dip"] = 1000;
  assert(parse_skin_decoration(edge));
  // Anything outside them, a missing key, a relative or embedded-NUL path, and the package keeps its place with no decoration.
  const auto rejected = [&decorated](const char *key, const nlohmann::json &value) {
    auto package = decorated;
    if (value.is_discarded()) package.erase(key);
    else package[key] = value;
    return !parse_skin_decoration(package);
  };
  const auto absent = nlohmann::json(nlohmann::json::value_t::discarded);
  assert(rejected("decoration_top_dip", 0));
  assert(rejected("decoration_top_dip", -1));
  assert(rejected("decoration_top_dip", 500.5));
  assert(rejected("decoration_top_dip", "24"));
  assert(rejected("decoration_top_dip", absent));
  assert(rejected("decoration_width_dip", 0));
  assert(rejected("decoration_width_dip", 1000.1));
  assert(rejected("decoration_width_dip", absent));
  assert(rejected("decoration_image", "images/ears.png"));
  assert(rejected("decoration_image", ""));
  assert(rejected("decoration_image", std::string("/skins/a\0b.png", 14)));
  assert(rejected("decoration_image", "/" + std::string(4096, 'a')));
  assert(rejected("decoration_image", 7));
  assert(rejected("decoration_image", absent));
  assert(!parse_skin_decoration(nlohmann::json::array()));
  // Only the package the resolved theme names is decorated; nothing is when it names none.
  const nlohmann::json decorated_catalog = {{"packages", nlohmann::json::array({decorated})}};
  assert(candidate_skin_decoration(decorated_catalog, "sakura"));
  assert(!candidate_skin_decoration(decorated_catalog, ""));
  assert(!candidate_skin_decoration(decorated_catalog, "absent"));
  assert(!candidate_skin_decoration(catalog, "sakura"));
  assert(!candidate_skin_decoration(nlohmann::json(), "sakura"));
  // The alignment the shared catalog publishes; absent or unknown is the trailing edge.
  assert(parse_skin_decoration(decorated)->align == CandidateSkinAlign::right);
  auto aligned = decorated;
  aligned["decoration_align"] = "left";
  assert(parse_skin_decoration(aligned)->align == CandidateSkinAlign::left);
  aligned["decoration_align"] = "center";
  assert(parse_skin_decoration(aligned)->align == CandidateSkinAlign::center);
  aligned["decoration_align"] = "top";
  assert(parse_skin_decoration(aligned)->align == CandidateSkinAlign::right);
  // The card radius of the drawn skin, bounded like the manifest.
  auto rounded = decorated;
  rounded["corner_radius_dip"] = 12.0;
  const nlohmann::json rounded_catalog = {{"packages", nlohmann::json::array({rounded})}};
  assert(candidate_skin_corner_radius(rounded_catalog, "sakura") == 12.0);
  assert(!candidate_skin_corner_radius(rounded_catalog, ""));
  assert(!candidate_skin_corner_radius(decorated_catalog, "sakura"));
  for (const nlohmann::json &value : {nlohmann::json(33), nlohmann::json(-1), nlohmann::json("12")}) {
    rounded["corner_radius_dip"] = value;
    assert(!candidate_skin_corner_radius(nlohmann::json{{"packages", nlohmann::json::array({rounded})}}, "sakura"));
  }
  return 0;
}
