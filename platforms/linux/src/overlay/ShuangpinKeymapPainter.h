#pragma once

#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <string>

#include "OverlayPaint.h"
#include "ShuangpinKeymapModel.h"

namespace msime::linux_host {

// 一帧键位图的内容：标题、三排键帽、零声母说明和要高亮的键。键帽内容随方案变，高亮随每个键变。
struct ShuangpinKeymapContent {
  std::string title;
  ShuangpinKeymapRows rows;
  std::string zero_initials;
  char highlighted = 0;
};

// 表面的逻辑尺寸：卡片加四周的阴影边（shadow 为 false 时没有）。
inline double shuangpin_keymap_margin(bool shadow) { return shadow ? ShuangpinKeymapGeometry::shadow_margin : 0.0; }
inline double shuangpin_keymap_surface_width(bool shadow) {
  return ShuangpinKeymapGeometry::width + 2.0 * shuangpin_keymap_margin(shadow);
}
inline double shuangpin_keymap_surface_height(bool shadow) {
  return ShuangpinKeymapGeometry::height + 2.0 * shuangpin_keymap_margin(shadow);
}

// 把排好的一段字画在 (x, y)，即它逻辑矩形的左上角。
inline void shuangpin_keymap_show_text(cairo_t *cairo, PangoLayout *layout, double x, double y) {
  cairo_move_to(cairo, x, y);
  pango_cairo_show_layout(cairo, layout);
  // show_layout 不清掉 move_to 留下的当前点，接着画的路径会从这里拉出一条线。
  cairo_new_path(cairo);
}

// 在逻辑像素里画整块键位图（调用方已把上下文缩放到设备像素并清成透明），照 macOS 的 ShuangpinKeymapPanel 排：标题行左边是方案名，右边是说明；三排键帽，键名在左上角用等宽粗体，声韵母在键帽下沿居中；最下面一行零声母。高亮的键填选中色，键名和声韵母换成选中项的字色。
inline void paint_shuangpin_keymap(cairo_t *cairo, const ShuangpinKeymapContent &content,
                                   const ShuangpinKeymapColors &colors, bool shadow) {
  using G = ShuangpinKeymapGeometry;
  const double origin = shuangpin_keymap_margin(shadow);
  if (shadow) overlay_paint_shadow(cairo, origin, origin, G::width, G::height, G::radius, static_cast<int>(origin));
  overlay_paint_card(cairo, origin, origin, G::width, G::height, G::radius, colors.surface, colors.border);

  cairo_save(cairo);
  cairo_translate(cairo, origin, origin);

  // 标题行：方案名 12 半粗，说明 10 常规次要色，两端对齐、各自在行里垂直居中。
  auto *title = overlay_text_layout(cairo, content.title, G::title_size, PANGO_WEIGHT_SEMIBOLD);
  const auto title_size = overlay_text_size(title);
  mode_badge_set_rgb(cairo, colors.text);
  shuangpin_keymap_show_text(cairo, title, G::inset, G::header_top + (G::header_height - title_size.height) / 2.0);
  g_object_unref(title);
  auto *subtitle = overlay_text_layout(cairo, kShuangpinKeymapSubtitle, G::subtitle_size);
  const auto subtitle_size = overlay_text_size(subtitle);
  mode_badge_set_rgb(cairo, colors.secondary);
  shuangpin_keymap_show_text(cairo, subtitle, G::width - G::inset - subtitle_size.width,
                             G::header_top + (G::header_height - subtitle_size.height) / 2.0);
  g_object_unref(subtitle);

  for (std::size_t row = 0; row < content.rows.size(); ++row) {
    const auto &keys = content.rows[row];
    for (std::size_t index = 0; index < keys.size(); ++index) {
      const auto &key = keys[index];
      const auto rect = shuangpin_keymap_key_rect(row, index, keys.size());
      const bool lit = content.highlighted && key.key.size() == 1 && key.key.front() == content.highlighted;
      mode_badge_rounded_rect(cairo, rect.x + 0.5, rect.y + 0.5, rect.width - 1.0, rect.height - 1.0, G::key_radius);
      mode_badge_set_rgb(cairo, lit ? colors.accent : colors.key);
      cairo_fill_preserve(cairo);
      // 高亮键的边是白色 28%，其余是淡分隔线，与 macOS 相同。
      if (lit) overlay_set_rgba(cairo, 0xFFFFFF, 0.28);
      else mode_badge_set_rgb(cairo, colors.key_border);
      cairo_set_line_width(cairo, 1.0);
      cairo_stroke(cairo);

      auto *label = overlay_text_layout(cairo, key.key, G::key_label_size, PANGO_WEIGHT_BOLD, kOverlayMonospaceFamily);
      mode_badge_set_rgb(cairo, lit ? colors.accent_text : colors.text);
      shuangpin_keymap_show_text(cairo, label, rect.x + 7.0, rect.y + 4.0);
      g_object_unref(label);

      if (!key.codes.empty()) {
        auto *codes = overlay_text_layout(cairo, key.codes, G::codes_size, PANGO_WEIGHT_MEDIUM);
        pango_layout_set_width(codes, static_cast<int>((rect.width - 4.0) * PANGO_SCALE));
        pango_layout_set_ellipsize(codes, PANGO_ELLIPSIZE_END);
        const auto size = overlay_text_size(codes);
        if (lit) overlay_set_rgba(cairo, colors.accent_text, 0.86);
        else mode_badge_set_rgb(cairo, colors.secondary);
        shuangpin_keymap_show_text(cairo, codes, rect.x + (rect.width - size.width) / 2.0,
                                   rect.y + rect.height - size.height - 4.0);
        g_object_unref(codes);
      }
    }
  }

  // 零声母一行，10 常规次要色，水平居中，放不下就在末尾省略。
  auto *footer = overlay_text_layout(cairo, content.zero_initials, G::footer_size);
  pango_layout_set_width(footer, static_cast<int>((G::width - 2.0 * G::inset) * PANGO_SCALE));
  pango_layout_set_ellipsize(footer, PANGO_ELLIPSIZE_END);
  const auto footer_size = overlay_text_size(footer);
  mode_badge_set_rgb(cairo, colors.secondary);
  shuangpin_keymap_show_text(cairo, footer, (G::width - footer_size.width) / 2.0,
                             shuangpin_keymap_footer_top() + (G::footer_height - footer_size.height) / 2.0);
  g_object_unref(footer);
  cairo_restore(cairo);
}

}  // namespace msime::linux_host
