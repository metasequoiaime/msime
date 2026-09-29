#pragma once

#include <cmath>
#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "../core/CandidateSkinCatalog.h"
#include "CandidatePalette.h"

namespace msime::linux_host {

struct ThemeColor {
  std::uint32_t rgb = 0;
  std::uint8_t alpha = 0xFF;
};

// The candidate palette slots the floating menus derive from (THEME_CONTRACT §3: surface for the menu, text for its items, hover for the hovered item, border for its separators and outline), kept as the theme set them, alpha included, because they are composited over the menu's own surface rather than the candidate card. A slot the theme leaves null stays empty here and the menu draws its native token; `system` leaves all of them empty. Only Fcitx5 reads them: IBus menus are drawn by the desktop shell and take no colours from an input method.
struct CandidateMenuSlots {
  std::optional<ThemeColor> surface;
  std::optional<ThemeColor> text;
  std::optional<ThemeColor> hover;
  std::optional<ThemeColor> border;
};

// The candidate colours both Linux frontends draw with, mapped from one resolved global theme in one place: IBus turns them into text attributes and Fcitx5 into a classic UI theme, so the two cannot disagree about what a theme, a package or a custom colour means.
struct CandidateColors {
  std::optional<std::uint32_t> text;
  std::optional<std::uint32_t> number;
  std::optional<std::uint32_t> accent;
  std::optional<std::uint32_t> background;
  std::optional<std::uint32_t> selected;
  std::optional<std::uint32_t> selected_text;
  std::optional<std::uint32_t> selected_number;
  // The translation line, only when a skin gives it a colour of its own (the resolved secondary differs from number); otherwise it draws in the row's text colour as before. Only IBus colours it: the classic UI draws a candidate's text in one colour.
  std::optional<std::uint32_t> translation;
  // The card's outline, already composited over the background: Fcitx5's classic UI paints the border with the SOURCE operator, so a translucent one would show the desktop through the panel rather than tint the surface the way it does on Windows. Only Fcitx5 draws it; IBus text attributes have no way to outline the panel.
  std::optional<std::uint32_t> border;
  int border_width = 0;
  CandidateMenuSlots menu;
};

inline std::optional<std::uint32_t> palette_color(const nlohmann::json &value) {
  if (!value.is_string()) return std::nullopt;
  const auto hex = value.get<std::string>();
  if (hex.size() != 7 || hex.front() != '#') return std::nullopt;
  std::uint32_t color = 0;
  for (std::size_t index = 1; index < hex.size(); ++index) {
    const auto c = static_cast<unsigned char>(hex[index]);
    std::uint32_t digit;
    if (c >= '0' && c <= '9') digit = c - '0';
    else if (c >= 'a' && c <= 'f') digit = c - 'a' + 10;
    else if (c >= 'A' && c <= 'F') digit = c - 'A' + 10;
    else return std::nullopt;
    color = (color << 4) | digit;
  }
  return color;
}

// One colour of a resolved theme: the shared layer normalizes every slot to #RRGGBB or #RRGGBBAA (alpha last) before it reaches a host, so nothing else is read here, and anything else reads as unset, which draws the native token.
inline std::optional<ThemeColor> theme_color(const nlohmann::json &value) {
  if (!value.is_string()) return std::nullopt;
  const auto text = value.get<std::string>();
  if (text.size() == 7) {
    if (const auto rgb = palette_color(value)) return ThemeColor{*rgb, 0xFF};
    return std::nullopt;
  }
  if (text.size() != 9) return std::nullopt;
  const auto rgb = palette_color(nlohmann::json(text.substr(0, 7)));
  const auto alpha = palette_color(nlohmann::json("#0000" + text.substr(7)));
  if (!rgb || !alpha) return std::nullopt;
  return ThemeColor{*rgb, static_cast<std::uint8_t>(*alpha)};
}

inline std::optional<std::uint32_t> contrasting_color(std::optional<std::uint32_t> background) {
  if (!background) return std::nullopt;
  const auto linear = [](std::uint32_t channel) {
    const double value = channel / 255.0;
    return value <= 0.04045 ? value / 12.92 : std::pow((value + 0.055) / 1.055, 2.4);
  };
  const auto luminance = 0.2126 * linear((*background >> 16) & 0xff) +
                         0.7152 * linear((*background >> 8) & 0xff) +
                         0.0722 * linear(*background & 0xff);
  const auto black_contrast = (luminance + 0.05) / 0.05;
  const auto white_contrast = 1.05 / (luminance + 0.05);
  return black_contrast >= white_contrast ? 0x000000u : 0xffffffu;
}

// Whether the candidate surfaces draw dark. An explicit candidate_theme wins; "follow" takes the global theme mode, as Windows resolves theme_cand against theme_mode and as the voice overlay does here (VoiceAction.h): the desktop appearance decides only when that mode is "system", which is also the shared default when the key is absent. The Fcitx5 mode badge takes its colours from here too, so it always matches the panel.
inline bool candidate_dark_theme(const nlohmann::json &preferences, bool system_dark) {
  const auto theme = preferences.value("candidate_theme", "follow");
  if (theme != "follow") return theme == "dark";
  const auto global = preferences.value("theme", "system");
  return global == "system" ? system_dark : global != "light";
}

// The candidate window being drawn, as msime_client_resolve_theme takes it: a package may declare only one of the two layouts.
inline std::string candidate_layout_id(const nlohmann::json &preferences) {
  return preferences.value("candidate_layout", std::string{}) == "horizontal" ? "horizontal" : "vertical";
}

// The msime_client_resolve_theme request for the candidate window: the global theme and the custom theme exactly as stored, the mode candidate_dark_theme settles on, the layout being drawn and, when the custom theme names an installed package, that package's catalogue entry unchanged. The shared layer uses the package only for `custom` and only when its id equals custom_theme.candidate_skin, so it is sent only then. The caller does the FFI call; this header stays free of it so the palette tests need no engine.
inline nlohmann::json candidate_theme_request(const nlohmann::json &preferences, bool dark,
                                              const nlohmann::json &catalog) {
  using Json = nlohmann::json;
  const auto theme = preferences.find("global_theme");
  Json request{{"global_theme", theme != preferences.end() && theme->is_string() ? *theme : Json("system")},
               {"dark", dark},
               {"layout", candidate_layout_id(preferences)}};
  const auto custom = preferences.find("custom_theme");
  if (custom == preferences.end() || !custom->is_object()) return request;
  request["custom_theme"] = *custom;
  const auto skin = custom->find("candidate_skin");
  if (request["global_theme"] != "custom" || skin == custom->end() || !skin->is_string()) return request;
  if (const auto *package = candidate_skin_package(catalog, skin->get<std::string>()))
    request["package"] = *package;
  return request;
}

// What both frontends draw for one resolved theme.
struct CandidateTheme {
  CandidateColors colors;
  // The mode the surfaces are drawn in: a theme with a fixed appearance overrides the host's.
  bool dark = false;
  // The package whose colours were drawn, the only signal for its decoration; empty when none.
  std::string candidate_skin;
};

// Map a ResolvedTheme (the `value` of msime_client_resolve_theme) onto the colours the frontends draw. Every null slot, or a null palette as `system` resolves to, takes the Adwaita token from candidate_native_palette in the mode drawn. The frontends paint opaque colours only, so translucent slots are composited: the surface over the native surface, everything else over the surface. The hover slot has no counterpart on the card (neither IBus attributes nor the classic UI theme track the pointer over candidates) and reaches only the Fcitx5 menu highlight, through `menu`; show_selected_bar is not read, because neither frontend draws a selection bar that a package could hide. An empty object resolves to the native tokens, which is also what a host draws when the call fails.
inline CandidateTheme candidate_theme_colors(const nlohmann::json &resolved, bool dark) {
  using Json = nlohmann::json;
  CandidateTheme theme;
  const auto appearance = resolved.find("appearance");
  theme.dark = appearance != resolved.end() && appearance->is_string() ? *appearance == "dark" : dark;
  const auto skin = resolved.find("candidate_skin");
  if (skin != resolved.end() && skin->is_string()) theme.candidate_skin = skin->get<std::string>();
  const auto native = candidate_native_palette(theme.dark);
  const auto candidate = resolved.find("candidate");
  const Json palette = candidate != resolved.end() && candidate->is_object() ? *candidate : Json::object();
  const auto slot = [&](const char *key) { return theme_color(palette.value(key, Json(nullptr))); };
  const auto surface_slot = slot("surface");
  const auto surface = surface_slot ? composite_color(surface_slot->rgb, surface_slot->alpha, native.surface)
                                    : native.surface;
  const auto over_surface = [&](const char *key) -> std::optional<std::uint32_t> {
    if (const auto value = slot(key)) return composite_color(value->rgb, value->alpha, surface);
    return std::nullopt;
  };
  auto &colors = theme.colors;
  colors.menu = CandidateMenuSlots{surface_slot, slot("text"), slot("hover"), slot("border")};
  colors.background = surface;
  colors.text = over_surface("text").value_or(native.text);
  colors.number = over_surface("number").value_or(native.number);
  const auto number_slot = slot("number");
  if (const auto secondary = slot("secondary");
      secondary && !(number_slot && number_slot->rgb == secondary->rgb && number_slot->alpha == secondary->alpha))
    colors.translation = over_surface("secondary");
  colors.accent = over_surface("accent").value_or(native.accent);
  // The selected row: a theme's own fill with its own foregrounds, an accent without a fill drawn solid as the design's Linux selection is, or the native solid #3584E4 with white text.
  const auto selected = over_surface("selected");
  const auto accent = over_surface("accent");
  const auto selected_text = over_surface("selected_text");
  const auto selected_number = over_surface("selected_number");
  if (selected) {
    colors.selected = selected;
    colors.selected_text = selected_text ? selected_text
                           : slot("text") ? colors.text
                                          : contrasting_color(selected);
    colors.selected_number = selected_number ? selected_number
                             : slot("number") ? colors.number
                                              : colors.selected_text;
  } else if (accent) {
    colors.selected = accent;
    colors.selected_text = selected_text ? selected_text : contrasting_color(accent);
    colors.selected_number = selected_number ? selected_number : colors.selected_text;
  } else {
    colors.selected = native.selected;
    colors.selected_text = native.selected_text;
    colors.selected_number = native.selected_number;
  }
  // The outline, composited because Fcitx5's classic UI paints the border with the SOURCE operator: a translucent one would show the desktop through the panel. A fully transparent border means none.
  if (const auto border = slot("border")) {
    if (border->alpha > 0) {
      colors.border = composite_color(border->rgb, border->alpha, surface);
      colors.border_width = 1;
    }
  } else {
    colors.border = native.border;
    colors.border_width = 1;
  }
  return theme;
}

}  // namespace msime::linux_host
