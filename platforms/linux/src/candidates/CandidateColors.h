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
  // 菜单里的分隔线；只有皮肤包的 [toolbar] 配色给出（divider），候选配色没有这个槽位，没有时分隔线跟随 border。
  std::optional<ThemeColor> divider;
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

// 所画皮肤包 [toolbar] 在这个明暗下的配色，按菜单的槽位读：background 是菜单底色，icon 是菜单文字，hover 是悬停项，border 是外框，divider 是分隔线（handle 是拖动把手，菜单没有）。工具栏的功能也都在输入法菜单里，所以 Fcitx5 把它画的菜单当作工具栏的落点（X11 上另画的悬浮工具栏用候选调色板，还不读这组颜色），与 macOS 把这些颜色用到悬浮工具栏上（CandidateSkin.cpp 的 ToolbarSkinTokens）对应。共享目录只发布读得懂的 #RRGGBB / #RRGGBBAA，这里再按同样的读法取一次，读不出的槽位留空。
inline CandidateMenuSlots candidate_skin_toolbar_menu(const nlohmann::json &catalog, std::string_view drawn, bool dark) {
  CandidateMenuSlots slots;
  const auto *package = candidate_skin_package(catalog, drawn);
  if (!package) return slots;
  const auto toolbar = package->find("toolbar");
  if (toolbar == package->end() || !toolbar->is_object()) return slots;
  const auto mode = toolbar->find(dark ? "dark" : "light");
  if (mode == toolbar->end() || !mode->is_object()) return slots;
  const auto slot = [&](const char *key) { return theme_color(mode->value(key, nlohmann::json(nullptr))); };
  slots.surface = slot("background");
  slots.text = slot("icon");
  slots.hover = slot("hover");
  slots.border = slot("border");
  slots.divider = slot("divider");
  return slots;
}

// 皮肤包写了的工具栏配色盖过候选配色推出来的菜单槽位，没写的槽位照旧。
inline CandidateMenuSlots overlay_menu_slots(CandidateMenuSlots base, const CandidateMenuSlots &toolbar) {
  if (toolbar.surface) base.surface = toolbar.surface;
  if (toolbar.text) base.text = toolbar.text;
  if (toolbar.hover) base.hover = toolbar.hover;
  if (toolbar.border) base.border = toolbar.border;
  if (toolbar.divider) base.divider = toolbar.divider;
  return base;
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

// 候选窗是否画深色。明确的 candidate_theme 优先；"follow" 取全局颜色模式，与 Windows 按 theme_mode 解析 theme_cand、这里的语音浮层（VoiceAction.h）一致：只有颜色模式是 "system"（键缺省时也是这个共享默认值）时才看桌面明暗。Fcitx5 模式角标和语音浮层不看 candidate_theme，按 surface_dark_theme 跟随颜色模式。
// 运行时的 preferences 可能是 null、数组或数字（runtime-options.json 被手工改过、或写入被截断）：按一份空偏好文档处理，即跟随系统明暗，而不是让 nlohmann 的 value() 抛出 type_error。两个宿主都从这里取模式，所以这个入口是唯一需要设防的地方。
inline bool candidate_dark_theme(const nlohmann::json &preferences, bool system_dark) {
  if (!preferences.is_object()) return system_dark;
  const auto theme = preferences.find("candidate_theme");
  if (theme != preferences.end() && theme->is_string() && *theme != "follow") return *theme == "dark";
  const auto global = preferences.find("theme");
  return global == preferences.end() || !global->is_string() || *global == "system"
             ? system_dark : *global != "light";
}

// 候选窗以外的浮动界面（Fcitx5 模式角标）是否画深色：各界面一律跟随颜色模式（HostCapabilities::surface_themes_follow），只看全局的 `theme`，它的 "system"（键缺省时也是这个共享默认值）跟随桌面明暗；存下的 toolbar_theme、voice_theme 不再读。解析出的主题若有固定明暗仍会盖过它，与候选窗相同。
inline bool surface_dark_theme(const nlohmann::json &preferences, bool system_dark) {
  // 与 candidate_dark_theme 同一份文档、同一条边界：非对象按空文档处理。
  if (!preferences.is_object()) return system_dark;
  const auto global = preferences.find("theme");
  return global == preferences.end() || !global->is_string() || *global == "system"
             ? system_dark : *global != "light";
}

// 传给 `msime_client_resolve_theme` 的候选布局；皮肤可能只支持一种布局，非对象文档取默认纵向，不抛异常。
inline std::string candidate_layout_id(const nlohmann::json &preferences) {
  if (!preferences.is_object()) return "vertical";
  const auto layout = preferences.find("candidate_layout");
  return layout != preferences.end() && layout->is_string() && *layout == "horizontal"
             ? "horizontal" : "vertical";
}

// 菜单和设置页写入的 `global_theme`、整个 `custom_theme` 构成选择指纹，覆盖皮肤与取色器变化；重读不算新选择，缺失主题键与 `system` 等价。
inline nlohmann::json candidate_theme_selection(const nlohmann::json &preferences) {
  using Json = nlohmann::json;
  if (!preferences.is_object()) return Json::object();
  const auto theme = preferences.find("global_theme");
  const auto custom = preferences.find("custom_theme");
  return Json{{"global_theme", theme != preferences.end() && theme->is_string() ? *theme : Json("system")},
              {"custom_theme", custom != preferences.end() ? *custom : Json(nullptr)}};
}

// 候选窗的 msime_client_resolve_theme 请求：原样存储的全局主题、去掉编号色和边框色的自定义主题、要绘制的明暗 `dark`（候选窗是 candidate_dark_theme 定下的那个，语音浮层和模式角标各按自己的规则）、正在绘制的布局，以及自定义主题在这个明暗下的槽位指名已安装皮肤包时，该包在目录里的原样条目。槽位按 candidate_skin_for 取：深色取 `candidate_skin_dark`，没设时取 `candidate_skin`；浅色取 `candidate_skin`。共享层只在 `custom` 下、且包 id 正是当前模式槽位指名的那个时才用它，所以只在这时发送。FFI 调用由调用方做，这个头文件不碰它，调色板测试因此不需要引擎。
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
  // 编号色和边框色跟随主题和皮肤（HostCapabilities 的 candidate_number_color、candidate_border_color 在 Linux 为假）：存下的两个槽位不交给解析器，编号色由文字颜色推出，边框取主题的发丝线。不改文档，选择指纹（candidate_theme_selection）照旧看整个 custom_theme。
  auto &drawn = request["custom_theme"];
  const auto colors = drawn.find("candidate_colors");
  if (colors != drawn.end() && colors->is_object()) {
    colors->erase("number");
    colors->erase("border");
  }
  const auto skin = candidate_skin_for(*custom, dark);
  if (request["global_theme"] != "custom" || skin.empty()) return request;
  if (const auto *package = candidate_skin_package(catalog, skin))
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
  // 共享层是否解析出自己的候选覆盖，见 `candidate_theme_covers`。
  bool covers_candidates = false;
};

// 有效 `system` 用原生配色绘制水杉样式，候选配色或有效皮肤也提供覆盖；无颜色自定义与解析失败仍不算。
inline bool candidate_theme_covers(const nlohmann::json &resolved) {
  if (const auto id = resolved.find("id"); id != resolved.end() && *id == "system") return true;
  if (const auto candidate = resolved.find("candidate");
      candidate != resolved.end() && candidate->is_object())
    return true;
  const auto skin = resolved.find("candidate_skin");
  return skin != resolved.end() && skin->is_string() && !skin->get<std::string>().empty();
}

// 将共享 ResolvedTheme 映射为 IBus 属性和 classicui 静态主题使用的不透明颜色；空槽位取当前明暗的 Adwaita token。
// 这两条桌面绘制路径不消费独立候选 hover 或选中条；X11 原生画布另行读取共享调色板与 candidate_details。
inline CandidateTheme candidate_theme_colors(const nlohmann::json &resolved, bool dark) {
  using Json = nlohmann::json;
  CandidateTheme theme;
  // 在下面的空槽位被原生 token 补齐之前判断覆盖，否则补齐后的颜色无法区分「主题给的颜色」与「平台默认」。
  theme.covers_candidates = candidate_theme_covers(resolved);
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
  colors.menu = CandidateMenuSlots{surface_slot, slot("text"), slot("hover"), slot("border"), std::nullopt};
  colors.background = surface;
  colors.text = over_surface("text").value_or(native.text);
  colors.number = over_surface("number").value_or(native.number);
  const auto number_slot = slot("number");
  if (const auto secondary = slot("secondary");
      secondary && !(number_slot && number_slot->rgb == secondary->rgb && number_slot->alpha == secondary->alpha))
    colors.translation = over_surface("secondary");
  colors.accent = over_surface("accent").value_or(native.accent);
  // 选中行：主题自己的选中底配它自己的前景色；只给了强调色时像设计稿的 Linux 选中行那样把强调色画成实心底；都没给时是原生的实心 #1A5FB4 配白字（`kLinuxSelection`）。
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

// A floating surface's palette from a resolved theme, already composited opaque by candidate_theme_colors: the card surface, its text, its accent text colour and its outline (none when the theme draws no border). Every slot is set there, so the native tokens are only the guard for a theme built by hand.
inline FloatingSurfaceColors floating_surface_colors(const CandidateTheme &theme) {
  const auto native = candidate_native_palette(theme.dark);
  const auto &colors = theme.colors;
  return FloatingSurfaceColors{
      colors.background.value_or(native.surface),
      colors.text.value_or(native.text),
      colors.accent.value_or(native.accent),
      colors.border && colors.border_width > 0 ? colors.border : std::nullopt,
  };
}

}  // namespace msime::linux_host
