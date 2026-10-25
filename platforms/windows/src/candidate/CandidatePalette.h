#pragma once
#include <algorithm>
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
// The colour a CSS string names, or none when it names no colour this parser can draw (empty, `auto`, `none`, a malformed rgb()/rgba() or an unsupported hex length). Hex takes #RGB, #RGBA, #RRGGBB and #RRGGBBAA, as msime-windows' ParseCandidateCssColor does.
inline std::optional<CandidateColor> try_parse_css_color(const std::string &text) {
  auto space = [](unsigned char ch) { return std::isspace(ch) != 0; };
  size_t begin = 0, end = text.size();
  while (begin < end && space(static_cast<unsigned char>(text[begin])))
    ++begin;
  while (end > begin && space(static_cast<unsigned char>(text[end - 1])))
    --end;
  std::string value = text.substr(begin, end - begin);
  if (value.empty() || value == "auto" || value == "none")
    return std::nullopt;
  // Fully transparent is a deliberate choice, not a missing value.
  if (value == "transparent")
    return CandidateColor{0.0f, 0.0f, 0.0f, 0.0f};
  if (value.rfind("rgba(", 0) == 0 || value.rfind("rgb(", 0) == 0) {
    const auto open = value.find('(');
    const auto close = value.rfind(')');
    if (open == std::string::npos || close == std::string::npos ||
        close <= open || close + 1 != value.size())
      return std::nullopt;
    std::string inner = value.substr(open + 1, close - open - 1);
    for (char &ch : inner)
      if (ch == ',')
        ch = ' ';
    std::istringstream stream(inner);
    float r = 0.0f, g = 0.0f, b = 0.0f, a = 1.0f;
    if (!(stream >> r >> g >> b))
      return std::nullopt;
    stream >> std::ws;
    if (!stream.eof()) {
      if (!(stream >> a))
        return std::nullopt;
      stream >> std::ws;
    }
    if (!stream.eof() || !std::isfinite(r) || !std::isfinite(g) ||
        !std::isfinite(b) || !std::isfinite(a) || r < 0.0f || r > 255.0f ||
        g < 0.0f || g > 255.0f || b < 0.0f || b > 255.0f || a < 0.0f ||
        a > 1.0f)
      return std::nullopt;
    return CandidateColor{r / 255.0f, g / 255.0f, b / 255.0f, a};
  }
  if (!value.empty() && value.front() == '#')
    value.erase(value.begin());
  for (char ch : value)
    if (!std::isxdigit(static_cast<unsigned char>(ch)))
      return std::nullopt;
  auto channel = [&value](size_t index, size_t width) {
    const auto digits = width == 1 ? std::string(2, value[index])
                                   : value.substr(index, 2);
    return static_cast<uint32_t>(std::stoul(digits, nullptr, 16));
  };
  if (value.size() == 3 || value.size() == 4)
    return CandidateColor{channel(0, 1) / 255.0f, channel(1, 1) / 255.0f,
                          channel(2, 1) / 255.0f,
                          value.size() == 4 ? channel(3, 1) / 255.0f : 1.0f};
  if (value.size() == 6)
    return CandidateColor{channel(0, 2) / 255.0f, channel(2, 2) / 255.0f,
                          channel(4, 2) / 255.0f, 1.0f};
  if (value.size() == 8)
    return CandidateColor{channel(0, 2) / 255.0f, channel(2, 2) / 255.0f,
                          channel(4, 2) / 255.0f, channel(6, 2) / 255.0f};
  return std::nullopt;
}
inline CandidateColor parse_css_color(const std::string &text,
                                      CandidateColor fallback) {
  return try_parse_css_color(text).value_or(fallback);
}
// The candidate slots of a resolved global theme (msime_client_resolve_theme). An absent slot is one the theme leaves to the platform, and keeps the native token; an unparsable one is treated the same way.
struct CandidatePaletteOverrides {
  std::optional<std::string> accent, selected, hover, surface, border, text,
      number, selected_text, selected_number, secondary;
  std::optional<bool> show_selected_bar;
};
// 候选窗、工具栏与托盘采用 macOS 的默认颜色；主题和皮肤仍逐槽覆盖。
struct CandidatePalette {
  CandidateColor surface = candidate_rgb(0x28282A, 0.97f);
  CandidateColor border = {1.0f, 1.0f, 1.0f, 0.12f};
  CandidateColor text = candidate_rgb(0xF5F5F7);
  CandidateColor number = candidate_rgb(0x98989D);
  // 与 macOS 一致，实心强调色承载白色选中文字，悬停保持独立的淡色。
  CandidateColor selected = candidate_rgb(0x5FBF84);
  CandidateColor hover = {1.0f, 1.0f, 1.0f, 0.10f};
  CandidateColor accent = candidate_rgb(0x5FBF84);
  // Row colours while the row is the selected one. Alpha 0 is the sentinel for "keep the unselected colour", which a theme can still ask for with a transparent slot.
  CandidateColor selected_text = candidate_rgb(0xFFFFFF);
  CandidateColor selected_number = candidate_rgb(0xFFFFFF, 0.82f);
  // 右键菜单沿用卡片表面；主题设置表面时同步覆盖。
  CandidateColor menu_fill = candidate_rgb(0x28282A, 0.97f);
  CandidateColor menu_border = {1.0f, 1.0f, 1.0f, 0.12f};
  CandidateColor menu_text = candidate_rgb(0xFFFFFF);
  CandidateColor menu_hover = {1.0f, 1.0f, 1.0f, 0.10f};
  // 皮肤可分别覆盖候选文字、预编辑、选中释义和选中竖条；没有声明时沿用上面的派生颜色。
  // Candidate row labels and annotations (a package's `candidate_text`); none draws them in `text`, which the pager, toolbar, keymap, tray, HUD and voice overlay keep using.
  std::optional<CandidateColor> candidate_text;
  std::optional<CandidateColor> preedit_text;
  std::optional<CandidateColor> preedit_caret;
  std::optional<CandidateColor> selected_translation;
  std::optional<CandidateColor> selected_bar;
  // The reading's rounded band and the 1 DIP rule under the preedit row.
  std::optional<CandidateColor> preedit_background;
  std::optional<CandidateColor> preedit_divider;
  // 三角翻页箭头横排时前面那条竖线（皮肤 `page_arrows` 加 `page_arrow_style = "triangle"`），和 msime-windows 一样只在那里画；首行的「n / m」翻页不用它。
  std::optional<CandidateColor> page_arrow_divider;
  // The translation line, when the theme's secondary colour is not simply the numbers (an external package's `translation`); none draws it like the numbers.
  std::optional<CandidateColor> translation;
  // The floating toolbar's separator; none draws it in the border colour.
  std::optional<CandidateColor> divider;
  // 皮肤 `backdrop = "acrylic" | "mica"` 的材质参数（`backdrop_opacity`、`backdrop_luminosity`，0-1），按明暗各写各的；没写时按 CandidateBackdrop.h 的缺省。
  std::optional<float> backdrop_opacity;
  std::optional<float> backdrop_luminosity;
  // 菜单与输入浮层保留柔和阴影。
  float shadow_alpha = 0.14f;
  float radius = 10.0f;
  float border_width = 1.0f;
  float container_padding = 6.0f;
  float item_radius = 6.0f;
  bool show_selected_bar = false;
};
// 固定排位候选未高亮时的正文色：和 macOS 候选窗一样是固定的 #379AD3（sRGB 55,154,211），不跟主题的强调色走。
inline constexpr uint32_t candidate_fixed_position_rgb = 0x379AD3;
// Resolve the candidate label as one row. The selected text wins for every highlighted entry; the fixed-position colour is only an unselected-row treatment.
inline CandidateColor candidate_row_text_color(const CandidatePalette &palette,
                                               CandidateColor normal,
                                               bool highlighted,
                                               bool fixed_position) {
  if (highlighted)
    return palette.selected_text.a > 0.0f ? palette.selected_text : normal;
  return fixed_position ? candidate_rgb(candidate_fixed_position_rgb) : normal;
}
// Resolve the row's index number colour: number, or selected_number on the selected row; alpha 0 in selected_number keeps number.
inline CandidateColor candidate_row_number_color(const CandidatePalette &palette,
                                                 bool highlighted) {
  return highlighted && palette.selected_number.a > 0.0f
             ? palette.selected_number
             : palette.number;
}
// Resolve the translation line. The theme's `secondary` follows `number` unless a package sets its own translation colour, which is then drawn on every row; otherwise the translation draws like the numbers. 没有皮肤释义色时，未高亮的固定排位候选的释义是它正文色的 0.62 倍透明度，和 macOS 一样（释义是候选文字的一部分，跟着它的固定排位色）。
inline CandidateColor candidate_row_translation_color(const CandidatePalette &palette,
                                                      bool highlighted,
                                                      bool fixed_position = false) {
  if (highlighted && palette.selected_translation)
    return *palette.selected_translation;
  if (palette.translation)
    return *palette.translation;
  if (fixed_position && !highlighted)
    return candidate_rgb(candidate_fixed_position_rgb, 0.62f);
  return candidate_row_number_color(palette, highlighted);
}
// 浅色模式与 macOS 使用相同的候选配色，几何与深色模式相同。
inline CandidatePalette candidate_light_palette() {
  CandidatePalette palette;
  palette.surface = candidate_rgb(0xFFFFFF, 0.97f);
  palette.border = {0.0f, 0.0f, 0.0f, 0.12f};
  palette.text = candidate_rgb(0x1D1D1F);
  palette.number = candidate_rgb(0x6E6E73);
  palette.selected = candidate_rgb(0x2C7A4B);
  palette.hover = {0.0f, 0.0f, 0.0f, 0.06f};
  palette.accent = candidate_rgb(0x2C7A4B);
  palette.menu_fill = palette.surface;
  palette.menu_border = palette.border;
  palette.menu_text = palette.text;
  palette.menu_hover = {0.0f, 0.0f, 0.0f, 0.06f};
  return palette;
}
// The native tokens the `system` theme draws, and every other theme draws beneath the slots it sets.
inline CandidatePalette candidate_native_palette(bool dark) {
  return dark ? CandidatePalette{} : candidate_light_palette();
}
// 主题逐槽覆盖；只改强调色时选中底跟随强调色。只有选中底变化才推导前景色，
// 避免空主题把原生深色模式的白字改掉。半透明选中底沿用普通行前景色。
inline CandidateColor candidate_selected_foreground(CandidateColor fill,
                                                   CandidateColor fallback) {
  if (fill.a < 0.85f)
    return fallback;
  auto linear = [](float value) {
    return value <= 0.04045f ? value / 12.92f
                           : std::pow((value + 0.055f) / 1.055f, 2.4f);
  };
  const float luminance = .2126f * linear(fill.r) + .7152f * linear(fill.g) +
                          .0722f * linear(fill.b);
  // 比较两个候选前景的对比度；中等亮度的橙色等背景不能靠偏高的阈值选白字。
  const float dark_luminance = linear(0x1A / 255.0f);
  const float white_contrast = 1.05f / (luminance + .05f);
  const float dark_contrast =
      ((std::max)(luminance, dark_luminance) + .05f) /
      ((std::min)(luminance, dark_luminance) + .05f);
  return candidate_rgb(white_contrast >= dark_contrast ? 0xFFFFFF : 0x1A1A1A);
}
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
  const auto original_selected = palette.selected;
  apply(overrides.accent, palette.accent);
  apply(overrides.hover, palette.hover);
  apply(overrides.surface, palette.surface);
  apply(overrides.border, palette.border);
  apply(overrides.text, palette.text);
  apply(overrides.number, palette.number);
  if (set(overrides.selected))
    apply(overrides.selected, palette.selected);
  else if (set(overrides.accent))
    palette.selected = palette.accent;
  const bool selection_changed = palette.selected != original_selected;
  if (set(overrides.selected_text))
    apply(overrides.selected_text, palette.selected_text);
  else if (selection_changed)
    palette.selected_text =
        candidate_selected_foreground(palette.selected, palette.text);
  if (set(overrides.selected_number))
    apply(overrides.selected_number, palette.selected_number);
  else if (selection_changed) {
    palette.selected_number =
        candidate_selected_foreground(palette.selected, palette.number);
    if (palette.selected.a >= 0.85f)
      palette.selected_number.a = 0.82f;
  }
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
// The floating toolbar derives from the candidate palette (surface, text, hover, border; a pressed button is drawn with the hover fill, as on macOS), so it follows the global theme. Its geometry is the toolbar's own, macOS's: a 10 DIP card radius (NativeCandidateTokens' radius, which macOS uses whatever the theme's card radius is, until a package's `[toolbar]` radius replaces it) and a 1 DIP hairline. The hover fill's corner follows the button size (toolbar_button_radius in ToolbarLayout.h), so item_radius is not set here and stays the candidate default; the image toolbar does not use it either, its own hover/pressed pill has a fixed corner (kImagePillRadius in FloatingToolbarWindow.cpp). The hover-for-pressed rule is the normal bar's only: an image toolbar still draws a pressed button with the selected fill and selected text colour.
inline CandidatePalette toolbar_palette(CandidatePalette palette) {
  palette.radius = 10.0f;
  palette.border_width = 1.0f;
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
