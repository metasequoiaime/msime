#include "../../settings/ThemeSkinModel.h"
#include <cstdio>
#include <iterator>
#include <map>
#include <string>

using namespace msime::settings::skins;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
CustomSkins skins(std::wstring base, std::wstring light, std::wstring dark) {
  CustomSkins value;
  value.base = std::move(base);
  value.light = std::move(light);
  value.dark = std::move(dark);
  return value;
}
// 目录：day 是浅色底，night 是深色底，auto 是 system 底；目录里没有的包槽位未知。
std::optional<Slot> catalog_slot(std::wstring const &id) {
  static const std::map<std::wstring, Slot> slots{
      {L"day", Slot::light}, {L"night", Slot::dark}, {L"auto", Slot::either}};
  const auto found = slots.find(id);
  if (found == slots.end())
    return std::nullopt;
  return found->second;
}
std::vector<Listed> catalog() {
  return {{L"day", L"白昼", Slot::light}, {L"night", L"夜色", Slot::dark},
          {L"auto", L"四季", Slot::either}};
}
} // namespace

int main() {
  // customThemeBase
  check(custom_theme_base(L"custom", L"paper") == L"paper", "a selected custom theme keeps its base");
  check(custom_theme_base(L"custom", L"") == L"system", "a custom theme without a base is over system");
  check(custom_theme_base(L"native", L"paper") == L"system", "native cannot be a base");
  check(custom_theme_base(L"night", L"paper") == L"night", "the theme on screen becomes the base");

  // applyCandidateSkin
  check(apply_candidate_skin(skins(L"system", L"", L""), L"auto", L"system", Slot::either,
                             catalog_slot) == skins(L"system", L"auto", L"auto"),
        "a system-based skin fills both slots");
  check(apply_candidate_skin(skins(L"system", L"", L""), L"night", L"night", Slot::dark,
                             catalog_slot) == skins(L"night", L"", L"night"),
        "a dark skin fills the dark slot and sets its base");
  check(apply_candidate_skin(skins(L"system", L"night", L""), L"night2", L"ink", Slot::dark,
                             catalog_slot) == skins(L"ink", L"", L"night2"),
        "a new dark skin drops an old dark skin kept in the light slot");
  check(apply_candidate_skin(skins(L"system", L"day", L""), L"night", L"night", Slot::dark,
                             catalog_slot) == skins(L"night", L"day", L"night"),
        "a dark skin keeps a light skin in the light slot");
  check(apply_candidate_skin(skins(L"night", L"night", L""), L"day", L"light", Slot::light,
                             catalog_slot) == skins(L"light", L"day", L"night"),
        "a light skin moves a known dark skin to the empty dark slot");
  check(apply_candidate_skin(skins(L"system", L"auto", L""), L"day", L"light", Slot::light,
                             catalog_slot) == skins(L"light", L"day", L"auto"),
        "a light skin moves a known system skin to the empty dark slot");
  check(apply_candidate_skin(skins(L"system", L"gone", L""), L"day", L"light", Slot::light,
                             catalog_slot) == skins(L"light", L"day", L""),
        "an unknown package is overwritten, not moved");
  check(apply_candidate_skin(skins(L"system", L"night", L"auto"), L"day", L"light", Slot::light,
                             catalog_slot) == skins(L"light", L"day", L"auto"),
        "a filled dark slot is left alone");

  // removeCandidateSkin
  check(remove_candidate_skin(skins(L"night", L"day", L"night"), L"night") ==
            skins(L"night", L"day", L""),
        "removing one skin keeps the other slot and the base");
  check(remove_candidate_skin(skins(L"system", L"auto", L"auto"), L"auto") ==
            skins(L"system", L"", L""),
        "removing a skin from both slots empties them");
  check(remove_candidate_skin(skins(L"night", L"", L"night"), L"night") ==
            skins(L"system", L"", L""),
        "removing the last skin resets the base to system");
  check(remove_candidate_skin(skins(L"night", L"day", L""), L"other") ==
            skins(L"night", L"day", L""),
        "removing a skin not in use changes nothing");

  // candidateSkinFor / skinDrawsIn
  check(candidate_skin_for(skins(L"system", L"day", L""), true) == L"day",
        "dark mode falls back to the light slot");
  check(candidate_skin_for(skins(L"system", L"day", L"night"), true) == L"night",
        "dark mode prefers the dark slot");
  check(candidate_skin_for(skins(L"system", L"", L"night"), false).empty(),
        "light mode reads only the light slot");
  check(skin_draws_in(Slot::either, true) && skin_draws_in(Slot::either, false),
        "a system skin draws in both modes");
  check(skin_draws_in(Slot::dark, true) && !skin_draws_in(Slot::dark, false),
        "a dark skin draws only in dark mode");

  // 使用中、卡片停在哪一款、两种明暗的说明
  check(skins_in_use(false, skins(L"system", L"day", L"night")).empty(),
        "skins are in use only while the custom theme is selected");
  check(skin_in_use(true, skins(L"system", L"day", L"night"), L"night"),
        "a skin in the dark slot is in use");
  check(!skin_in_use(true, skins(L"system", L"", L""), L""), "an empty id is never in use");
  check(shown_package(true, skins(L"system", L"day", L"night"), true, catalog()) == 1u,
        "the carousel stops on the skin of the preview mode");
  check(shown_package(true, skins(L"system", L"gone", L"night"), false, catalog()) == 1u,
        "an unlisted preview skin falls through to the other slot");
  check(!shown_package(true, skins(L"system", L"gone", L""), false, catalog()),
        "no listed skin selects the custom card");
  check(!shown_package(false, skins(L"system", L"day", L""), false, catalog()),
        "no skin is shown while another theme is selected");
  check(slot_summary(true, skins(L"system", L"day", L"night"), catalog()) ==
            L"浅色模式用「白昼」，深色模式用「夜色」。",
        "the summary names both slots");
  check(slot_summary(true, skins(L"system", L"night", L""), catalog()) ==
            L"浅色模式不用皮肤，深色模式用「夜色」。",
        "a dark skin in the light slot is not drawn in light mode");
  check(slot_summary(true, skins(L"system", L"gone", L""), catalog()) ==
            L"浅色模式用「gone」，深色模式用「gone」。",
        "an unlisted package is named by its id");
  check(slot_summary(false, skins(L"system", L"day", L""), catalog()).empty(),
        "no summary while another theme is selected");

  // ExternalSkinCard
  const std::vector<std::wstring> both{L"light", L"dark"};
  const std::vector<std::wstring> light_only{L"light"};
  const std::vector<std::wstring> vertical{L"vertical"};
  check(card_dark(Slot::dark, false, false, light_only), "a dark base fixes the card's mode");
  check(!card_dark(Slot::either, false, true, both), "the card's own preview switch wins");
  check(card_dark(Slot::either, std::nullopt, true, both), "the card follows the active mode");
  check(!card_dark(Slot::either, std::nullopt, true, light_only),
        "an unsupported active mode shows the package's first mode");
  check(card_compatible(Slot::either, vertical, both, L"vertical", true),
        "a package supporting the layout and mode can be chosen");
  check(!card_compatible(Slot::either, vertical, both, L"horizontal", true),
        "an unsupported layout cannot be chosen");
  check(!card_compatible(Slot::either, vertical, light_only, L"vertical", true),
        "a system package must support the active mode");
  check(card_compatible(Slot::light, vertical, light_only, L"vertical", true),
        "a fixed package is not blocked by the active mode");
  check(card_description(true, L"", L"跟随系统", vertical, both) == L"基于 跟随系统",
        "a package without a description names its base");
  check(card_description(true, L"清爽", L"跟随系统", vertical, both) == L"清爽",
        "a package's own description is shown");
  check(card_description(false, L"清爽", L"跟随系统", vertical, both) ==
            L"当前布局或明暗模式不受支持（vertical，light/dark）",
        "an incompatible package says what it supports");
  check(card_meta(L"sample", L"1.0", L"") == L"sample · v1.0", "empty meta parts are left out");
  check(card_meta(L"sample", L"", L"作者") == L"sample · 作者", "an empty version is left out");
  check(slot_label(Slot::either) == L"跟随明暗，浅色、深色模式都能用", "the system slot label");

  // 候选颜色
  check(candidate_text_color(L"#A1b2C3") == std::wstring(L"#a1b2c3"), "a colour is read in lowercase");
  check(!candidate_text_color(L"#abc"), "a short colour is not a colour");
  check(!candidate_text_color(L"#abcdeg"), "a non-hex colour is not a colour");
  check(!candidate_text_color(L""), "an empty colour is unset");
  const auto pick = candidate_color_change(L"night", L"system", false);
  check(pick.select_custom && pick.reset_skins && pick.base == L"night",
        "picking a colour over a built-in theme keeps it as the base and drops the skins");
  const auto again = candidate_color_change(L"custom", L"paper", false);
  check(again.select_custom && !again.reset_skins,
        "picking a colour on the custom theme keeps its skins");
  const auto clear = candidate_color_change(L"night", L"system", true);
  check(!clear.select_custom && !clear.reset_skins, "following the theme changes nothing else");
  check(std::size(color_slots) == 7 && color_slots[0].key == L"surface" &&
            color_slots[6].key == L"border" && !color_slots[0].pressed && color_slots[1].pressed,
        "seven pickers from surface to border");

  if (failures)
    return 1;
  std::puts("theme skin model ok");
  return 0;
}
