#pragma once

#include "NativeCanvasAnimation.h"
#include <pango/pangocairo.h>
#include <functional>
#include <map>
#include <vector>

namespace msime::linux_host {

struct NativeCanvasRect {
  double x = 0, y = 0, width = 0, height = 0;
  bool contains(double px, double py) const {
    return px >= x && py >= y && px < x + width && py < y + height;
  }
};
struct NativeCanvasDecoration {
  NativeCanvasImage image;
  std::string anchor;
  double x = 0, y = 0, width = 0, height = 0;
  int z = 2;
  std::shared_ptr<const NativeCanvasAnimation> animation = {};
};
struct NativeCanvasAssets {
  NativeCanvasImage panel;
  std::array<double, 4> slice{}, width{}, content{12, 12, 12, 12}, outer{};
  bool fill = true;
  double line_gap = 0;
  std::vector<NativeCanvasDecoration> decorations;
  std::shared_ptr<const NativeCanvasAnimation> panel_animation = {};
};

inline NativeCanvasRect native_canvas_decoration_box(const NativeCanvasDecoration &decoration,
                                                     const NativeCanvasRect &panel) {
  const auto &anchor = decoration.anchor;
  const double x = anchor.find("left") != std::string::npos ? panel.x :
      anchor.find("right") != std::string::npos || anchor == "right" ? panel.x + panel.width - decoration.width :
      panel.x + (panel.width - decoration.width) / 2;
  const double y = anchor.find("top") != std::string::npos ? panel.y :
      anchor.find("bottom") != std::string::npos ? panel.y + panel.height - decoration.height :
      panel.y + (panel.height - decoration.height) / 2;
  return {x + decoration.x, y + decoration.y, decoration.width, decoration.height};
}

// 包已经经过共享 API 校验；这里仍复核相对路径，且每次换肤只解码一次。
inline std::shared_ptr<NativeCanvasAssets> load_native_canvas(const nlohmann::json &package,
                                                            const std::filesystem::path &directory,
                                                            const std::string &id, bool dark) {
  if (!package.is_object() || package.value("id", std::string{}) != id || !package.value("canvas", false)) return {};
  auto result = std::make_shared<NativeCanvasAssets>();
  std::map<std::string, std::shared_ptr<const NativeCanvasAnimation>> cache;
  std::uint64_t remaining_pixels = 8000000;
  auto load = [&](const nlohmann::json &name) -> std::shared_ptr<const NativeCanvasAnimation> {
    const auto path = fcitx_canvas_resource(directory, id, name);
    if (!path) return {};
    const auto key = path->string();
    if (const auto at = cache.find(key); at != cache.end()) return at->second;
    const auto bytes = read_fcitx_canvas_image(*path);
    auto image = bytes ? decode_native_canvas_animation(*bytes, remaining_pixels) : nullptr;
    cache.emplace(key, image);
    return image;
  };
  if (const auto border = package.find("borderImage"); border != package.end() && border->is_object()) {
    result->panel_animation = load(border->at(!dark && border->contains("lightSource") ? "lightSource" : "source"));
    if (!result->panel_animation) return {};
    result->panel = result->panel_animation->frames.front();
    result->slice = fcitx_canvas_box(border->at(border->contains("stretchDip") ? "stretchDip" : "sliceDip")).value();
    result->width = border->contains("widthDip") ? fcitx_canvas_box(border->at("widthDip")).value() : result->slice;
    result->fill = border->value("fill", true);
  }
  for (std::size_t at = 0; at < 4; ++at) result->content[at] += result->width[at];
  if (package.contains("contentInsetsDip")) {
    const auto &insets = package.at("contentInsetsDip");
    constexpr const char *keys[] = {"top", "right", "bottom", "left"};
    for (std::size_t at = 0; at < 4; ++at)
      result->content[at] = std::max(result->width[at], insets.at(keys[at]).get<double>());
  }
  if (package.contains("outerInsetsDip")) {
    const auto &insets = package.at("outerInsetsDip");
    result->outer = {insets.at("top"), insets.at("right"), insets.at("bottom"), insets.at("left")};
  }
  result->line_gap = package.value("lineGapDip", 0.0);
  for (const auto &item : package.value("decorations", nlohmann::json::array())) {
    auto image = load(item.at("image"));
    if (!image) return {};
    result->decorations.push_back({image->frames.front(), item.value("anchor", std::string("right")),
        item.value("offsetXDip", 0.0), item.value("offsetYDip", 0.0),
        item.at("widthDip"), item.at("heightDip"), item.value("zIndex", 2), std::move(image)});
  }
  std::stable_sort(result->decorations.begin(), result->decorations.end(),
      [](const auto &left, const auto &right) { return left.z < right.z; });
  return result;
}

struct NativeCanvasItem { std::string label, text; };
struct NativeCanvasAction { std::string text; std::function<void()> invoke; };
struct NativeCanvasContent {
  std::string preedit, auxiliary;
  std::string font = "sans 16";
  bool horizontal = false;
  int selected = -1;
  bool previous = false, next = false, wheel_paging = false;
  std::vector<NativeCanvasItem> items;
  // 回调持有原 CandidateList 快照；身份围栏仍由 CandidateWord 与运行时复核。
  std::function<void(int)> select;
  std::function<void(bool)> page;
  std::function<std::vector<NativeCanvasAction>(int)> actions;
};
struct NativeCanvasLayout {
  NativeCanvasRect panel, preedit, auxiliary, previous, next;
  std::vector<NativeCanvasRect> items;
  double width = 0, height = 0;
};

inline std::pair<double, double> native_canvas_text_size(cairo_t *cr, const std::string &font,
                                                        const std::string &text) {
  auto *layout = pango_cairo_create_layout(cr);
  auto *description = pango_font_description_from_string(font.c_str());
  pango_layout_set_font_description(layout, description);
  pango_layout_set_text(layout, text.c_str(), static_cast<int>(text.size()));
  int width = 0, height = 0;
  pango_layout_get_pixel_size(layout, &width, &height);
  pango_font_description_free(description);
  g_object_unref(layout);
  return {static_cast<double>(width), static_cast<double>(height)};
}

inline NativeCanvasLayout measure_native_canvas(cairo_t *cr, const NativeCanvasAssets &assets,
                                                const NativeCanvasContent &content) {
  NativeCanvasLayout layout;
  const auto line = native_canvas_text_size(cr, content.font, "国Mg").second;
  double y = assets.content[0], width = 0;
  if (!content.preedit.empty()) {
    const auto size = native_canvas_text_size(cr, content.font, content.preedit);
    layout.preedit = {assets.content[3], y, size.first, line};
    width = size.first;
    y += line + assets.line_gap;
  }
  const double row_y = y;
  double x = assets.content[3];
  for (const auto &item : content.items) {
    const auto size = native_canvas_text_size(cr, content.font, item.label + item.text);
    const double item_width = size.first + 16, item_height = std::max(line, size.second) + 8;
    layout.items.push_back({x, y, item_width, item_height});
    width = std::max(width, x - assets.content[3] + item_width);
    if (content.horizontal) x += item_width + 4;
    else y += item_height + 4;
  }
  if (content.horizontal && !layout.items.empty()) y = row_y + layout.items.front().height;
  if (!content.horizontal && !layout.items.empty()) y -= 4;
  if (!content.auxiliary.empty() || content.previous || content.next) {
    y += 4;
    const auto size = native_canvas_text_size(cr, content.font, content.auxiliary);
    layout.auxiliary = {assets.content[3], y, size.first, line};
    width = std::max(width, size.first + (content.previous || content.next ? line * 2 + 12 : 0));
    y += line;
  }
  layout.panel = {0, 0, width + assets.content[3] + assets.content[1], y + assets.content[2]};
  auto reach = assets.outer;
  for (const auto &decoration : assets.decorations) {
    const auto box = native_canvas_decoration_box(decoration, layout.panel);
    reach[0] = std::max(reach[0], -box.y);
    reach[1] = std::max(reach[1], box.x + box.width - layout.panel.width);
    reach[2] = std::max(reach[2], box.y + box.height - layout.panel.height);
    reach[3] = std::max(reach[3], -box.x);
  }
  for (auto &edge : reach) edge = std::clamp(edge, 0.0, 500.0);
  auto shift = [&](NativeCanvasRect &rect) { rect.x += reach[3]; rect.y += reach[0]; };
  shift(layout.panel); shift(layout.preedit); shift(layout.auxiliary);
  for (auto &item : layout.items) {
    if (!content.horizontal) item.width = width;
    shift(item);
  }
  layout.previous = {layout.panel.x + layout.panel.width - assets.content[1] - line * 2 - 8,
                     layout.auxiliary.y, line, line};
  layout.next = {layout.previous.x + line + 8, layout.previous.y, line, line};
  layout.width = std::ceil(layout.panel.width + reach[1] + reach[3]);
  layout.height = std::ceil(layout.panel.height + reach[0] + reach[2]);
  return layout;
}

inline void native_canvas_color(cairo_t *cr, const nlohmann::json &palette, const char *key,
                                const char *fallback) {
  const auto color = theme_color(palette.value(key, nlohmann::json(fallback))).value_or(*theme_color(fallback));
  cairo_set_source_rgba(cr, ((color.rgb >> 16) & 255) / 255.0, ((color.rgb >> 8) & 255) / 255.0,
                       (color.rgb & 255) / 255.0, color.alpha / 255.0);
}

inline void native_canvas_image(cairo_t *cr, cairo_surface_t *image, const NativeCanvasRect &source,
                                const NativeCanvasRect &destination) {
  if (!image || source.width <= 0 || source.height <= 0 || destination.width <= 0 || destination.height <= 0) return;
  // 为每一块建立独立源表面，PAD 只延伸该块边缘，避免九宫格相邻块串色。
  auto *tile = cairo_surface_create_for_rectangle(image, source.x, source.y, source.width, source.height);
  cairo_save(cr);
  cairo_rectangle(cr, destination.x, destination.y, destination.width, destination.height);
  cairo_clip(cr);
  cairo_translate(cr, destination.x, destination.y);
  cairo_scale(cr, destination.width / source.width, destination.height / source.height);
  cairo_set_source_surface(cr, tile, 0, 0);
  cairo_pattern_set_extend(cairo_get_source(cr), CAIRO_EXTEND_PAD);
  cairo_paint(cr);
  cairo_restore(cr);
  cairo_surface_destroy(tile);
}

inline void paint_native_canvas(cairo_t *cr, const NativeCanvasAssets &assets, const NativeCanvasContent &content,
                                 const NativeCanvasLayout &layout, const nlohmann::json &palette,
                                 std::uint64_t elapsed_ms = 0) {
  const auto frame = [elapsed_ms](const NativeCanvasImage &image, const std::shared_ptr<const NativeCanvasAnimation> &animation) {
    return animation ? animation->frames[sample_native_canvas_animation(*animation, elapsed_ms).index].get() : image.get();
  };
  const auto widgets = [&](bool front) {
    for (const auto &decoration : assets.decorations) {
      if ((decoration.z >= 1) != front) continue;
      auto box = native_canvas_decoration_box(decoration, layout.panel);
      auto *image = frame(decoration.image, decoration.animation);
      const double width = cairo_image_surface_get_width(image);
      const double height = cairo_image_surface_get_height(image);
      const double ratio = std::min(box.width / width, box.height / height);
      const NativeCanvasRect target{box.x + (box.width - width * ratio) / 2,
          box.y + (box.height - height * ratio) / 2, width * ratio, height * ratio};
      native_canvas_image(cr, image, {0, 0, width, height}, target);
    }
  };
  widgets(false);
  const auto &panel = layout.panel;
  if (assets.panel) {
    auto *image = frame(assets.panel, assets.panel_animation);
    const auto axis = [](double length, double before, double after) {
      const double ratio = before + after > length ? length / (before + after) : 1;
      return std::array<double, 4>{0, before * ratio, length - after * ratio, length};
    };
    const auto sx = axis(cairo_image_surface_get_width(image), assets.slice[3], assets.slice[1]);
    const auto sy = axis(cairo_image_surface_get_height(image), assets.slice[0], assets.slice[2]);
    const double factor = std::min({1.0, panel.width / std::max(1.0, assets.width[3] + assets.width[1]),
                                   panel.height / std::max(1.0, assets.width[0] + assets.width[2])});
    const std::array<double, 4> dx{panel.x, panel.x + assets.width[3] * factor,
        panel.x + panel.width - assets.width[1] * factor, panel.x + panel.width};
    const std::array<double, 4> dy{panel.y, panel.y + assets.width[0] * factor,
        panel.y + panel.height - assets.width[2] * factor, panel.y + panel.height};
    for (int row = 0; row < 3; ++row) for (int col = 0; col < 3; ++col) {
      if (!assets.fill && row == 1 && col == 1) continue;
      native_canvas_image(cr, image, {sx[col], sy[row], sx[col + 1] - sx[col], sy[row + 1] - sy[row]},
          {dx[col], dy[row], dx[col + 1] - dx[col], dy[row + 1] - dy[row]});
    }
  } else {
    native_canvas_color(cr, palette, "surface", "#f8f8f8");
    cairo_rectangle(cr, panel.x, panel.y, panel.width, panel.height);
    cairo_fill(cr);
  }
  const auto text = [&](const std::string &value, const NativeCanvasRect &rect, const char *key, const char *fallback) {
    auto *line = pango_cairo_create_layout(cr);
    auto *font = pango_font_description_from_string(content.font.c_str());
    pango_layout_set_font_description(line, font);
    pango_layout_set_text(line, value.c_str(), static_cast<int>(value.size()));
    native_canvas_color(cr, palette, key, fallback);
    cairo_move_to(cr, rect.x, rect.y);
    pango_cairo_show_layout(cr, line);
    pango_font_description_free(font);
    g_object_unref(line);
  };
  text(content.preedit, layout.preedit, "text", "#202020");
  for (std::size_t at = 0; at < content.items.size(); ++at) {
    auto rect = layout.items[at];
    const bool selected = static_cast<int>(at) == content.selected;
    if (selected) {
      native_canvas_color(cr, palette, "selected", "#dceaff");
      cairo_rectangle(cr, rect.x, rect.y, rect.width, rect.height);
      cairo_fill(cr);
    }
    rect.x += 8; rect.y += 4;
    const auto &item = content.items[at];
    text(item.label, rect, selected ? "selected_number" : "number", "#555555");
    rect.x += native_canvas_text_size(cr, content.font, item.label).first;
    text(item.text, rect, selected ? "selected_text" : "text", "#202020");
  }
  text(content.auxiliary, layout.auxiliary, "secondary", "#555555");
  if (content.previous) text("‹", layout.previous, "text", "#202020");
  if (content.next) text("›", layout.next, "text", "#202020");
  widgets(true);
}

} // msime::linux_host 命名空间
