#pragma once
#include <cctype>
#include <cmath>
#include <cstdint>
#include <optional>
#include <sstream>
#include <string>

namespace msime::windows {
// Candidate colors ported from the shipped presenter. Skin manifests carry CSS
// strings because the settings page renders them in a WebView, so a native
// presenter parses the same subset and keeps its own value for anything it
// cannot represent. Channels are straight alpha in [0,1]; the renderer
// converts to its own color type, leaving this header free of Direct2D.
struct CandidateColor {
  float r = 0.0f, g = 0.0f, b = 0.0f, a = 1.0f;
  friend bool operator==(const CandidateColor &left,
                         const CandidateColor &right) {
    return left.r == right.r && left.g == right.g && left.b == right.b &&
           left.a == right.a;
  }
  friend bool operator!=(const CandidateColor &left,
                         const CandidateColor &right) {
    return !(left == right);
  }
};
inline constexpr CandidateColor candidate_rgb(uint32_t rgb,
                                              float alpha = 1.0f) {
  return {((rgb >> 16) & 0xFF) / 255.0f, ((rgb >> 8) & 0xFF) / 255.0f,
          (rgb & 0xFF) / 255.0f, alpha};
}
inline CandidateColor parse_css_color(const std::string &text,
                                      CandidateColor fallback) {
  auto space = [](unsigned char ch) { return std::isspace(ch) != 0; };
  size_t begin = 0, end = text.size();
  while (begin < end && space(static_cast<unsigned char>(text[begin])))
    ++begin;
  while (end > begin && space(static_cast<unsigned char>(text[end - 1])))
    --end;
  std::string value = text.substr(begin, end - begin);
  if (value.empty() || value == "auto" || value == "none")
    return fallback;
  // Fully transparent is a deliberate choice, not a missing value.
  if (value == "transparent")
    return {0.0f, 0.0f, 0.0f, 0.0f};
  if (value.rfind("rgba(", 0) == 0 || value.rfind("rgb(", 0) == 0) {
    const auto open = value.find('(');
    const auto close = value.rfind(')');
    if (open == std::string::npos || close == std::string::npos ||
        close <= open || close + 1 != value.size())
      return fallback;
    std::string inner = value.substr(open + 1, close - open - 1);
    for (char &ch : inner)
      if (ch == ',')
        ch = ' ';
    std::istringstream stream(inner);
    float r = 0.0f, g = 0.0f, b = 0.0f, a = 1.0f;
    if (!(stream >> r >> g >> b))
      return fallback;
    stream >> a;
    std::string trailing;
    if (stream >> trailing)
      return fallback;
    return {r / 255.0f, g / 255.0f, b / 255.0f, a};
  }
  if (!value.empty() && value.front() == '#')
    value.erase(value.begin());
  for (char ch : value)
    if (!std::isxdigit(static_cast<unsigned char>(ch)))
      return fallback;
  auto channel = [&value](size_t index, size_t width) {
    const auto digits = width == 1 ? std::string(2, value[index])
                                   : value.substr(index, 2);
    return static_cast<uint32_t>(std::stoul(digits, nullptr, 16));
  };
  if (value.size() == 3)
    return {channel(0, 1) / 255.0f, channel(1, 1) / 255.0f,
            channel(2, 1) / 255.0f, 1.0f};
  if (value.size() == 6)
    return {channel(0, 2) / 255.0f, channel(2, 2) / 255.0f,
            channel(4, 2) / 255.0f, 1.0f};
  if (value.size() == 8)
    return {channel(0, 2) / 255.0f, channel(2, 2) / 255.0f,
            channel(4, 2) / 255.0f, channel(6, 2) / 255.0f};
  return fallback;
}
// The candidate slots of a resolved global theme (msime_client_resolve_theme). An absent slot is one the theme leaves to the platform, and keeps the native token; an unparsable one is treated the same way.
struct CandidatePaletteOverrides {
  std::optional<std::string> accent, selected, hover, surface, border, text,
      number, selected_text, selected_number, secondary;
  std::optional<bool> show_selected_bar;
};
// The card, flyout, toolbar and tray colours. The defaults are the Windows 11 (Fluent) dark tokens the `system` theme draws; candidate_light_palette() holds the light ones.
struct CandidatePalette {
  CandidateColor surface = candidate_rgb(0x2C2C2C);
  CandidateColor border = {1.0f, 1.0f, 1.0f, 0.08f};
  CandidateColor text = candidate_rgb(0xFFFFFF);
  CandidateColor number = {1.0f, 1.0f, 1.0f, 0.72f};
  // Fluent selects a row with the hover fill and marks it with the accent text and the 3px pill, so the two fills are the same token.
  CandidateColor selected = {1.0f, 1.0f, 1.0f, 0.07f};
  CandidateColor hover = {1.0f, 1.0f, 1.0f, 0.07f};
  CandidateColor accent = candidate_rgb(0x60CDFF);
  // Row colours while the row is the selected one. Alpha 0 is the sentinel for "keep the unselected colour", which a theme can still ask for with a transparent slot.
  CandidateColor selected_text = candidate_rgb(0x60CDFF);
  CandidateColor selected_number = {1.0f, 1.0f, 1.0f, 0.72f};
  // The candidate right-click flyout. Natively this is the Fluent menu material; a theme that sets the surface draws its menus in it too.
  CandidateColor menu_fill = candidate_rgb(0x2C2C2C, 0.97f);
  CandidateColor menu_border = {1.0f, 1.0f, 1.0f, 0.08f};
  CandidateColor menu_text = candidate_rgb(0xFFFFFF);
  CandidateColor menu_hover = {1.0f, 1.0f, 1.0f, 0.07f};
  // The translation line, when the theme's secondary colour is not simply the numbers (an external package's `translation`); none draws it like the numbers.
  std::optional<CandidateColor> translation;
  // The floating toolbar's separator; none draws it in the border colour.
  std::optional<CandidateColor> divider;
  // One Fluent flyout shadow, 0 8px 16px rgba(0,0,0,.14), in both modes and for every theme.
  float shadow_alpha = 0.14f;
  float radius = 8.0f;
  float border_width = 1.0f;
  float container_padding = 6.0f;
  float item_radius = 4.0f;
  bool show_selected_bar = true;
};
// Resolve the candidate label as one row. The selected text wins for every highlighted entry; the fixed-position accent is only an unselected-row treatment.
inline CandidateColor candidate_row_text_color(const CandidatePalette &palette,
                                               CandidateColor normal,
                                               bool highlighted,
                                               bool fixed_position) {
  if (highlighted)
    return palette.selected_text.a > 0.0f ? palette.selected_text : normal;
  return fixed_position ? palette.accent : normal;
}
// Resolve the row's index number colour: number, or selected_number on the selected row; alpha 0 in selected_number keeps number.
inline CandidateColor candidate_row_number_color(const CandidatePalette &palette,
                                                 bool highlighted) {
  return highlighted && palette.selected_number.a > 0.0f
             ? palette.selected_number
             : palette.number;
}
// Resolve the translation line. The theme's `secondary` follows `number` unless a package sets its own translation colour, which is then drawn on every row; otherwise the translation draws like the numbers.
inline CandidateColor candidate_row_translation_color(const CandidatePalette &palette,
                                                      bool highlighted) {
  return palette.translation ? *palette.translation
                             : candidate_row_number_color(palette, highlighted);
}
// The Fluent light tokens; geometry and the shadow are shared with the dark defaults.
inline CandidatePalette candidate_light_palette() {
  CandidatePalette palette;
  palette.surface = candidate_rgb(0xFFFFFF);
  palette.border = {0.0f, 0.0f, 0.0f, 0.08f};
  palette.text = candidate_rgb(0x1B1B1B);
  palette.number = candidate_rgb(0x5E5E5E);
  palette.selected = {0.0f, 0.0f, 0.0f, 0.045f};
  palette.hover = {0.0f, 0.0f, 0.0f, 0.045f};
  palette.accent = candidate_rgb(0x005FB8);
  palette.selected_text = candidate_rgb(0x005FB8);
  palette.selected_number = candidate_rgb(0x5E5E5E);
  palette.menu_fill = candidate_rgb(0xF9F9F9, 0.97f);
  palette.menu_border = {0.0f, 0.0f, 0.0f, 0.08f};
  palette.menu_text = candidate_rgb(0x1B1B1B);
  palette.menu_hover = {0.0f, 0.0f, 0.0f, 0.045f};
  return palette;
}
// The native tokens the `system` theme draws, and every other theme draws beneath the slots it sets.
inline CandidatePalette candidate_native_palette(bool dark) {
  return dark ? CandidatePalette{} : candidate_light_palette();
}
// Apply a resolved theme's candidate slots over a palette. A slot the theme sets is drawn as given. A derived slot it leaves to the platform follows the Fluent rule from the final values: the selection fill is the hover fill, the selected text is the accent and the selected numbers are the secondary colour. The menus take the card's surface, text, hover and border, so a theme colours them too; with no surface set they keep the native menu material.
inline CandidatePalette
candidate_palette(const CandidatePaletteOverrides &overrides,
                  CandidatePalette palette = {}) {
  auto set = [](const std::optional<std::string> &value) {
    return value && !value->empty();
  };
  auto apply = [&set](const std::optional<std::string> &value,
                      CandidateColor &target) {
    if (set(value))
      target = parse_css_color(*value, target);
  };
  apply(overrides.accent, palette.accent);
  apply(overrides.hover, palette.hover);
  apply(overrides.surface, palette.surface);
  apply(overrides.border, palette.border);
  apply(overrides.text, palette.text);
  apply(overrides.number, palette.number);
  if (set(overrides.selected))
    apply(overrides.selected, palette.selected);
  else
    palette.selected = palette.hover;
  if (set(overrides.selected_text))
    apply(overrides.selected_text, palette.selected_text);
  else
    palette.selected_text = palette.accent;
  if (set(overrides.selected_number))
    apply(overrides.selected_number, palette.selected_number);
  else
    palette.selected_number = palette.number;
  // The contract's secondary equals number unless a package gave a translation colour.
  if (set(overrides.secondary) && overrides.secondary != overrides.number)
    palette.translation = parse_css_color(*overrides.secondary, palette.number);
  if (set(overrides.surface))
    palette.menu_fill = palette.surface;
  palette.menu_text = palette.text;
  palette.menu_hover = palette.hover;
  palette.menu_border = palette.border;
  if (overrides.show_selected_bar)
    palette.show_selected_bar = *overrides.show_selected_bar;
  return palette;
}
// The floating toolbar derives from the candidate palette (surface, text, hover, border, and the selected fill with the selected text for a pressed button), so it follows the global theme. Its geometry stays the toolbar's own.
inline CandidatePalette toolbar_palette(CandidatePalette palette) {
  palette.radius = 8.0f;
  palette.border_width = 1.4f;
  palette.item_radius = 6.5f;
  return palette;
}
// The tray menu is drawn like the candidate flyout: the menu slots of the candidate palette, the accent for a switch that is on and the secondary colour for a row whose capability is missing.
inline CandidatePalette tray_menu_palette(CandidatePalette palette) {
  palette.surface = palette.menu_fill;
  palette.border = palette.menu_border;
  palette.text = palette.menu_text;
  palette.hover = palette.menu_hover;
  palette.border_width = 1.0f;
  return palette;
}
// A Fluent switch that is off is an outline, not a fill: the ring and the thumb are the text colour at reduced opacity, so they read on any theme's surface.
inline CandidateColor tray_toggle_off_color(const CandidatePalette &palette) {
  return {palette.text.r, palette.text.g, palette.text.b, 0.6f};
}
// Secondary text of the tray menu: shortcut hints, group captions and tool captions, drawn as the design's 60% of the row text rather than a colour of their own, so they follow any theme.
inline CandidateColor tray_menu_secondary_color(const CandidatePalette &palette) {
  return {palette.text.r, palette.text.g, palette.text.b, palette.text.a * 0.6f};
}
// The thumb of a switch that is on, drawn over the accent: black over a light accent and white over a dark one, with the same luminance threshold as the shared layer's readable_text, so an accent such as ink's white keeps a visible thumb.
inline CandidateColor candidate_on_accent(const CandidateColor &accent) {
  auto linear = [](float value) {
    return value <= 0.04045f ? value / 12.92f
                             : std::pow((value + 0.055f) / 1.055f, 2.4f);
  };
  const float luminance = 0.2126f * linear(accent.r) +
                          0.7152f * linear(accent.g) +
                          0.0722f * linear(accent.b);
  return luminance > 0.179f ? candidate_rgb(0x000000) : candidate_rgb(0xFFFFFF);
}
} // namespace msime::windows
