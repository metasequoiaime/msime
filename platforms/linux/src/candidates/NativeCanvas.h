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
  // 只保存共享 catalog 的已校验布局字段，按实际排列与字号延迟解析。
  nlohmann::json geometry = nlohmann::json::object();
  nlohmann::json horizontal = nlohmann::json::object(), vertical = nlohmann::json::object();
  std::string font_family;
  double min_width = 0;
  bool explicit_content = false, explicit_line_gap = false, fit_content = false;
  bool page_arrows = false, expand_panel = false;
  double card_radius = 0, item_radius = 0, border_width = 0;
  bool user_radius = false;
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
  result->geometry = package.value("geometry", nlohmann::json::object());
  result->horizontal = package.value("horizontal", nlohmann::json::object());
  result->vertical = package.value("vertical", nlohmann::json::object());
  result->font_family = package.value("fontFamily", std::string{});
  const auto number = [&](const char *key) {
    const auto at = package.find(key);
    return at != package.end() && at->is_number() ? at->get<double>() : 0.0;
  };
  result->card_radius = number("cornerRadiusDip");
  result->item_radius = number("itemCornerRadiusDip");
  result->border_width = number("borderWidthDip");
  result->min_width = package.value("minWidthDip", 0.0);
  result->explicit_content = package.contains("contentInsetsDip");
  result->explicit_line_gap = package.contains("lineGapDip");
  result->fit_content = package.value("fitContent", false);
  // 原生画布只接受 canvas 基底；其出厂开关沿用 fluent，翻页缺省关闭。
  result->page_arrows = package.value("pageArrows", false);
  result->expand_panel = package.value("expandPanel", false);
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

struct NativeCanvasItem {
  std::string label, text;
  std::string translation = {}, suffix = {};
  nlohmann::json translation_runs = nlohmann::json::array();
};
struct NativeCanvasAction { std::string text; std::function<void()> invoke; };
struct NativeCanvasContent {
  std::string preedit, auxiliary;
  std::string font = "sans 16";
  bool horizontal = false;
  int selected = -1;
  // UTF-8 字节位置；-1 表示没有与显示文字同源的光标。
  int preedit_cursor = -1;
  bool menu = false;
  bool previous = false, next = false, wheel_paging = false;
  std::vector<NativeCanvasItem> items;
  // 回调持有原 CandidateList 快照；身份围栏仍由 CandidateWord 与运行时复核。
  std::function<void(int)> select;
  std::function<void(bool)> page;
  std::function<std::vector<NativeCanvasAction>(int)> actions;
};
struct NativeCanvasPointer { int hovered = -1, pressed = -1; };

// 只消费共享层规范化后的颜色，保留用户取色器与可读选中文字的优先级。
inline nlohmann::json native_canvas_palette(const nlohmann::json &document) {
  auto palette = document.value("candidate", nlohmann::json{});
  if (!palette.is_object()) palette = nlohmann::json::object();
  if (const auto details = document.find("candidate_details"); details != document.end() && details->is_object())
    for (const auto &[key, value] : details->items()) if (!value.is_null()) palette[key] = value;
  return palette;
}

struct NativeCanvasMetrics {
  std::string font, number_font;
  double font_size = 0, number_scale = 1, number_min_width = 0;
  std::array<double, 4> content{}, preedit_margin{};
  double gap = 4, extra_height = 8, number_inset = 8, right = 8, number_gap = 0, min_item_width = 0;
  bool custom_height = false, justify = false, corners_follow_card = false, bar_inside = false;
  bool triangle_pager = false;
  double card_radius = 0, item_radius = 0, bar_width = 3, bar_height = 0;
};

inline NativeCanvasMetrics native_canvas_metrics(cairo_t *cr, const NativeCanvasAssets &assets,
                                                  const NativeCanvasContent &content) {
  NativeCanvasMetrics result;
  auto geometry = assets.geometry;
  geometry.update(content.horizontal ? assets.horizontal : assets.vertical);
  auto *font = pango_font_description_from_string(content.font.c_str());
  if (!assets.font_family.empty()) {
    const char *configured = pango_font_description_get_family(font);
    const auto family = assets.font_family + (configured ? "," + std::string(configured) : "");
    pango_font_description_set_family(font, family.c_str());
  }
  // Pango 的普通字号是 point；布局和皮肤 em 使用 DIP，显示缩放随后由 Cairo 统一施加。
  auto *context = pango_cairo_create_context(cr);
  double dpi = pango_cairo_context_get_resolution(context);
  if (dpi <= 0) dpi = pango_cairo_font_map_get_resolution(PANGO_CAIRO_FONT_MAP(pango_context_get_font_map(context)));
  result.font_size = pango_font_description_get_size(font) / static_cast<double>(PANGO_SCALE);
  if (!pango_font_description_get_size_is_absolute(font)) result.font_size *= dpi / 72.0;
  g_object_unref(context);
  auto string = [](const PangoFontDescription *description) {
    char *value = pango_font_description_to_string(description);
    std::string result(value);
    g_free(value);
    return result;
  };
  result.font = string(font);
  result.number_scale = geometry.value("numberScale", 1.0);
  if (geometry.contains("numberScale") || geometry.contains("numberInset") || geometry.contains("numberGap"))
    result.number_min_width = 9;
  // 0 表示隐藏序号；避免构造 0 字号被 Pango 回退为默认字号。
  pango_font_description_set_absolute_size(font, std::max(0.01, result.font_size * result.number_scale) * PANGO_SCALE);
  result.number_font = string(font);
  pango_font_description_free(font);
  const auto length = [&](const char *key, double fallback) {
    if (!geometry.contains(key)) return fallback;
    const auto &value = geometry.at(key);
    const double amount = value.at("value");
    return value.value("em", false) ? std::round(amount * result.font_size) : amount;
  };
  result.content = assets.content;
  if (geometry.contains("padding") && !assets.explicit_content) {
    const double padding = length("padding", 12);
    for (std::size_t at = 0; at < 4; ++at) result.content[at] = assets.width[at] + padding;
  }
  result.card_radius = assets.card_radius;
  result.item_radius = assets.item_radius;
  result.corners_follow_card = assets.user_radius || geometry.value("itemCornersFollowCard", false);
  result.bar_width = std::max(3.0, length("selectedBarWidth", 3));
  result.bar_height = std::max(result.bar_width * 2, result.font_size * geometry.value("selectedBarHeightEm", 0.85));
  result.bar_inside = geometry.value("selectedBarInside", false);
  result.gap = length("itemGap", 4);
  if (content.horizontal && geometry.contains("itemGap") && !assets.fit_content) result.gap += 6;
  result.custom_height = geometry.contains("itemExtraHeight");
  result.extra_height = length("itemExtraHeight", 8);
  // 来源把序号左缘限制在至少 5 DIP；未指定时保留原生窗口的 8 DIP。
  result.number_inset = geometry.contains("numberInset") ? std::max(5.0, length("numberInset", 8)) : 8;
  result.right = length("itemPaddingRight", 8);
  if (assets.fit_content) result.right = result.number_inset;
  result.number_gap = length("numberGap", 0);
  result.min_item_width = content.horizontal ? length("itemMinWidth", 0) : 0;
  result.justify = content.horizontal && geometry.value("justifyRows", false);
  result.triangle_pager = geometry.value("pageArrowStyle", std::string("chevron")) == "triangle";
  if (geometry.contains("preeditMarginDip")) {
    const auto &margin = geometry.at("preeditMarginDip");
    result.preedit_margin = {margin.at("top"), margin.at("right"), margin.at("bottom"), margin.at("left")};
  }
  if (assets.explicit_line_gap || assets.line_gap != 0) result.preedit_margin[2] = assets.line_gap;
  return result;
}

struct NativeCanvasPager {
  bool visible = false, triangle = false;
  double button_width = 0, button_height = 0, gap = 1, glyph = 0, stroke = 0, radius = 0;
  double divider_width = 0, divider_gap = 0, left = 0, right = 0, lift = 0;
  double glyph_width() const { return glyph * (triangle ? 0.84 : 0.5 * 0.55); }
  double width() const { return divider_width + divider_gap + 2 * button_width + gap; }
  double extent() const { return left + width() + right; }
};

inline NativeCanvasPager native_canvas_pager(const NativeCanvasAssets &assets,
    const NativeCanvasContent &content, const NativeCanvasMetrics &metrics, double item_height) {
  NativeCanvasPager pager;
  pager.visible = assets.page_arrows && !(content.horizontal && assets.expand_panel) &&
      !content.menu && !content.items.empty();
  if (!pager.visible) return pager;
  pager.triangle = metrics.triangle_pager;
  const double em = metrics.font_size;
  pager.button_width = std::round(em * 0.72);
  pager.button_height = std::round(em * 0.9);
  pager.glyph = em * 0.5;
  pager.stroke = std::max(1.2, em / 13);
  pager.radius = std::min(3.0, metrics.item_radius);
  if (pager.triangle) {
    pager.glyph = em * 0.52;
    pager.stroke = pager.glyph * 0.375;
    pager.radius = metrics.item_radius;
    pager.gap = 0;
    pager.button_width = std::round(em * (content.horizontal ? 0.96 : 0.81));
    pager.button_height = content.horizontal ? std::round(item_height) : std::round(em * 1.2);
    if (content.horizontal) {
      pager.divider_width = 1;
      pager.divider_gap = std::round(em * 0.12);
      pager.left = std::round(em * 0.3);
    }
  } else if (content.horizontal) {
    pager.left = 2; pager.right = 1;
    pager.lift = std::max(0.0, (item_height - pager.button_height) / 2);
  }
  return pager;
}

struct NativeCanvasLayout {
  NativeCanvasMetrics metrics;
  NativeCanvasPager pager;
  NativeCanvasRect panel, preedit, auxiliary, previous, next, page_divider;
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

// 释义范围按原串字节解释；只给共享层识别的区段加属性，不在宿主重做词性识别。
inline PangoLayout *native_canvas_item_layout(cairo_t *cr, const NativeCanvasMetrics &metrics,
    const NativeCanvasItem &item, bool horizontal, const nlohmann::json &colors = nlohmann::json::object(),
    bool highlighted = false) {
  auto *line = pango_cairo_create_layout(cr);
  auto *font = pango_font_description_from_string(metrics.font.c_str());
  pango_layout_set_font_description(line, font);
  pango_font_description_free(font);
  const std::string gap = item.translation.empty() ? "" : horizontal ? "\n" : " ";
  const auto value = item.text + gap + item.translation + item.suffix;
  pango_layout_set_text(line, value.c_str(), static_cast<int>(value.size()));
  if (item.translation.empty()) return line;
  const auto start = item.text.size() + gap.size(), end = start + item.translation.size();
  auto *attributes = pango_attr_list_new();
  const auto insert = [&](PangoAttribute *attribute, std::size_t from, std::size_t to) {
    attribute->start_index = static_cast<guint>(from);
    attribute->end_index = static_cast<guint>(to);
    pango_attr_list_change(attributes, attribute);
  };
  insert(pango_attr_size_new_absolute(static_cast<int>(std::round(metrics.font_size * 0.78 * PANGO_SCALE))), start, end);
  if (!horizontal) {
    PangoRectangle ink{}, logical{0, 0, static_cast<int>(std::round(metrics.font_size * 0.65 * PANGO_SCALE)), 0};
    insert(pango_attr_shape_new(&ink, &logical), item.text.size(), start);
  }
  const auto color = [&](const char *key) { return theme_color(colors.value(key, nlohmann::json{})); };
  const auto visible = [](const std::optional<ThemeColor> &color) { return color && color->alpha > 0; };
  auto base = color(highlighted ? "selected_text" : "text");
  if (!base) base = color("text");
  if (!base) base = ThemeColor{0x202020, 255};
  auto translation = highlighted ? color("selected_translation") : std::optional<ThemeColor>{};
  if (!visible(translation)) translation = color("translation");
  if (!visible(translation)) {
    translation = base;
    translation->alpha = static_cast<std::uint8_t>(std::round(translation->alpha * 0.62));
  }
  const auto paint = [&](ThemeColor color, std::size_t from, std::size_t to) {
    insert(pango_attr_foreground_new(((color.rgb >> 16) & 255) * 257,
        ((color.rgb >> 8) & 255) * 257, (color.rgb & 255) * 257), from, to);
    insert(pango_attr_foreground_alpha_new(color.alpha * 257), from, to);
  };
  paint(*translation, start, end);
  if (item.translation_runs.is_array()) for (const auto &run : item.translation_runs) {
    if (!run.is_object() || !run.contains("start") || !run.contains("length") ||
        !run["start"].is_number_integer() || !run["length"].is_number_integer() ||
        run["start"] < 0 || run["length"] <= 0 || run["start"] > item.translation.size() ||
        !run.contains("role") || !run["role"].is_string()) continue;
    const auto from = run["start"].get<std::size_t>(), length = run["length"].get<std::size_t>();
    if (length > item.translation.size() - from ||
        !g_utf8_validate(item.translation.data() + from, static_cast<gssize>(length), nullptr)) continue;
    const auto role = run["role"].get<std::string>();
    if (role != "pos" && role != "separator") continue;
    const char *normal = role == "pos" ? "translation_pos" : "translation_separator";
    const char *selected = role == "pos" ? "selected_translation_pos" : "selected_translation_separator";
    auto run_color = highlighted ? color(selected) : std::optional<ThemeColor>{};
    if (!visible(run_color)) run_color = color(normal);
    paint(visible(run_color) ? *run_color : *translation, start + from, start + from + length);
  }
  pango_layout_set_attributes(line, attributes);
  pango_attr_list_unref(attributes);
  return line;
}

inline std::pair<double, double> native_canvas_item_size(cairo_t *cr, const NativeCanvasMetrics &metrics,
                                                        const NativeCanvasItem &item, bool horizontal) {
  auto *line = native_canvas_item_layout(cr, metrics, item, horizontal);
  int width = 0, height = 0;
  pango_layout_get_pixel_size(line, &width, &height);
  g_object_unref(line);
  return {static_cast<double>(width), static_cast<double>(height)};
}

inline NativeCanvasLayout measure_native_canvas(cairo_t *cr, const NativeCanvasAssets &assets,
                                                const NativeCanvasContent &content) {
  NativeCanvasLayout layout;
  layout.metrics = native_canvas_metrics(cr, assets, content);
  const auto &metrics = layout.metrics;
  const auto &padding = metrics.content;
  const auto line = native_canvas_text_size(cr, metrics.font, "国Mg").second;
  const auto number_line = metrics.number_scale == 0 ? 0 : native_canvas_text_size(cr, metrics.number_font, "Mg").second;
  double y = padding[0], width = 0;
  if (!content.preedit.empty()) {
    const auto size = native_canvas_text_size(cr, metrics.font, content.preedit);
    const auto &margin = metrics.preedit_margin;
    layout.preedit = {padding[3] + margin[3], y + margin[0], size.first, line};
    width = size.first + margin[3] + margin[1];
    y += margin[0] + line + margin[2];
  }
  const double row_y = y;
  const double base_height = std::max({line, number_line, metrics.font_size * 1.35 + metrics.extra_height});
  layout.pager = native_canvas_pager(assets, content, metrics, metrics.font_size * 1.35 + metrics.extra_height);
  const auto &pager = layout.pager;
  double x = padding[3], row_height = 0, list_width = 0;
  for (const auto &item : content.items) {
    const auto size = native_canvas_item_size(cr, metrics, item, content.horizontal);
    const auto label = item.label.empty() ? 0 : std::max(metrics.number_min_width,
        metrics.number_scale == 0 ? 0 : native_canvas_text_size(cr, metrics.number_font, item.label).first);
    const double item_height = metrics.custom_height ? std::max(base_height, size.second) :
        std::max({line, number_line, size.second}) + metrics.extra_height;
    row_height = std::max(row_height, item_height);
    const double item_width = std::max(metrics.min_item_width,
        size.first + label + metrics.number_inset + metrics.right + (label > 0 ? metrics.number_gap : 0));
    layout.items.push_back({x, y, item_width, item_height});
    list_width = std::max(list_width, x - padding[3] + item_width);
    width = std::max(width, x - padding[3] + item_width);
    if (content.horizontal) x += item_width + metrics.gap;
    else y += item_height + metrics.gap;
  }
  if (content.horizontal && !layout.items.empty()) {
    y = row_y + row_height;
    for (auto &item : layout.items) item.height = row_height;
  }
  if (!content.horizontal && !layout.items.empty()) y -= metrics.gap;
  const double list_bottom = y;
  if (pager.visible) {
    if (content.horizontal) {
      width = std::max(width, list_width + pager.extent());
      y = std::max(y, row_y + pager.button_height + pager.lift);
    } else {
      const double left = std::max(0.0, padding[3] + metrics.number_inset -
          (pager.button_width - pager.glyph_width()) / 2);
      layout.previous = {left, y + 1, pager.button_width, pager.button_height};
      layout.next = {left + pager.button_width + pager.gap, y + 1, pager.button_width, pager.button_height};
      width = std::max(width, left + pager.width() - padding[3]);
      y += pager.button_height + 2;
    }
  }
  if (!content.auxiliary.empty()) {
    y += 4;
    const auto size = native_canvas_text_size(cr, metrics.font, content.auxiliary);
    layout.auxiliary = {padding[3], y, size.first, line};
    width = std::max(width, size.first);
    y += line;
  }
  width = std::max(width, assets.min_width - padding[3] - padding[1]);
  if (metrics.justify && !layout.items.empty()) {
    const double used = layout.items.back().x + layout.items.back().width - padding[3];
    const double extra = std::max(0.0, width - (pager.visible ? pager.extent() : 0) - used) / layout.items.size();
    for (std::size_t at = 0; at < layout.items.size(); ++at) {
      layout.items[at].x += at * extra;
      layout.items[at].width += extra;
    }
  }
  if (pager.visible && content.horizontal) {
    const double right = padding[3] + width - pager.right;
    const double bottom = std::max(list_bottom, row_y + pager.button_height + pager.lift) - pager.lift;
    layout.previous = {right - 2 * pager.button_width - pager.gap, bottom - pager.button_height,
        pager.button_width, pager.button_height};
    layout.next = {right - pager.button_width, layout.previous.y, pager.button_width, pager.button_height};
    layout.page_divider = {layout.previous.x - pager.divider_gap - pager.divider_width, layout.previous.y,
        pager.divider_width, pager.button_height};
  }
  layout.panel = {0, 0, width + padding[3] + padding[1], y + padding[2]};
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
  shift(layout.previous); shift(layout.next); shift(layout.page_divider);
  for (auto &item : layout.items) {
    if (!content.horizontal) item.width = width;
    shift(item);
  }
  layout.width = std::ceil(layout.panel.width + reach[1] + reach[3]);
  layout.height = std::ceil(layout.panel.height + reach[0] + reach[2]);
  return layout;
}

inline int native_canvas_hit(const NativeCanvasContent &content, const NativeCanvasLayout &layout,
                             double x, double y) {
  for (std::size_t at = 0; at < layout.items.size(); ++at)
    if (layout.items[at].contains(x, y)) return static_cast<int>(at);
  if (layout.pager.visible && content.previous && layout.previous.contains(x, y)) return -2;
  if (layout.pager.visible && content.next && layout.next.contains(x, y)) return -3;
  return -1;
}

// 逐角路径与来源一致：每角上限为短边的一半；0 仍是合法直角。
inline void native_canvas_round_rect(cairo_t *cr, const NativeCanvasRect &rect, std::array<double, 4> radii) {
  const double limit = std::max(0.0, std::min(rect.width, rect.height) / 2);
  for (auto &radius : radii) radius = std::clamp(radius, 0.0, limit);
  const auto [tl, tr, br, bl] = radii;
  const double right = rect.x + rect.width, bottom = rect.y + rect.height;
  constexpr double pi = 3.14159265358979323846;
  cairo_new_sub_path(cr);
  cairo_move_to(cr, rect.x + tl, rect.y);
  cairo_line_to(cr, right - tr, rect.y);
  if (tr > 0) cairo_arc(cr, right - tr, rect.y + tr, tr, -pi / 2, 0);
  cairo_line_to(cr, right, bottom - br);
  if (br > 0) cairo_arc(cr, right - br, bottom - br, br, 0, pi / 2);
  cairo_line_to(cr, rect.x + bl, bottom);
  if (bl > 0) cairo_arc(cr, rect.x + bl, bottom - bl, bl, pi / 2, pi);
  cairo_line_to(cr, rect.x, rect.y + tl);
  if (tl > 0) cairo_arc(cr, rect.x + tl, rect.y + tl, tl, pi, pi * 1.5);
  cairo_close_path(cr);
}

inline std::array<double, 4> native_canvas_item_corners(const NativeCanvasContent &content,
                                                       const NativeCanvasLayout &layout, std::size_t at) {
  const auto &metrics = layout.metrics;
  std::array<double, 4> radii;
  radii.fill(metrics.item_radius);
  if (!metrics.corners_follow_card || layout.items.empty()) return radii;
  const auto &item = layout.items[at];
  const auto &first = layout.items.front();
  double bottom = first.y;
  for (const auto &box : layout.items) bottom = std::max(bottom, box.y + box.height);
  const bool top = content.preedit.empty() && item.y <= first.y + 0.5;
  const bool last = item.y + item.height >= bottom - 0.5;
  const bool left = item.x <= first.x + 0.5;
  const bool end = item.x + item.width >= layout.panel.x + layout.panel.width - metrics.content[1] - 0.5;
  if (top && left) radii[0] = metrics.card_radius;
  if (top && end) radii[1] = metrics.card_radius;
  if (last && end) radii[2] = metrics.card_radius;
  if (last && left) radii[3] = metrics.card_radius;
  return radii;
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
                                 std::uint64_t elapsed_ms = 0, NativeCanvasPointer pointer = {}) {
  const auto frame = [elapsed_ms](const NativeCanvasImage &image, const std::shared_ptr<const NativeCanvasAnimation> &animation) {
    return animation ? animation->frames[sample_native_canvas_animation(*animation, elapsed_ms).index].get() : image.get();
  };
  const auto widgets = [&](bool front) {
    if (content.menu) return;
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
  auto colors = palette;
  // alpha 0 在这些来源槽位表示跟随普通文字或 accent，不是隐藏文字。
  const auto inherit = [&](const char *key, const char *parent, bool transparent_follows = true) {
    const auto color = theme_color(colors.value(key, nlohmann::json{}));
    if (!color || (transparent_follows && color->alpha == 0)) colors[key] = colors.value(parent, nlohmann::json{});
  };
  inherit("preedit_text", "text", false); inherit("preedit_caret", "accent"); inherit("selected_bar", "accent");
  inherit("selected_text", "text"); inherit("selected_number", "number");
  if (content.menu) {
    for (const auto &[key, parent] : std::vector<std::pair<const char *, const char *>>{
        {"surface", "menu_surface"}, {"text", "menu_text"}, {"hover", "menu_hover"}, {"border", "menu_border"}})
      if (colors.contains(parent)) colors[key] = colors.at(parent);
    colors["preedit_text"] = colors.value("text", nlohmann::json{});
    colors["preedit_background"] = colors["preedit_divider"] = "#00000000";
  }
  widgets(false);
  const auto &panel = layout.panel;
  if (assets.panel && !content.menu) {
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
    native_canvas_color(cr, colors, "surface", "#f8f8f8");
    const double radius = layout.metrics.card_radius;
    native_canvas_round_rect(cr, panel, {radius, radius, radius, radius});
    cairo_fill(cr);
  }
  const auto &metrics = layout.metrics;
  const double border_width = content.menu ? 1 : assets.border_width;
  if ((!assets.panel || content.menu) && border_width > 0) {
    native_canvas_color(cr, colors, "border", "#00000000");
    const double half = border_width / 2, radius = std::max(0.0, metrics.card_radius - half);
    native_canvas_round_rect(cr, {panel.x + half, panel.y + half, std::max(0.0, panel.width - 2 * half),
        std::max(0.0, panel.height - 2 * half)}, {radius, radius, radius, radius});
    cairo_set_line_width(cr, border_width); cairo_stroke(cr);
  }
  const auto text = [&](const std::string &value, const NativeCanvasRect &rect, const std::string &font_name,
                        const char *key, const char *fallback) {
    auto *line = pango_cairo_create_layout(cr);
    auto *font = pango_font_description_from_string(font_name.c_str());
    pango_layout_set_font_description(line, font);
    pango_layout_set_text(line, value.c_str(), static_cast<int>(value.size()));
    native_canvas_color(cr, colors, key, fallback);
    cairo_move_to(cr, rect.x, rect.y);
    pango_cairo_show_layout(cr, line);
    pango_font_description_free(font);
    g_object_unref(line);
  };
  if (!content.preedit.empty()) {
    auto background = layout.preedit;
    background.width = std::max(background.width, panel.x + panel.width - metrics.content[1] -
        metrics.preedit_margin[1] - background.x);
    native_canvas_color(cr, colors, "preedit_background", "#00000000");
    const double radius = metrics.item_radius;
    native_canvas_round_rect(cr, background, {radius, radius, radius, radius}); cairo_fill(cr);
    native_canvas_color(cr, colors, "preedit_divider", "#00000000");
    cairo_rectangle(cr, background.x, background.y + background.height - 1, background.width, 1); cairo_fill(cr);
    text(content.preedit, layout.preedit, metrics.font, "preedit_text", "#202020");
    if (!content.menu && content.preedit_cursor >= 0 &&
        static_cast<std::size_t>(content.preedit_cursor) <= content.preedit.size() &&
        (static_cast<std::size_t>(content.preedit_cursor) == content.preedit.size() ||
         (static_cast<unsigned char>(content.preedit[content.preedit_cursor]) & 0xc0) != 0x80)) {
      auto *line = pango_cairo_create_layout(cr);
      auto *font = pango_font_description_from_string(metrics.font.c_str());
      pango_layout_set_font_description(line, font);
      pango_layout_set_text(line, content.preedit.c_str(), static_cast<int>(content.preedit.size()));
      PangoRectangle strong{}, weak{};
      pango_layout_get_cursor_pos(line, content.preedit_cursor, &strong, &weak);
      native_canvas_color(cr, colors, "preedit_caret", "#3584e4");
      cairo_rectangle(cr, layout.preedit.x + strong.x / static_cast<double>(PANGO_SCALE),
          layout.preedit.y + strong.y / static_cast<double>(PANGO_SCALE), 2,
          strong.height / static_cast<double>(PANGO_SCALE)); cairo_fill(cr);
      pango_font_description_free(font); g_object_unref(line);
    }
  }
  const auto show_bar = colors.find("show_selected_bar");
  const bool selection_bar = show_bar != colors.end() && show_bar->is_boolean() && show_bar->get<bool>();
  for (std::size_t at = 0; at < content.items.size(); ++at) {
    auto rect = layout.items[at];
    const bool selected = static_cast<int>(at) == content.selected;
    const bool pressed = static_cast<int>(at) == pointer.pressed;
    const bool hovered = static_cast<int>(at) == pointer.hovered;
    if (selected || pressed || hovered) {
      native_canvas_color(cr, colors, (selected || pressed) && !content.menu ? "selected" : "hover",
          selected || pressed ? "#dceaff" : "#00000010");
      native_canvas_round_rect(cr, rect, native_canvas_item_corners(content, layout, at)); cairo_fill(cr);
      if (!content.menu && (selected || pressed) && selection_bar) {
        native_canvas_color(cr, colors, "selected_bar", "#3584e4");
        const double width = metrics.bar_width, height = metrics.bar_height, radius = width / 2;
        native_canvas_round_rect(cr, {rect.x - (metrics.bar_inside ? 0 : width / 2),
            rect.y + std::max(0.0, (rect.height - height) / 2), width, height},
            {radius, radius, radius, radius}); cairo_fill(cr);
      }
    }
    const auto &item = content.items[at];
    const auto text_height = native_canvas_item_size(cr, metrics, item, content.horizontal).second;
    const auto label_size = native_canvas_text_size(cr, metrics.number_font, item.label);
    const double top = rect.y;
    const bool centered = metrics.custom_height || metrics.number_scale != 1 || !assets.font_family.empty();
    rect.x += metrics.number_inset;
    if (metrics.number_scale > 0 && !item.label.empty()) {
      rect.y = top + (centered ? (rect.height - label_size.second) / 2 : 4);
      text(item.label, rect, metrics.number_font, selected || pressed ? "selected_number" : "number", "#555555");
    }
    // 序号字号为 0 时不画字形，但来源仍保留最小 9 DIP 序号栏与间距。
    if (!item.label.empty()) rect.x += std::max(metrics.number_min_width,
        metrics.number_scale == 0 ? 0 : label_size.first) + metrics.number_gap;
    rect.y = top + (centered ? (rect.height - text_height) / 2 : 4);
    auto *line = native_canvas_item_layout(cr, metrics, item, content.horizontal, colors, selected || pressed);
    native_canvas_color(cr, colors, selected || pressed ? "selected_text" : "text", "#202020");
    cairo_move_to(cr, rect.x, rect.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
  }
  text(content.auxiliary, layout.auxiliary, metrics.font, "secondary", "#555555");
  if (layout.pager.visible) {
    const auto &pager = layout.pager;
    if (pager.divider_width > 0) {
      native_canvas_color(cr, colors, "page_arrow_divider", "#00000000");
      const auto &box = layout.page_divider;
      cairo_rectangle(cr, box.x, box.y, box.width, box.height); cairo_fill(cr);
    }
    for (const bool next : {false, true}) {
      const auto &box = next ? layout.next : layout.previous;
      const int part = next ? -3 : -2;
      const bool enabled = next ? content.next : content.previous;
      if (enabled && (pointer.pressed == part || pointer.hovered == part)) {
        const auto selected = theme_color(colors.value("selected", nlohmann::json{}));
        native_canvas_color(cr, colors, pointer.pressed == part && selected && selected->alpha > 0 ? "selected" : "hover",
            "#00000010");
        native_canvas_round_rect(cr, box, {pager.radius, pager.radius, pager.radius, pager.radius}); cairo_fill(cr);
      }
      const auto glyph = theme_color(colors.value(pager.triangle ? "text" : "number", nlohmann::json{}))
          .value_or(ThemeColor{pager.triangle ? 0x202020u : 0x555555u, 255});
      const double alpha = glyph.alpha / 255.0 * (enabled ? 1 : pager.triangle ? 0.25 : 0.3);
      cairo_set_source_rgba(cr, ((glyph.rgb >> 16) & 255) / 255.0,
          ((glyph.rgb >> 8) & 255) / 255.0, (glyph.rgb & 255) / 255.0, alpha);
      const double cx = box.x + box.width / 2, cy = box.y + box.height / 2;
      const double height = std::min(pager.glyph, box.height), width = pager.glyph_width();
      const double tip = cx + (next ? width / 2 : -width / 2), tail = 2 * cx - tip;
      if (pager.triangle) {
        // 三角逐角切圆后一次填充；禁用时不会因半透明描边重叠而变深。
        using Point = std::pair<double, double>;
        const std::array<Point, 3> corners{{{tip, cy}, {tail, cy - height / 2}, {tail, cy + height / 2}}};
        const double radius = std::clamp(pager.stroke, 0.0, height / 2);
        const auto toward = [radius](Point from, Point to) {
          const double dx = to.first - from.first, dy = to.second - from.second;
          const double length = std::hypot(dx, dy), t = length > 0 ? radius / length : 0;
          return Point{from.first + dx * t, from.second + dy * t};
        };
        const auto start = toward(corners[0], corners[1]);
        cairo_move_to(cr, start.first, start.second);
        for (int at = 1; at <= 3; ++at) {
          const auto corner = corners[at % 3];
          const auto before = toward(corner, corners[at - 1]);
          const auto after = toward(corner, corners[(at + 1) % 3]);
          cairo_line_to(cr, before.first, before.second);
          cairo_curve_to(cr, before.first + (corner.first - before.first) * 2 / 3,
              before.second + (corner.second - before.second) * 2 / 3,
              after.first + (corner.first - after.first) * 2 / 3,
              after.second + (corner.second - after.second) * 2 / 3, after.first, after.second);
        }
        cairo_close_path(cr); cairo_fill(cr);
      } else {
        cairo_move_to(cr, tail, cy - height / 2); cairo_line_to(cr, tip, cy);
        cairo_line_to(cr, tail, cy + height / 2);
        cairo_set_line_width(cr, pager.stroke); cairo_set_line_join(cr, CAIRO_LINE_JOIN_MITER);
        cairo_stroke(cr);
      }
    }
  }
  widgets(true);
}

} // msime::linux_host 命名空间
