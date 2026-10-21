#pragma once

#include <cstdint>
#include <optional>

#include "../candidates/CandidateColors.h"
#include "ShuangpinKeymapModel.h"

namespace msime::linux_host {

// 悬浮工具栏、它的菜单和按钮提示要画的颜色，全部不透明（0xRRGGBB）。按 THEME_CONTRACT §3 从候选调色板推出，与 macOS 的 applySkin 相同：本体是候选底色，字和图标是候选文字色，悬停底是主题的悬停色（主题没给时是文字色在底色上的一层淡色，macOS 每个主题的悬停都是这样），分隔线取描边（主题不画描边时取文字色的一层淡色）。系统主题因此是 Adwaita 的白或 #303030 底，皮肤画自己的颜色。
struct FloatingToolbarColors {
  std::uint32_t surface = 0xFFFFFF;
  std::uint32_t text = 0x2E2E2E;
  std::uint32_t hover = 0xEDEDED;
  std::uint32_t divider = 0xE6E6E6;
  // 工具栏外框；主题不画描边时没有。
  std::optional<std::uint32_t> border;
  // 菜单里勾选项和按下项的强调色：选中行的底色和字色，与候选窗的选中项同一种颜色。
  std::uint32_t accent = 0x3584E4;
  std::uint32_t accent_text = 0xFFFFFF;
  bool dark = false;
  bool operator==(const FloatingToolbarColors &other) const {
    return surface == other.surface && text == other.text && hover == other.hover && divider == other.divider &&
           border == other.border && accent == other.accent && accent_text == other.accent_text && dark == other.dark;
  }
  bool operator!=(const FloatingToolbarColors &other) const { return !(*this == other); }
};

inline FloatingToolbarColors floating_toolbar_colors(const CandidateTheme &theme) {
  const auto native = candidate_native_palette(theme.dark);
  const auto &colors = theme.colors;
  FloatingToolbarColors result;
  result.dark = theme.dark;
  result.surface = colors.background.value_or(native.surface);
  result.text = colors.text.value_or(native.text);
  result.border = colors.border && colors.border_width > 0 ? colors.border : std::nullopt;
  if (colors.menu.hover && colors.menu.hover->alpha > 0)
    result.hover = composite_color(colors.menu.hover->rgb, colors.menu.hover->alpha, result.surface);
  else
    result.hover = composite_color(result.text, theme.dark ? 0x24 : 0x1A, result.surface);
  result.divider = result.border.value_or(composite_color(result.text, 0x2E, result.surface));
  result.accent = colors.selected.value_or(native.selected);
  result.accent_text = colors.selected_text.value_or(native.selected_text);
  return result;
}

// 键位图的颜色：卡片和字跟候选窗，键帽是底色上一层很淡的文字色（对应 macOS 的 controlBackgroundColor），键帽描边是 macOS separatorColor 那样的淡线，副标题、零声母和键帽上的声韵母用候选编号色（候选窗的次要文字）。高亮键与 macOS 一样填主题的强调色（`accent` 槽位）、字用白色；主题没给强调色时 candidate_theme_colors 填的是 Adwaita 写在卡片上的强调字色（native.accent，深色下是 #78AEED 这种浅蓝，不适合当底），这时用 GNOME 的强调底色 #3584E4。
inline ShuangpinKeymapColors shuangpin_keymap_colors(const CandidateTheme &theme) {
  const auto native = candidate_native_palette(theme.dark);
  const auto &colors = theme.colors;
  ShuangpinKeymapColors result;
  result.surface = colors.background.value_or(native.surface);
  result.text = colors.text.value_or(native.text);
  result.secondary = colors.number.value_or(native.number);
  result.key = composite_color(result.text, theme.dark ? 0x14 : 0x0A, result.surface);
  result.key_border = composite_color(result.text, theme.dark ? 0x26 : 0x24, result.surface);
  result.accent = colors.accent && *colors.accent != native.accent ? *colors.accent : kLinuxAccent;
  result.accent_text = 0xFFFFFF;
  result.border = colors.border && colors.border_width > 0 ? colors.border : std::nullopt;
  return result;
}

}  // namespace msime::linux_host
