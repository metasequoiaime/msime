#include "../src/core/CandidateSkinCatalog.h"
#include "../src/core/GlobalTheme.h"

#include <cassert>
#include <string>
#include <vector>

using Json = nlohmann::json;
using msime::linux_host::apply_theme_choice;
using msime::linux_host::candidate_corner_radius;
using msime::linux_host::candidate_corner_radius_preference;
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
  Json oversized = Json{{"candidate_skin_catalog", { {"packages", Json::array()} }}};
  for (int index = 0; index < 40; ++index) {
    oversized["candidate_skin_catalog"]["packages"].push_back(
        Json{{"id", "skin" + std::to_string(index)}, {"title", "Skin"},
             {"base", "system"}, {"layouts", Json::array({"vertical"})}});
  }
  assert(parse_configured_skins(oversized).size() == 32);
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
      R"({"themes":[{"id":"system","title":"跟随系统","appearance":null},{"id":"shuishan","title":"水杉","appearance":"dark"},)"
      R"({"id":"light","title":"浅色","appearance":"light"},{"id":"paper","title":"纸白","appearance":"light"},)"
      R"({"id":"night","title":"夜青","appearance":"dark"},{"id":"ink","title":"墨","appearance":"dark"},)"
      R"({"id":"custom","title":"自定义","appearance":null}],)"
      R"("default":"system"})");
  const auto choices = theme_choices(theme_catalog, packages);
  assert(choices.size() == 9);
  assert(choices.front().id == "system" && choices.front().title == "跟随系统" && !choices.front().package_base);
  assert(choices[6].id == "custom" && !choices[6].package_base);
  assert(choices[7].id == "solarized" && choices[7].package_base == "system" && choices[7].package_slot.empty());
  assert(choices[8].id == "sakura" && choices[8].title == "樱花" && choices[8].package_base == "light" &&
         choices[8].package_slot == "light");
  assert(!find_theme_choice(choices, "retired"));
  // Without the shared catalogue nothing is listed: the host keeps no list of its own, so it cannot tell a package's base is one the shared layer draws over either.
  assert(theme_choices(Json::object(), packages).empty());

  // The selected entry: the global theme, or the package a custom theme is drawn over.
  assert(current_theme_choice(Json::object(), choices, false) == "system");
  assert(current_theme_choice(Json{{"global_theme", "night"}}, choices, false) == "night");
  const Json over_sakura = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}};
  assert(current_theme_choice(over_sakura, choices, false) == "sakura");
  auto uninstalled = over_sakura;
  uninstalled["custom_theme"]["candidate_skin"] = "gone";
  assert(current_theme_choice(uninstalled, choices, false) == "custom");
  auto shown_elsewhere = over_sakura;
  shown_elsewhere["global_theme"] = "ink";
  assert(current_theme_choice(shown_elsewhere, choices, false) == "ink");

  // A built-in theme only selects itself; the custom theme is kept for when it comes back.
  const Json keyboard = {{"background", "#000000"}};
  Json preferences = {{"global_theme", "custom"},
                      {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}, {"keyboard", keyboard}}}};
  auto change = theme_choice_change(preferences, choices, "night");
  assert(change && *change == Json({{"global_theme", "night"}}));
  apply_theme_choice(preferences, *change);
  assert(preferences.at("global_theme") == "night" && preferences.at("custom_theme").at("candidate_skin") == "sakura");
  // A package selects the custom theme over it and its manifest base, as the settings page's package card does, and keeps the rest.
  change = theme_choice_change(preferences, choices, "solarized");
  assert(change && change->at("global_theme") == "custom");
  apply_theme_choice(preferences, *change);
  assert(preferences.at("custom_theme").at("candidate_skin") == "solarized");
  assert(preferences.at("custom_theme").at("base") == "system");
  assert(preferences.at("custom_theme").at("keyboard") == keyboard);
  assert(current_theme_choice(preferences, choices, false) == "solarized");
  // 自定义 selects the custom theme as it stands, as the settings page's 自定义 card does: the stored package, base and keyboard are not edited, so a custom theme over a listed package shows that package as chosen.
  change = theme_choice_change(preferences, choices, "custom");
  assert(change && *change == Json({{"global_theme", "custom"}}));
  const auto before_custom = preferences;
  apply_theme_choice(preferences, *change);
  assert(preferences == before_custom);
  assert(current_theme_choice(preferences, choices, false) == "solarized");
  Json stored_package = {{"global_theme", "ink"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}};
  apply_theme_choice(stored_package, *theme_choice_change(stored_package, choices, "custom"));
  assert(stored_package == Json({{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}}));
  assert(current_theme_choice(stored_package, choices, false) == "sakura");
  // Without a listed package the menu shows 自定义 itself.
  assert(current_theme_choice(uninstalled, choices, false) == "custom");
  // A preferences document without a custom theme gets one.
  Json bare = Json::object();
  apply_theme_choice(bare, *theme_choice_change(bare, choices, "sakura"));
  assert(bare.at("custom_theme") == Json({{"candidate_skin", "sakura"}, {"base", "light"}}));
  // Retired skin ids and anything else the menu does not list change nothing.
  assert(!theme_choice_change(bare, choices, "willow_green"));
  assert(!theme_choice_change(bare, choices, "unsafe/id"));

  // 两个槽位：皮肤包的槽位是它清单 base 在主题目录里的明暗。合成的皮肤 dusk、starry 以深色主题为底，paper-notes 以浅色为底，mist 跟随系统；规则与设置页的 applyCandidateSkin 及其测试 apps/desktop/tests/candidate/candidate-skin-slots.test.ts 一一对应。
  auto slot_packages = packages;
  slot_packages.push_back({"dusk", "黄昏", "night"});
  slot_packages.push_back({"starry", "星空", "ink"});
  slot_packages.push_back({"paper-notes", "便笺", "paper"});
  slot_packages.push_back({"mist", "雾", "system"});
  const auto slotted = theme_choices(theme_catalog, slot_packages);
  assert(find_theme_choice(slotted, "dusk")->package_slot == "dark");
  assert(find_theme_choice(slotted, "starry")->package_slot == "dark");
  assert(find_theme_choice(slotted, "paper-notes")->package_slot == "light");
  assert(find_theme_choice(slotted, "mist")->package_slot.empty());
  const auto choose = [&](Json preferences, std::string_view id) {
    const auto made = theme_choice_change(preferences, slotted, id);
    assert(made && made->at("global_theme") == "custom");
    apply_theme_choice(preferences, *made);
    return preferences;
  };
  // 每款皮肤只填自己的槽位，base 照旧写成包的 base。
  auto slots = choose(Json{{"custom_theme", {{"candidate_skin_dark", "dusk"}}}}, "sakura");
  assert(slots.at("custom_theme") == Json({{"base", "light"}, {"candidate_skin", "sakura"}, {"candidate_skin_dark", "dusk"}}));
  slots = choose(slots, "starry");
  assert(slots.at("custom_theme") == Json({{"base", "ink"}, {"candidate_skin", "sakura"}, {"candidate_skin_dark", "starry"}}));
  slots = choose(slots, "mist");
  assert(slots.at("custom_theme") == Json({{"base", "system"}, {"candidate_skin", "mist"}, {"candidate_skin_dark", "mist"}}));
  // 只设过一款深色皮肤的旧文档把它存在 candidate_skin 里：选浅色皮肤时先把它挪进深色槽位。
  const Json legacy_dark = {{"global_theme", "custom"}, {"custom_theme", {{"base", "ink"}, {"candidate_skin", "starry"}}}};
  assert(choose(legacy_dark, "sakura").at("custom_theme") ==
         Json({{"base", "light"}, {"candidate_skin", "sakura"}, {"candidate_skin_dark", "starry"}}));
  // 原来是浅色皮肤时不挪，深色槽位保持空着；不在菜单里、不知道明暗的旧皮肤直接覆盖，也不挪。
  const auto replaced = choose(Json{{"custom_theme", {{"candidate_skin", "paper-notes"}}}}, "sakura");
  assert(replaced.at("custom_theme").at("candidate_skin") == "sakura" &&
         !replaced.at("custom_theme").contains("candidate_skin_dark"));
  assert(!choose(Json{{"custom_theme", {{"candidate_skin", "gone"}}}}, "sakura").at("custom_theme").contains("candidate_skin_dark"));
  // 跟随系统的旧皮肤确知能在深色模式画，照样挪过去。
  assert(choose(Json{{"custom_theme", {{"candidate_skin", "mist"}}}}, "sakura").at("custom_theme").at("candidate_skin_dark") == "mist");
  // 新的深色皮肤取代旧文档放在 candidate_skin 里的深色皮肤，那个键随之删掉。
  assert(choose(legacy_dark, "dusk").at("custom_theme") == Json({{"base", "night"}, {"candidate_skin_dark", "dusk"}}));
  // 浅色皮肤留在 candidate_skin 里不动。
  assert(choose(Json{{"custom_theme", {{"candidate_skin", "sakura"}}}}, "dusk").at("custom_theme") ==
         Json({{"base", "night"}, {"candidate_skin", "sakura"}, {"candidate_skin_dark", "dusk"}}));

  // 勾选：任一槽位的皮肤都算当前，先取候选窗当前明暗下的那款，这个模式的槽位没有可列出的包时再取另一个槽位的。
  const Json both = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"candidate_skin_dark", "dusk"}}}};
  assert(current_theme_choice(both, slotted, false) == "sakura");
  assert(current_theme_choice(both, slotted, true) == "dusk");
  const Json dark_only = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin_dark", "dusk"}}}};
  assert(current_theme_choice(dark_only, slotted, false) == "dusk");
  assert(current_theme_choice(dark_only, slotted, true) == "dusk");
  auto dark_gone = both;
  dark_gone["custom_theme"]["candidate_skin_dark"] = "gone";
  assert(current_theme_choice(dark_gone, slotted, true) == "sakura");
  // 深色模式没设 candidate_skin_dark 时与浅色模式一样取 candidate_skin。
  assert(current_theme_choice(legacy_dark, slotted, true) == "starry");

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
  // The user's radius from the settings wins over the drawn skin's; unset or out of range leaves it to the skin, and with neither the host keeps its own.
  assert(candidate_corner_radius_preference(Json{{"candidate_corner_radius", 4}}) == 4.0);
  assert(candidate_corner_radius_preference(Json{{"candidate_corner_radius", 0}}) == 0.0);
  assert(!candidate_corner_radius_preference(Json::object()));
  assert(!candidate_corner_radius_preference(Json()));
  for (const Json &value : {Json(33), Json(-1), Json("8"), Json(nullptr)})
    assert(!candidate_corner_radius_preference(Json{{"candidate_corner_radius", value}}));
  assert(candidate_corner_radius(Json{{"candidate_corner_radius", 0}}, rounded_catalog, "sakura") == 0.0);
  assert(candidate_corner_radius(Json{{"candidate_corner_radius", 20}}, decorated_catalog, "sakura") == 20.0);
  assert(candidate_corner_radius(Json::object(), rounded_catalog, "sakura") == 12.0);
  assert(candidate_corner_radius(Json{{"candidate_corner_radius", 40}}, rounded_catalog, "sakura") == 12.0);
  assert(!candidate_corner_radius(Json::object(), decorated_catalog, "sakura"));
  return 0;
}
