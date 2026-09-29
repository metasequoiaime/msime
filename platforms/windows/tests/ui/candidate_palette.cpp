#include "CandidatePalette.h"
#include <cmath>
#include <stdexcept>

using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate palette validation failed");
}
bool same(CandidateColor color, float r, float g, float b, float a) {
  auto near = [](float value, float expected) {
    return std::fabs(value - expected) < 0.002f;
  };
  return near(color.r, r) && near(color.g, g) && near(color.b, b) &&
         near(color.a, a);
}
int main() {
  const CandidateColor fallback = candidate_rgb(0x112233, 0.5f);
  // Hex forms the manifests use, with and without alpha.
  require(same(parse_css_color("#ff8000", fallback), 1.0f, 128 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("ff8000", fallback), 1.0f, 128 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("#f80", fallback), 1.0f, 136 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("#e9e8e89d", fallback), 233 / 255.0f,
               232 / 255.0f, 232 / 255.0f, 157 / 255.0f));
  require(same(parse_css_color("  #ff8000\t", fallback), 1.0f, 128 / 255.0f,
               0.0f, 1.0f));

  // Functional notation, including the optional alpha argument.
  require(same(parse_css_color("rgb(255, 128, 0)", fallback), 1.0f,
               128 / 255.0f, 0.0f, 1.0f));
  require(same(parse_css_color("rgba(255, 128, 0, 0.25)", fallback), 1.0f,
               128 / 255.0f, 0.0f, 0.25f));
  require(same(parse_css_color("rgb(255 128 0)", fallback), 1.0f, 128 / 255.0f,
               0.0f, 1.0f));

  // Keywords: transparent is a value, the rest defer to the caller's color.
  require(same(parse_css_color("transparent", fallback), 0.0f, 0.0f, 0.0f,
               0.0f));
  for (const char *keyword : {"auto", "none", "", "   "})
    require(parse_css_color(keyword, fallback) == fallback);

  // Anything this presenter cannot represent keeps the built-in color rather
  // than rendering an invisible or wrong candidate window.
  for (const char *unsupported :
       {"red", "#12345", "#gggggg", "hsl(10, 20%, 30%)", "rgb(255, 128)",
        "rgb(255, 128, 0) trailing",
        "rgb(255 128 0", "var(--accent)", "#"})
    require(parse_css_color(unsupported, fallback) == fallback);

  // Defaults are the Fluent dark tokens the system theme draws.
  const CandidatePalette defaults;
  require(same(defaults.surface, 0x2C / 255.0f, 0x2C / 255.0f, 0x2C / 255.0f,
               1.0f));
  require(same(defaults.text, 1.0f, 1.0f, 1.0f, 1.0f));
  require(same(defaults.border, 1.0f, 1.0f, 1.0f, 0.08f));
  require(same(defaults.hover, 1.0f, 1.0f, 1.0f, 0.07f));
  require(same(defaults.number, 1.0f, 1.0f, 1.0f, 0.72f));
  require(same(defaults.accent, 0x60 / 255.0f, 0xCD / 255.0f, 0xFF / 255.0f,
               1.0f));
  // Fluent selects with the hover fill, the accent text and the pill.
  require(defaults.selected == defaults.hover &&
          defaults.selected_text == defaults.accent &&
          defaults.selected_number == defaults.number);
  require(defaults.show_selected_bar && defaults.radius == 8.0f &&
          defaults.border_width == 1.0f && defaults.item_radius == 4.0f &&
          defaults.container_padding == 6.0f);
  require(std::fabs(defaults.shadow_alpha - 0.14f) < 0.002f);
  require(same(defaults.menu_fill, 0x2C / 255.0f, 0x2C / 255.0f,
               0x2C / 255.0f, 0.97f));

  // The light tokens, with the geometry and the shadow shared.
  const auto light_defaults = candidate_light_palette();
  require(same(light_defaults.surface, 1.0f, 1.0f, 1.0f, 1.0f));
  require(same(light_defaults.border, 0.0f, 0.0f, 0.0f, 0.08f));
  require(same(light_defaults.text, 0x1B / 255.0f, 0x1B / 255.0f,
               0x1B / 255.0f, 1.0f));
  require(same(light_defaults.number, 0x5E / 255.0f, 0x5E / 255.0f,
               0x5E / 255.0f, 1.0f));
  require(same(light_defaults.hover, 0.0f, 0.0f, 0.0f, 0.045f));
  require(same(light_defaults.accent, 0.0f, 0x5F / 255.0f, 0xB8 / 255.0f,
               1.0f));
  require(light_defaults.selected == light_defaults.hover &&
          light_defaults.selected_text == light_defaults.accent);
  require(same(light_defaults.menu_fill, 0xF9 / 255.0f, 0xF9 / 255.0f,
               0xF9 / 255.0f, 0.97f));
  require(light_defaults.radius == defaults.radius &&
          light_defaults.shadow_alpha == defaults.shadow_alpha &&
          light_defaults.show_selected_bar == defaults.show_selected_bar);
  require(candidate_native_palette(true).surface == defaults.surface &&
          candidate_native_palette(false).surface == light_defaults.surface);

  // A theme overrides only the slots it sets; the rest keep the native tokens.
  CandidatePaletteOverrides overrides;
  overrides.text = "#101010";
  overrides.show_selected_bar = false;
  const auto skinned = candidate_palette(overrides);
  require(same(skinned.text, 0x10 / 255.0f, 0x10 / 255.0f, 0x10 / 255.0f,
               1.0f));
  require(!skinned.show_selected_bar);
  require(skinned.surface == defaults.surface &&
          skinned.accent == defaults.accent);

  // Derived slots a theme leaves to the platform follow the final values: the selection fill is the hover fill, the selected text the accent and the selected numbers the secondary colour.
  CandidatePaletteOverrides derived;
  derived.accent = "#00FF00";
  derived.hover = "#11223344";
  derived.number = "#445566";
  const auto follow = candidate_palette(derived, light_defaults);
  require(same(follow.accent, 0.0f, 1.0f, 0.0f, 1.0f));
  require(follow.selected == follow.hover && follow.selected_text == follow.accent &&
          follow.selected_number == follow.number);
  require(same(follow.hover, 0x11 / 255.0f, 0x22 / 255.0f, 0x33 / 255.0f,
               0x44 / 255.0f));
  // A slot the theme sets is drawn as given, even where Fluent would derive it.
  CandidatePaletteOverrides fixed = derived;
  fixed.selected = "#0000FF";
  fixed.selected_text = "#FFFFFF";
  fixed.selected_number = "#FFFFFFCC";
  const auto explicit_slots = candidate_palette(fixed, light_defaults);
  require(same(explicit_slots.selected, 0.0f, 0.0f, 1.0f, 1.0f));
  require(same(explicit_slots.selected_text, 1.0f, 1.0f, 1.0f, 1.0f));
  require(same(explicit_slots.selected_number, 1.0f, 1.0f, 1.0f,
               0xCC / 255.0f));

  // Empty and unparsable slots keep the value being replaced.
  CandidatePaletteOverrides blank;
  blank.surface = "";
  blank.text = "definitely not a color";
  const auto unchanged = candidate_palette(blank);
  require(unchanged.surface == defaults.surface &&
          unchanged.text == defaults.text &&
          unchanged.menu_fill == defaults.menu_fill);

  // The menus take the theme's surface, text, hover and border; with no surface set they keep the native menu material.
  CandidatePaletteOverrides paper;
  paper.surface = "#F7F1E3";
  paper.text = "#2B2B2B";
  paper.hover = "#0000000F";
  paper.border = "#00000014";
  const auto papered = candidate_palette(paper, light_defaults);
  require(papered.menu_fill == papered.surface &&
          papered.menu_text == papered.text &&
          papered.menu_hover == papered.hover &&
          papered.menu_border == papered.border);
  require(skinned.menu_fill == defaults.menu_fill &&
          skinned.menu_text == skinned.text);

  // A fixed candidate uses the accent only until it becomes the selected row, where the selected text wins; a transparent selected text keeps the normal colour.
  const auto normal_text = candidate_rgb(0x123456);
  require(candidate_row_text_color(defaults, normal_text, false, false) ==
          normal_text);
  require(candidate_row_text_color(defaults, normal_text, false, true) ==
          defaults.accent);
  require(candidate_row_text_color(defaults, normal_text, true, false) ==
          defaults.selected_text);
  CandidatePaletteOverrides keep;
  keep.selected_text = "transparent";
  const auto kept = candidate_palette(keep);
  require(candidate_row_text_color(kept, normal_text, true, true) ==
          normal_text);

  // The translation is the theme's secondary colour, which the contract fixes to number: number on a plain row, selected_number on the selected one, and number again when selected_number is transparent. It never derives from the text colour.
  CandidatePaletteOverrides numbered;
  numbered.text = "#101010";
  numbered.number = "#7A7A7A";
  numbered.selected_text = "#2C7A4B";
  numbered.selected_number = "#5FBF84";
  const auto secondary = candidate_palette(numbered);
  require(candidate_row_number_color(secondary, false) == secondary.number);
  require(candidate_row_number_color(secondary, true) ==
          secondary.selected_number);
  require(!(candidate_row_number_color(secondary, false) == secondary.text));
  require(!(candidate_row_number_color(secondary, true) ==
            secondary.selected_text));
  CandidatePaletteOverrides keep_number = numbered;
  keep_number.selected_number = "transparent";
  const auto kept_number = candidate_palette(keep_number);
  require(candidate_row_number_color(kept_number, true) == kept_number.number);

  // The toolbar is the candidate palette in the toolbar's own geometry, so it follows the theme.
  for (const auto &base : {defaults, light_defaults, papered}) {
    const auto toolbar = toolbar_palette(base);
    require(toolbar.surface == base.surface && toolbar.text == base.text &&
            toolbar.hover == base.hover && toolbar.border == base.border &&
            toolbar.selected == base.selected &&
            toolbar.selected_text == base.selected_text &&
            toolbar.accent == base.accent);
    require(toolbar.radius == 8.0f && toolbar.border_width > 0.0f);
  }

  // The tray menu draws the menu slots of the same palette.
  for (const auto &base : {defaults, light_defaults, papered}) {
    const auto tray = tray_menu_palette(base);
    require(tray.surface == base.menu_fill && tray.text == base.menu_text &&
            tray.hover == base.menu_hover && tray.border == base.menu_border &&
            tray.accent == base.accent);
    require(tray.border_width == 1.0f);
    // A dimmed row stays distinct from a live one without disappearing.
    require(tray.number != tray.text && tray.number.a > 0.0f);
    // The menu stays readable and its hover visible.
    require(tray.text != tray.surface && tray.hover != tray.surface &&
            tray.surface.a > 0.0f);
    // A switch that is off is an outline in the text colour at 60%.
    const auto off = tray_toggle_off_color(tray);
    require(off.r == tray.text.r && off.g == tray.text.g &&
            off.b == tray.text.b && std::fabs(off.a - 0.6f) < 0.002f);
    // Hints and captions are the row text at 60% of its own opacity, so they stay subordinate to the label and never outshine it.
    const auto secondary = tray_menu_secondary_color(tray);
    require(secondary.r == tray.text.r && secondary.g == tray.text.g &&
            secondary.b == tray.text.b &&
            std::fabs(secondary.a - tray.text.a * 0.6f) < 0.002f &&
            secondary.a > 0.0f && secondary.a < tray.text.a);
  }
  // Light and dark are genuinely different menus.
  require(tray_menu_palette(defaults).surface !=
          tray_menu_palette(light_defaults).surface);

  // The thumb of a switch that is on reads on the accent: white on the light accent #005FB8, black on the dark #60CDFF and on a white accent.
  require(candidate_on_accent(light_defaults.accent) == candidate_rgb(0xFFFFFF));
  require(candidate_on_accent(defaults.accent) == candidate_rgb(0x000000));
  require(candidate_on_accent(candidate_rgb(0xFFFFFF)) ==
          candidate_rgb(0x000000));
  require(candidate_on_accent(candidate_rgb(0x000000)) ==
          candidate_rgb(0xFFFFFF));
}
