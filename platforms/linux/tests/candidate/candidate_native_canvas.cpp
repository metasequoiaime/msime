#include "../../src/candidates/NativeCanvas.h"
#include "../../src/candidates/NativeCanvasSnapshot.h"
#include <cassert>
#include <iostream>

namespace host = msime::linux_host;
using Json = nlohmann::json;

host::NativeCanvasImage image(int width, int height, std::uint32_t color) {
  host::NativeCanvasImage surface(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height), cairo_surface_destroy);
  auto *cr = cairo_create(surface.get());
  cairo_set_source_rgb(cr, ((color >> 16) & 255) / 255.0, ((color >> 8) & 255) / 255.0, (color & 255) / 255.0);
  cairo_paint(cr); cairo_destroy(cr);
  return surface;
}
std::uint32_t pixel(cairo_surface_t *surface, int x, int y) {
  cairo_surface_flush(surface);
  std::uint32_t value = 0;
  std::memcpy(&value, cairo_image_surface_get_data(surface) + y * cairo_image_surface_get_stride(surface) + x * 4, 4);
  return value;
}
// 黑底上的抗锯齿像素仍保留分段色主通道，小字号的单个分隔符不保证有足够纯色像素。
int segment_ink(cairo_surface_t *surface, const host::NativeCanvasLayout &layout, std::uint32_t color) {
  const int own_channel = color == 0xff0000u ? 0 : color == 0x00ff00u ? 1 : 2;
  int ink = 0;
  for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) {
    const auto value = pixel(surface, x, y);
    const int rgb[3] = {static_cast<int>((value >> 16) & 0xff), static_cast<int>((value >> 8) & 0xff),
        static_cast<int>(value & 0xff)};
    const int own = rgb[own_channel], other = rgb[(own_channel + 1) % 3], third = rgb[(own_channel + 2) % 3];
    if (own > 60 && other < own / 2 && third < own / 2) ++ink;
  }
  return ink;
}
void skin_layout() {
  const auto target = image(1000, 1000, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 12";
  content.preedit = "synthetic";
  content.items = {{"1. ", "alpha"}, {"2. ", "beta"}};
  const auto original = host::measure_native_canvas(cr, assets, content);
  // 12 point 在默认 96dpi 下是 16 DIP；绝对字号不得再乘 point 换算。
  assert(std::abs(original.metrics.font_size - 16) < 0.001);
  assert(original.preedit.height == host::native_canvas_text_size(cr, original.metrics.font, "国Mg").second);
  content.font = "Sans 16px";
  assert(host::measure_native_canvas(cr, assets, content).metrics.font_size == 16);
  content.font = "Sans 12";
  assets.geometry = {{"padding", {{"value", 0.5}, {"em", true}}},
      {"itemGap", {{"value", 1.1}, {"em", true}}},
      {"itemExtraHeight", {{"value", 20}, {"em", false}}},
      {"numberInset", {{"value", 12}, {"em", false}}},
      {"itemPaddingRight", {{"value", 14}, {"em", false}}},
      {"numberGap", {{"value", 9}, {"em", false}}}, {"numberScale", 0.5},
      {"preeditMarginDip", {{"top", 3}, {"right", 5}, {"bottom", 7}, {"left", 11}}}};
  assets.horizontal = {{"itemGap", {{"value", 10}, {"em", false}}},
      {"itemMinWidth", {{"value", 150}, {"em", false}}}};
  assets.vertical = {{"numberGap", {{"value", 13}, {"em", false}}}};
  auto vertical = host::measure_native_canvas(cr, assets, content);
  assert(vertical.metrics.content[0] == 8 && vertical.metrics.gap == 18);
  assert(vertical.metrics.number_gap == 13);
  assert(vertical.preedit.x == 19 && vertical.preedit.y == 11);
  assert(vertical.items[0].y == vertical.preedit.y + vertical.preedit.height + 7);
  assert(vertical.items[0].height >= 41.6 && vertical.items[0].height < 41.601);
  assert(vertical.items[1].y == vertical.items[0].y + vertical.items[0].height + 18);
  assert(vertical.items[0].contains(vertical.items[0].x + 1, vertical.items[0].y + 1));
  assert(!vertical.items[0].contains(vertical.items[0].x + 1, vertical.items[0].y + vertical.items[0].height + 1));
  assert(host::native_canvas_text_size(cr, vertical.metrics.number_font, "12.").first <
         host::native_canvas_text_size(cr, vertical.metrics.font, "12.").first);
  content.horizontal = true;
  auto horizontal = host::measure_native_canvas(cr, assets, content);
  assert(horizontal.items[0].width == 150 && horizontal.metrics.number_gap == 9);
  assert(horizontal.items[1].x - horizontal.items[0].x == 166);
  assets.fit_content = true;
  horizontal = host::measure_native_canvas(cr, assets, content);
  assert(horizontal.items[1].x - horizontal.items[0].x == 160);
  assert(horizontal.metrics.right == horizontal.metrics.number_inset);
  assets.min_width = 500;
  assets.horizontal["justifyRows"] = true;
  horizontal = host::measure_native_canvas(cr, assets, content);
  assert(horizontal.panel.width == 500);
  assert(horizontal.items[0].width == horizontal.items[1].width);
  assert(horizontal.items.back().x + horizontal.items.back().width == horizontal.panel.width - 8);
  assets.horizontal["justifyRows"] = false;
  horizontal = host::measure_native_canvas(cr, assets, content);
  assert(horizontal.items[0].width == 150 && horizontal.panel.width == 500);
  content.items[1].text = "synthetic\nsecond line\nthird line";
  const auto multiline = host::measure_native_canvas(cr, assets, content);
  assert(multiline.items[0].height == multiline.items[1].height);
  assert(multiline.items[0].height >= host::native_canvas_text_size(cr, multiline.metrics.font, content.items[1].text).second);
  content.items[1].text = "beta";
  // 显式 content insets 优先；九宫格边宽与 padding 相加只用于未指定 insets 的包。
  assets.width = {2, 3, 4, 5};
  auto metrics = host::native_canvas_metrics(cr, assets, content);
  assert((metrics.content == std::array<double, 4>{10, 11, 12, 13}));
  assets.explicit_content = true;
  assets.content = {20, 21, 22, 23};
  metrics = host::native_canvas_metrics(cr, assets, content);
  assert(metrics.content == assets.content);
  assets.explicit_line_gap = true;
  assets.line_gap = 0;
  vertical = host::measure_native_canvas(cr, assets, content);
  assert(vertical.items[0].y == vertical.preedit.y + vertical.preedit.height);
  content.preedit.clear();
  vertical = host::measure_native_canvas(cr, assets, content);
  assert(vertical.items[0].y == 20);
  assets.font_family = "monospace";
  metrics = host::native_canvas_metrics(cr, assets, content);
  auto *font = pango_font_description_from_string(metrics.font.c_str());
  assert(std::string(pango_font_description_get_family(font)) == "monospace,Sans");
  pango_font_description_free(font);
  assert(metrics.font_size == 16);
  // 实际绘制后的文字像素必须全部落在测量与点击共用的候选矩形中。
  assets = {};
  assets.geometry = {{"itemExtraHeight", {{"value", 24}, {"em", false}}},
      {"numberScale", 0.5}, {"numberGap", {{"value", 18}, {"em", false}}}};
  content = {};
  content.font = "Sans 12";
  content.items = {{"1.", "synthetic"}};
  content.selected = 0;
  auto painted = host::measure_native_canvas(cr, assets, content);
  host::paint_native_canvas(cr, assets, content, painted,
      {{"surface", "#000000"}, {"selected", "#000000"}, {"selected_text", "#FFFFFF"}, {"selected_number", "#FFFFFF"}});
  int ink = 0;
  for (int y = 0; y < painted.height; ++y) for (int x = 0; x < painted.width; ++x) {
    if ((pixel(target.get(), x, y) & 0xffffff) == 0) continue;
    ++ink;
    assert(painted.items[0].contains(x, y));
  }
  assert(ink > 100);
  // 序号缩放为零时不绘制，但仍保留来源的 9 DIP 序号栏与 number_gap。
  assets.geometry["numberScale"] = 0.0;
  const auto no_numbers = host::measure_native_canvas(cr, assets, content);
  const auto zero_target = image(400, 200, 0);
  auto *zero_cr = cairo_create(zero_target.get());
  host::paint_native_canvas(zero_cr, assets, content, no_numbers,
      {{"surface", "#000000"}, {"selected", "#000000"}, {"selected_text", "#FFFFFF"}, {"selected_number", "#FFFFFF"}});
  const double text_left = no_numbers.items[0].x + no_numbers.metrics.number_inset + 9 + 18;
  int text_ink = 0;
  for (int y = 0; y < no_numbers.height; ++y) for (int x = 0; x < no_numbers.width; ++x) {
    if ((pixel(zero_target.get(), x, y) & 0xffffff) == 0) continue;
    ++text_ink;
    assert(x >= text_left && no_numbers.items[0].contains(x, y));
  }
  assert(text_ink > 100);
  cairo_destroy(zero_cr);
  content.items[0].label.clear();
  assert(no_numbers.width - host::measure_native_canvas(cr, assets, content).width == 27);
  cairo_destroy(cr);
}

void skin_appearance() {
  host::NativeCanvasAssets assets;
  assets.card_radius = 0;
  assets.item_radius = 12;
  assets.geometry = {{"itemCornersFollowCard", true}, {"itemExtraHeight", {{"value", 24}, {"em", false}}},
      {"itemMinWidth", {{"value", 120}, {"em", false}}}, {"selectedBarWidth", {{"value", 1}, {"em", false}}},
      {"selectedBarHeightEm", 0.0}};
  host::NativeCanvasContent content;
  content.font = "Sans 12";
  content.horizontal = true;
  content.items = {{"", " "}, {"", " "}};
  content.selected = 0;
  const auto target = image(500, 300, 0);
  auto *cr = cairo_create(target.get());
  auto layout = host::measure_native_canvas(cr, assets, content);
  const auto rect = layout.items[0];
  // 显式零卡片圆角仍保持贴角；中间的两个角保留普通圆角。
  assert((host::native_canvas_item_corners(content, layout, 0) == std::array<double, 4>{0, 12, 12, 0}));
  assets.min_width = 400;
  layout = host::measure_native_canvas(cr, assets, content);
  assert((host::native_canvas_item_corners(content, layout, 1) == std::array<double, 4>{12, 12, 12, 12}));
  assets.min_width = 0;
  Json palette = {{"surface", "#000000"}, {"selected", "#FF0000"}, {"hover", "#00FF00"},
      {"accent", "#0000FF"}, {"selected_bar", "#00000000"}, {"show_selected_bar", true}};
  layout = host::measure_native_canvas(cr, assets, content);
  assert(layout.metrics.bar_width == 3 && layout.metrics.bar_height == 6);
  // 候选窗不透明度只淡化卡片底色：选中高亮仍不透明，卡片其余部分的 alpha 按比例降低。
  {
    const auto translucent = image(500, 300, 0);
    cairo_t *faded = cairo_create(translucent.get());
    cairo_set_operator(faded, CAIRO_OPERATOR_CLEAR);
    cairo_paint(faded);
    cairo_set_operator(faded, CAIRO_OPERATOR_OVER);
    auto see_through = assets;
    see_through.opacity = 0.5;
    host::paint_native_canvas(faded, see_through, content, layout, palette);
    assert(pixel(translucent.get(), rect.x + 2, rect.y + rect.height / 2) >> 24 == 0xff);
    const auto card = pixel(translucent.get(), layout.items[1].x + 15, layout.items[1].y + layout.items[1].height / 2) >> 24;
    assert(card >= 0x7f && card <= 0x80);
    cairo_destroy(faded);
  }
  host::paint_native_canvas(cr, assets, content, layout, palette);
  assert((pixel(target.get(), rect.x, rect.y) & 0xffffff) == 0xff0000);
  assert((pixel(target.get(), rect.x + rect.width - 1, rect.y) & 0xffffff) == 0);
  const int middle = rect.y + rect.height / 2;
  // 外置条骑在左缘；透明条色回退到 accent，不能变成消失。
  assert((pixel(target.get(), rect.x - 1, middle) & 0xffffff) == 0x0000ff);
  assets.horizontal["selectedBarInside"] = true;
  layout = host::measure_native_canvas(cr, assets, content);
  host::paint_native_canvas(cr, assets, content, layout, palette, 0, {1, -1});
  assert((pixel(target.get(), rect.x - 1, middle) & 0xffffff) == 0);
  assert((pixel(target.get(), layout.items[1].x + 15, middle) & 0xffffff) == 0x00ff00);
  host::paint_native_canvas(cr, assets, content, layout, palette, 0, {1, 1});
  assert((pixel(target.get(), layout.items[1].x + 15, middle) & 0xffffff) == 0xff0000);
  host::paint_native_canvas(cr, assets, content, layout, palette);
  assert((pixel(target.get(), layout.items[1].x + 15, middle) & 0xffffff) == 0);
  content.preedit = "synthetic";
  content.preedit_cursor = 2;
  palette.update({{"text", "#FFFFFF"}, {"preedit_text", "#FFFF00"}, {"preedit_caret", "#00FFFF"},
      {"preedit_background", "#880088"}, {"preedit_divider", "#FF8800"}});
  layout = host::measure_native_canvas(cr, assets, content);
  assert((host::native_canvas_item_corners(content, layout, 0) == std::array<double, 4>{12, 12, 12, 0}));
  host::paint_native_canvas(cr, assets, content, layout, palette);
  int text_ink = 0, caret_ink = 0, background_ink = 0, divider_ink = 0;
  for (int y = layout.preedit.y; y < layout.preedit.y + layout.preedit.height; ++y)
    for (int x = layout.preedit.x; x < layout.panel.x + layout.panel.width - layout.metrics.content[1]; ++x) {
      const auto color = pixel(target.get(), x, y) & 0xffffff;
      text_ink += color == 0xffff00; caret_ink += color == 0x00ffff;
      background_ink += color == 0x880088; divider_ink += color == 0xff8800;
    }
  assert(text_ink > 10 && caret_ink > 20 && background_ink > 100 && divider_ink > 100);
  // 显式透明预编辑文字保持透明，不能套用选中条的 alpha 0 跟随规则。
  palette["preedit_text"] = "#00000000";
  palette["preedit_background"] = "#000000";
  palette["preedit_divider"] = "#000000";
  content.preedit_cursor = -1;
  host::paint_native_canvas(cr, assets, content, layout, palette);
  for (int y = layout.preedit.y; y < layout.preedit.y + layout.preedit.height; ++y)
    for (int x = layout.preedit.x + 12; x < layout.preedit.x + layout.preedit.width - 12; ++x)
      assert((pixel(target.get(), x, y) & 0xffffff) == 0);
  // 非 UTF-8 边界不画伪光标，不将变换后的文字偏移当作编辑位置。
  content.preedit = "合成"; content.preedit_cursor = 1;
  layout = host::measure_native_canvas(cr, assets, content);
  host::paint_native_canvas(cr, assets, content, layout, palette);
  for (int y = layout.preedit.y; y < layout.preedit.y + layout.preedit.height; ++y)
    for (int x = layout.preedit.x; x < layout.panel.width; ++x)
      assert((pixel(target.get(), x, y) & 0xffffff) != 0x00ffff);
  const auto merged = host::native_canvas_palette({{"candidate", {{"text", "#112233"}, {"selected_text", "#FFFFFF"}}},
      {"candidate_details", {{"preedit_text", "#445566"}, {"menu_surface", "#123456"}, {"selected_bar", nullptr}}}});
  assert(merged.at("text") == "#112233" && merged.at("selected_text") == "#FFFFFF");
  assert(merged.at("preedit_text") == "#445566" && !merged.contains("selected_bar"));
  content.preedit = "menu"; content.menu = true;
  assets.panel = image(30, 30, 0xff00ff);
  palette.update({{"menu_surface", "#123456"}, {"menu_hover", "#654321"}, {"preedit_background", "#FF00FF"}});
  layout = host::measure_native_canvas(cr, assets, content);
  host::paint_native_canvas(cr, assets, content, layout, palette, 0, {1, -1});
  assert((pixel(target.get(), 5, 5) & 0xffffff) == 0x123456);
  assert((pixel(target.get(), layout.panel.width - 20, layout.preedit.y + 5) & 0xffffff) == 0x123456);
  assert((pixel(target.get(), layout.items[1].x + 15, layout.items[1].y + 15) & 0xffffff) == 0x654321);
  cairo_destroy(cr);
}

// 固定了排位的行未选中时正文画 macOS 的固定色 #379AD3，选中时照旧用选中文字色。
void fixed_position_appearance() {
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 30px";
  content.items = {{"", "MMMM"}};
  content.items[0].fixed = true;
  const Json palette = {{"surface", "#000000"}, {"selected", "#000000"}, {"hover", "#000000"},
      {"text", "#FFFFFF"}, {"selected_text", "#FFFFFF"}};
  const auto ink = [&](int selected) {
    content.selected = selected;
    auto target = image(600, 200, 0);
    auto *cr = cairo_create(target.get());
    const auto layout = host::measure_native_canvas(cr, assets, content);
    host::paint_native_canvas(cr, assets, content, layout, palette);
    std::map<std::uint32_t, int> colors;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) ++colors[pixel(target.get(), x, y) & 0xffffff];
    cairo_destroy(cr);
    return colors;
  };
  auto normal = ink(-1);
  assert(normal[host::kCandidateFixedPositionColor] > 100 && normal[0xffffff] == 0);
  auto selected = ink(0);
  assert(selected[0xffffff] > 100 && selected[host::kCandidateFixedPositionColor] == 0);
  content.items[0].fixed = false;
  auto plain = ink(-1);
  assert(plain[0xffffff] > 100 && plain[host::kCandidateFixedPositionColor] == 0);
}

// 光标后编辑：canvas 包的画法同样把首选开头固定的那几个字用序号色画淡（选中行用选中序号色），有释义时释义的配色不受影响，行的大小不变。
void caret_edit_dim_appearance() {
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 30px";
  content.items = {{"", "MMMM"}};
  const Json palette = {{"surface", "#000000"}, {"selected", "#000000"}, {"hover", "#000000"},
      {"text", "#FFFFFF"}, {"selected_text", "#FFFFFF"}, {"number", "#00FF00"}, {"selected_number", "#0000FF"},
      {"translation", "#FF0000"}};
  const auto ink = [&](std::size_t dim_bytes, int selected) {
    content.items[0].dim_prefix_bytes = dim_bytes;
    content.selected = selected;
    auto target = image(800, 300, 0);
    auto *cr = cairo_create(target.get());
    const auto layout = host::measure_native_canvas(cr, assets, content);
    host::paint_native_canvas(cr, assets, content, layout, palette);
    std::map<std::uint32_t, int> colors;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) ++colors[pixel(target.get(), x, y) & 0xffffff];
    cairo_destroy(cr);
    return std::make_pair(layout.items[0], colors);
  };
  auto [plain_box, plain] = ink(0, -1);
  assert(plain[0xffffff] > 100 && plain[0x00ff00] == 0);
  auto [dim_box, dimmed] = ink(2, -1);
  assert(dimmed[0x00ff00] > 50 && dimmed[0xffffff] > 50 && dimmed[0xffffff] < plain[0xffffff]);
  assert(dim_box.width == plain_box.width && dim_box.height == plain_box.height);
  auto [selected_box, selected] = ink(2, 0);
  assert(selected[0x0000ff] > 50 && selected[0x00ff00] == 0);
  (void)selected_box;
  // 有释义的行：画淡只落在正文开头，释义仍是释义色。
  content.items[0].translation = "MMM";
  auto [glossed_box, glossed] = ink(2, -1);
  assert(glossed[0x00ff00] > 50 && glossed[0xff0000] > 50);
  (void)glossed_box;
}

// 读音在它那一行释义下面单独一行（竖排时释义因此挪到正文下面），按释义色的 0.72 倍透明度画；释义里词性区段的颜色照旧，选中行不减淡。
void pronunciation_appearance() {
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 30px";
  content.items = {{"", "MMMM", "n. MMM", "", Json::array({Json{{"start", 0}, {"length", 2}, {"role", "pos"}}})}};
  const Json palette = {{"surface", "#000000"}, {"selected", "#000000"}, {"hover", "#000000"},
      {"text", "#FFFFFF"}, {"selected_text", "#FFFFFF"}, {"translation", "#FF0000"},
      {"translation_pos", "#00FF00"}, {"selected_translation", "#FF0000"}, {"selected_translation_pos", "#00FF00"}};
  const auto render = [&] {
    auto target = image(1000, 400, 0);
    auto *cr = cairo_create(target.get());
    const auto layout = host::measure_native_canvas(cr, assets, content);
    host::paint_native_canvas(cr, assets, content, layout, palette);
    std::map<std::uint32_t, int> colors;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) ++colors[pixel(target.get(), x, y) & 0xffffff];
    cairo_destroy(cr);
    return std::make_pair(layout.items[0], colors);
  };
  const auto count = [](const std::map<std::uint32_t, int> &colors, std::uint32_t color) {
    const auto found = colors.find(color);
    return found == colors.end() ? 0 : found->second;
  };
  // 0xFF 的 0.72 倍是 0xB8。
  constexpr std::uint32_t dimmed = 0xb80000;
  const auto [plain_box, plain] = render();
  content.items[0].pronunciation = "/MMM/";
  const auto [read_box, read] = render();
  assert(read_box.height > plain_box.height + 10);
  assert(count(read, 0x00ff00) > 3 && count(read, 0xff0000) > 3);
  assert(count(read, dimmed) > count(plain, dimmed) + 20);
  content.selected = 0;
  const auto [selected_box, selected] = render();
  assert(std::abs(selected_box.height - read_box.height) < 0.5);
  assert(count(selected, dimmed) + 20 < count(read, dimmed));
}

void translation_appearance() {
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 30px";
  content.items = {{"", "MMMM", "n. MMM；v. MMM", " MM", Json::array({
      Json{{"start", 0}, {"length", 2}, {"role", "pos"}},
      Json{{"start", 6}, {"length", 3}, {"role", "separator"}},
      Json{{"start", 9}, {"length", 2}, {"role", "pos"}}})}};
  Json palette = {{"surface", "#000000"}, {"selected", "#000000"}, {"hover", "#000000"},
      {"text", "#FFFFFF"}, {"selected_text", "#FFFFFF"}, {"translation", "#FF0000"},
      {"translation_pos", "#00FF00"}, {"translation_separator", "#0000FF"},
      {"selected_translation", "#00FFFF"}, {"selected_translation_pos", "#FF00FF"},
      {"selected_translation_separator", "#FF8000"}};
  const auto render = [&](host::NativeCanvasPointer pointer = {}) {
    auto target = image(1000, 400, 0);
    auto *cr = cairo_create(target.get());
    const auto layout = host::measure_native_canvas(cr, assets, content);
    host::paint_native_canvas(cr, assets, content, layout, palette, 0, pointer);
    std::map<std::uint32_t, int> colors;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) {
      const auto color = pixel(target.get(), x, y) & 0xffffff;
      if (color != 0) assert(layout.items[0].contains(x, y));
      ++colors[color];
    }
    cairo_destroy(cr);
    return std::make_pair(layout, colors);
  };
  const auto normal = render();
  assert(normal.second.at(0xffffff) > 100);
  for (const auto color : {0xff0000u, 0x00ff00u, 0x0000ffu}) assert(normal.second.at(color) > 3);
  assert(render({0, -1}).second == normal.second);
  content.selected = 0;
  const auto selected = render();
  for (const auto color : {0x00ffffu, 0xff00ffu, 0xff8000u}) assert(selected.second.at(color) > 3);
  content.selected = -1;
  assert(render({0, 0}).second == selected.second);
  // 错位的 UTF-8 范围、越界、负数和未知角色不能改变颜色或破坏字形。
  const auto valid_runs = content.items[0].translation_runs;
  for (const auto &run : Json::array({
      Json{{"start", 7}, {"length", 1}, {"role", "pos"}},
      Json{{"start", -1}, {"length", 1}, {"role", "pos"}},
      Json{{"start", 0}, {"length", 999999}, {"role", "pos"}},
      Json{{"start", 3}, {"length", 2}, {"role", "unknown"}}}))
    content.items[0].translation_runs.push_back(run);
  assert(render().second == normal.second);
  content.items[0].translation_runs = valid_runs;
  content.horizontal = true;
  const auto horizontal = render();
  assert(horizontal.first.items[0].height > normal.first.items[0].height);
  assert(horizontal.first.items[0].width < normal.first.items[0].width);
  for (const auto color : {0xff0000u, 0x00ff00u, 0x0000ffu}) assert(horizontal.second.at(color) > 3);
  // 透明的选中分段色回到普通分段色；透明释义色回到正文色乘来源的 0.62。
  content.selected = 0;
  for (const auto key : {"selected_translation", "selected_translation_pos", "selected_translation_separator"})
    palette[key] = "#00000000";
  assert(render().second == horizontal.second);
  content.items[0].translation_runs = Json::array();
  palette["translation"] = "#00000000";
  const auto inherited = render();
  assert(inherited.second.at(0x9e9e9e) > 3);
}

void constrained_text_layout() {
  host::NativeCanvasAssets assets;
  host::NativeCanvasContent content;
  content.font = "Sans 22px";
  const Json palette = {{"surface", "#000000"}, {"selected", "#000000"}, {"hover", "#000000"},
      {"text", "#FFFFFF"}, {"selected_text", "#FFFFFF"}, {"number", "#FFFFFF"},
      {"selected_number", "#FFFFFF"}, {"translation", "#FF0000"},
      {"translation_pos", "#00FF00"}, {"translation_separator", "#0000FF"}};
  const auto target = image(700, 2000, 0);
  auto *cr = cairo_create(target.get());
  const auto render = [&](double limit) {
    const auto layout = host::measure_native_canvas(cr, assets, content, limit);
    assert(layout.width <= limit && layout.height < 2000);
    host::paint_native_canvas(cr, assets, content, layout, palette);
    std::map<std::uint32_t, int> colors;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x) {
      const auto color = pixel(target.get(), x, y) & 0xffffff;
      if (color != 0) {
        const auto hits = std::count_if(layout.items.begin(), layout.items.end(),
            [&](const auto &box) { return box.contains(x, y); });
        assert(hits == 1);
      }
      ++colors[color];
    }
    assert(colors.at(0xffffff) > 30);
    return std::make_pair(layout, colors);
  };
  // 汉字、组合字符、emoji 与无空格长串都必须完整换行，不能靠裁切通过尺寸围栏。
  const std::string sample = "合成测试é🙂";
  for (bool horizontal : {false, true}) for (bool unicode : {false, true}) {
    content.horizontal = horizontal;
    std::string body;
    for (int at = 0; at < 32; ++at) body += unicode ? sample : "MMMM";
    content.items = {{"1.", body}};
    const auto natural = host::measure_native_canvas(cr, assets, content);
    const auto wrapped = render(320).first;
    assert(natural.width > 320 && wrapped.items[0].height > natural.items[0].height);
    auto *line = host::native_canvas_item_layout(cr, wrapped.metrics, content.items[0], horizontal,
        Json::object(), false, wrapped.item_text_widths[0]);
    assert(pango_layout_get_line_count(line) > 1);
    assert(pango_layout_get_text(line) == body);
    g_object_unref(line);
  }
  content.horizontal = false;
  content.items = {{"1.", "MMMM", "n. MMM；v. MMM", "", Json::array({
      Json{{"start", 0}, {"length", 2}, {"role", "pos"}},
      Json{{"start", 6}, {"length", 3}, {"role", "separator"}},
      Json{{"start", 9}, {"length", 2}, {"role", "pos"}}})}};
  const auto narrow = render(240);
  const auto wide = render(600);
  assert(narrow.first.items[0].height > wide.first.items[0].height);
  auto *line = host::native_canvas_item_layout(cr, narrow.first.metrics, content.items[0], false,
      Json::object(), false, narrow.first.item_text_widths[0]);
  assert(std::string(pango_layout_get_text(line)).find("MMMM\nn.") == 0);
  g_object_unref(line);
  line = host::native_canvas_item_layout(cr, wide.first.metrics, content.items[0], false,
      Json::object(), false, wide.first.item_text_widths[0]);
  assert(pango_layout_get_line_count(line) == 1);
  g_object_unref(line);
  const auto segment_counts = [&](const host::NativeCanvasLayout &layout) {
    host::paint_native_canvas(cr, assets, content, layout, palette);
    std::map<std::uint32_t, int> counts;
    for (const auto color : {0xff0000u, 0x00ff00u, 0x0000ffu})
      counts[color] = segment_ink(target.get(), layout, color);
    return counts;
  };
  const auto narrow_segments = segment_counts(narrow.first), wide_segments = segment_counts(wide.first);
  for (const auto color : {0xff0000u, 0x00ff00u, 0x0000ffu}) {
    assert(narrow_segments.at(color) > 2 && wide_segments.at(color) > 2);
  }
  for (int at = 0; at < 24; ++at) content.items[0].translation += " 合成MMM";
  for (bool horizontal : {false, true}) {
    content.horizontal = horizontal;
    const auto wrapped = render(320);
    assert(wrapped.first.items[0].height > narrow.first.items[0].height * 2);
    const auto counts = segment_counts(wrapped.first);
    for (const auto color : {0xff0000u, 0x00ff00u, 0x0000ffu}) assert(counts.at(color) > 2);
  }
  // 每个横排行独立撑高与均分；均分不能重排原先受限的文字。
  content.horizontal = true;
  content.items = {{"1.", "MMMM"}, {"2.", "MM"}, {"3.", std::string(80, 'M')}, {"4.", "MM"}};
  assets.item_radius = 12;
  assets.min_width = 320;
  assets.horizontal = {{"justifyRows", true}, {"itemMinWidth", {{"value", 120}, {"em", false}}},
      {"itemCornersFollowCard", true}};
  const auto rows = render(320).first;
  assert(rows.items[0].y == rows.items[1].y && rows.items[0].height == rows.items[1].height);
  assert(rows.items[2].y > rows.items[1].y && rows.items[3].y > rows.items[2].y);
  assert(rows.items[2].height > rows.items[0].height);
  for (auto at : {1, 2, 3})
    assert(rows.items[at].x + rows.items[at].width == rows.panel.x + rows.panel.width - rows.metrics.content[1]);
  assert(rows.item_text_widths[3] < rows.items[3].width - rows.metrics.number_inset - rows.metrics.right);
  assert((host::native_canvas_item_corners(content, rows, 3) == std::array<double, 4>{12, 12, 0, 0}));
  Json selected_palette = palette;
  selected_palette["selected"] = "#FFFF00";
  for (int at = 0; at < 4; ++at) {
    content.selected = at;
    host::paint_native_canvas(cr, assets, content, rows, selected_palette);
    const auto &box = rows.items[at];
    // 每行末尾均分出的空白仍属于该候选，并使用同一高亮与点击矩形。
    assert((pixel(target.get(), box.x + box.width - 3, box.y + box.height / 2) & 0xffffff) == 0xffff00);
    assert(box.contains(box.x + box.width - 3, box.y + box.height / 2));
  }
  content.selected = -1;
  // 极窄区域中的一个完整字形或序号栏不能变成静默裁字。
  content.items = {{"1.", "M"}};
  assets = {};
  assert(host::measure_native_canvas(cr, assets, content, 30).width > 30);
  // 换行后的拼音光标与辅助行共用测量宽度；翻页不再挤在辅助行末尾。
  content = {};
  content.font = "Sans 22px";
  content.preedit = std::string(60, 'M');
  content.preedit_cursor = content.preedit.size();
  content.auxiliary = std::string(70, 'W');
  content.previous = content.next = true;
  const auto wrapped = host::measure_native_canvas(cr, assets, content, 240);
  assert(wrapped.width <= 240);
  assert(wrapped.preedit.height > 26 && wrapped.auxiliary.height > 26);
  assert(wrapped.auxiliary.x + wrapped.auxiliary.width <= wrapped.panel.x + wrapped.panel.width);
  line = host::native_canvas_text_layout(cr, wrapped.metrics.font, content.preedit, wrapped.preedit_width);
  PangoRectangle strong{}, weak{};
  pango_layout_get_cursor_pos(line, content.preedit_cursor, &strong, &weak);
  assert(strong.y > 0);
  g_object_unref(line);
  host::paint_native_canvas(cr, assets, content, wrapped,
      {{"surface", "#000000"}, {"preedit_text", "#FFFFFF"}, {"preedit_caret", "#00FFFF"}, {"secondary", "#0000FF"}});
  int caret_ink = 0, auxiliary_ink = 0;
  const host::NativeCanvasRect caret{wrapped.preedit.x + strong.x / double(PANGO_SCALE),
      wrapped.preedit.y + strong.y / double(PANGO_SCALE), 2, strong.height / double(PANGO_SCALE)};
  for (int y = 0; y < wrapped.height; ++y) for (int x = 0; x < wrapped.width; ++x) {
    const auto color = pixel(target.get(), x, y) & 0xffffff;
    if (color == 0x00ffff) { assert(caret.contains(x, y)); ++caret_ink; }
    if (color == 0x0000ff) { assert(wrapped.auxiliary.contains(x, y)); ++auxiliary_ink; }
  }
  assert(caret_ink > 20 && auxiliary_ink > 100);
  // 横排翻页独占右列；折行与逐行均分后的候选都不能伸进翻页槽位。
  content = {};
  content.font = "Sans 22px";
  content.horizontal = true;
  content.next = true;
  content.items = {{"1.", "MMMM"}, {"2.", "MMMM"}, {"3.", std::string(40, 'M')}};
  assets.page_arrows = true;
  assets.horizontal = {{"justifyRows", true}};
  const auto paged = host::measure_native_canvas(cr, assets, content, 320);
  assert(paged.pager.visible && paged.width <= 320);
  assert(paged.items.back().y > paged.items.front().y);
  for (const auto &box : paged.items) assert(box.x + box.width < paged.previous.x);
  // 关掉 show_candidate_page_number 时皮肤要的翻页箭头也不画，候选拿回那一列，点不到翻页。
  content.page_arrows = false;
  const auto unpaged = host::measure_native_canvas(cr, assets, content, 320);
  assert(!unpaged.pager.visible);
  assert(host::native_canvas_hit(content, unpaged, paged.next.x + paged.next.width / 2,
                                 paged.next.y + paged.next.height / 2) != -3);
  cairo_destroy(cr);
}

void pager_layout() {
  const Json palette = {{"surface", "#000000"}, {"text", "#FFFFFF"}, {"number", "#FFFFFF"},
      {"hover", "#00FF00"}, {"selected", "#FF0000"}, {"page_arrow_divider", "#0000FF"}};
  for (bool horizontal : {false, true}) for (bool triangle : {false, true}) {
    host::NativeCanvasAssets assets;
    assets.page_arrows = true;
    assets.outer = {5, 7, 9, 11};
    assets.item_radius = 3;
    assets.geometry["pageArrowStyle"] = triangle ? "chevron" : "triangle";
    (horizontal ? assets.horizontal : assets.vertical)["pageArrowStyle"] = triangle ? "triangle" : "chevron";
    host::NativeCanvasContent content;
    content.font = "Sans 20px";
    content.horizontal = horizontal;
    content.next = true;
    content.preedit = "synthetic preedit";
    content.auxiliary = "auxiliary information wider than candidate rows";
    content.items = {{"1.", "alpha"}, {"2.", "beta\nsecond line"}};
    const auto target = image(1000, 600, 0);
    auto *cr = cairo_create(target.get());
    const auto layout = host::measure_native_canvas(cr, assets, content);
    const auto &pager = layout.pager;
    assert(pager.visible && pager.triangle == triangle);
    assert(pager.button_width == (triangle ? horizontal ? 19 : 16 : 14));
    assert(pager.button_height == (triangle ? horizontal ? 35 : 24 : 18));
    assert(layout.previous.width == pager.button_width && layout.next.height == pager.button_height);
    const auto center = [](const host::NativeCanvasRect &box) {
      return std::make_pair(box.x + box.width / 2, box.y + box.height / 2);
    };
    const auto previous = center(layout.previous), next = center(layout.next);
    assert(host::native_canvas_hit(content, layout, previous.first, previous.second) == -1);
    assert(host::native_canvas_hit(content, layout, next.first, next.second) == -3);
    assert(host::native_canvas_hit(content, layout, layout.next.x + layout.next.width, next.second) == -1);
    for (const auto &box : layout.items) {
      assert(!box.contains(previous.first, previous.second) && !box.contains(next.first, next.second));
      if (horizontal) assert(box.x + box.width < layout.previous.x);
      else assert(box.y + box.height < layout.previous.y);
    }
    assert(layout.auxiliary.y > layout.next.y + layout.next.height);
    assert(layout.panel.contains(layout.previous.x, layout.previous.y));
    assert(layout.panel.contains(layout.next.x + layout.next.width - 0.1, layout.next.y + layout.next.height - 0.1));
    if (!horizontal) {
      const double tip = layout.previous.x + (pager.button_width - pager.glyph_width()) / 2;
      assert(std::abs(tip - layout.items.front().x - layout.metrics.number_inset) < 0.001);
    }
    host::paint_native_canvas(cr, assets, content, layout, palette, 0, {-3, -1});
    assert((pixel(target.get(), layout.next.x + 2, layout.next.y + 2) & 0xffffff) == 0x00ff00);
    assert((pixel(target.get(), layout.previous.x + 2, layout.previous.y + 2) & 0xffffff) == 0);
    host::paint_native_canvas(cr, assets, content, layout, palette, 0, {-2, -3});
    assert((pixel(target.get(), layout.next.x + 2, layout.next.y + 2) & 0xffffff) == 0xff0000);
    assert((pixel(target.get(), layout.previous.x + 2, layout.previous.y + 2) & 0xffffff) == 0);
    if (triangle && horizontal) {
      assert(layout.page_divider.width == 1);
      assert((pixel(target.get(), layout.page_divider.x, layout.page_divider.y + 2) & 0xffffff) == 0x0000ff);
    } else assert(layout.page_divider.width == 0);
    // 禁用字形仍存在；同一颜色按来源透明度衰减，不能把不可用箭头直接删掉。
    host::paint_native_canvas(cr, assets, content, layout, palette);
    int disabled_ink = 0, enabled_ink = 0;
    for (int y = layout.previous.y; y < layout.previous.y + pager.button_height; ++y)
      for (int x = layout.previous.x; x < layout.previous.x + pager.button_width; ++x) {
        const auto value = pixel(target.get(), x, y) & 255;
        disabled_ink += value > 0;
        assert(value <= (triangle ? 64u : 77u));
      }
    for (int y = layout.next.y; y < layout.next.y + pager.button_height; ++y)
      for (int x = layout.next.x; x < layout.next.x + pager.button_width; ++x)
        enabled_ink += (pixel(target.get(), x, y) & 255) > 100;
    assert(disabled_ink > 3 && enabled_ink > 3);
    content.previous = true;
    assert(host::native_canvas_hit(content, layout, previous.first, previous.second) == -2);
    // 均分只分配候选区，不能把翻页槽位覆盖掉；没有辅助行也保留独立翻页区。
    content.auxiliary.clear();
    assets.min_width = 700;
    assets.horizontal["justifyRows"] = true;
    auto justified = host::measure_native_canvas(cr, assets, content);
    if (horizontal) assert(justified.items.back().x + justified.items.back().width < justified.previous.x);
    content.previous = content.next = false;
    assert(host::measure_native_canvas(cr, assets, content).pager.visible);
    assets.expand_panel = true;
    content.expandable = true;
    assert(host::measure_native_canvas(cr, assets, content).pager.visible == !horizontal);
    assets.expand_panel = false;
    content.menu = true;
    assert(!host::measure_native_canvas(cr, assets, content).pager.visible);
    content.menu = false;
    assets.page_arrows = false;
    const auto hidden = host::measure_native_canvas(cr, assets, content);
    assert(!hidden.pager.visible && hidden.previous.width == 0 && hidden.next.width == 0);
    assert(host::native_canvas_hit(content, hidden, next.first, next.second) >= -1);
    cairo_destroy(cr);
  }
}


Json overlay_metadata() {
  return {{"index", 0}, {"fontSizeDip", 24}, {"minFontSizeDip", 8}, {"maxWidthDip", 0},
      {"alignH", "center"}, {"alignV", "center"}, {"offsetXDip", 0}, {"offsetYDip", 0},
      {"stripBadge", true}, {"resolvedColor", "#FF00FFFF"}};
}

void decoration_overlay() {
  const auto metadata = overlay_metadata();
  assert(host::native_canvas_overlay(metadata));
  for (const auto &[key, value] : std::vector<std::pair<std::string, Json>>{
      {"index", true}, {"index", 1.5}, {"index", -1}, {"fontSizeDip", "24"},
      {"maxWidthDip", -1}, {"stripBadge", 1}, {"alignH", "unsafe"}, {"resolvedColor", "#FF00FF"},
      {"resolvedColor", "#FF00FFZZ"}}) {
    auto invalid = metadata; invalid[key] = value;
    assert(!host::native_canvas_overlay(invalid));
  }
  const auto target = image(2400, 500, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  assets.banner = host::NativeCanvasBanner{image(40, 20, 0x123456), {}, 120, 80, "right",
      host::native_canvas_overlay(metadata)};
  host::NativeCanvasContent content;
  content.font = "Sans 16px";
  content.items = {{"", "placeholder"}, {"1.", "ordinary*"}, {"2.", "second"}};
  content.items[1].decoration_text = "alpha(cloud)";
  content.items[1].decoration_badge = "cloud";
  content.items[2].decoration_text = "beta";
  content.selected = 1;
  auto layout = host::measure_native_canvas(cr, assets, content);
  assert(layout.overlay && layout.overlay->text == "alpha(cloud)");
  assert(layout.panel.y >= 80 && layout.overlay->box.y < layout.panel.y);
  auto &style = *assets.banner->candidate;
  style.stripBadge = false;
  assert(host::measure_native_canvas(cr, assets, content).overlay->text == "alpha(cloud)cloud");
  style.stripBadge = true;
  content.selected = 2;
  assert(host::measure_native_canvas(cr, assets, content).overlay->text == "beta");
  content.selected = 0;
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  content.selected = -1;
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  style.index = 1;
  assert(host::measure_native_canvas(cr, assets, content).overlay->text == "alpha(cloud)");
  style.index = 3;
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  style.index = 1;
  // 九种对齐真实绘制；文字和声明框相对定位，装饰区域不获得候选命中。
  for (const auto *h : {"left", "center", "right"}) for (const auto *v : {"top", "center", "bottom"}) {
    style.alignH = h; style.alignV = v;
    layout = host::measure_native_canvas(cr, assets, content);
    assert(layout.overlay);
    cairo_save(cr); cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR); cairo_paint(cr); cairo_restore(cr);
    host::paint_native_canvas(cr, assets, content, layout, Json::object());
    int ink = 0;
    for (int y = 0; y < layout.panel.y; ++y) for (int x = 0; x < layout.width; ++x) {
      if ((pixel(target.get(), x, y) & 0xffffff) != 0xff00ff) continue;
      ++ink;
      assert(host::native_canvas_hit(content, layout, x, y) == -1);
      const auto &box = layout.overlay->ink;
      assert(x >= box.x && x < box.x + box.width && y >= box.y && y < box.y + box.height);
    }
    assert(ink > 10);
  }
  content.items[1].decoration_text = std::string(300, 'W');
  style.maxWidthDip = 1;
  style.fontSizeDip = 24; style.minFontSizeDip = 8;
  layout = host::measure_native_canvas(cr, assets, content);
  assert(layout.overlay->font_size == 8 && layout.overlay->text.size() == 300);
  assert(layout.overlay->box.width > assets.banner->width && layout.width > 500);
  assert(layout.overlay->ink.x >= 0 && layout.overlay->ink.x + layout.overlay->ink.width <= layout.width);
  style.offsetXDip = -1000;
  assert(host::measure_native_canvas(cr, assets, content).width > layout.width);
  content.menu = true;
  const auto menu = host::measure_native_canvas(cr, assets, content);
  assert(!menu.overlay && menu.panel.y == 0);
  content.menu = false;
  assets.banner.reset();
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  const auto dark = host::native_canvas_palette({{"candidate", nullptr}}, true);
  assert(host::theme_color(dark.at("surface"))->rgb == host::candidate_native_palette(true).surface);
  assert(host::theme_color(dark.at("text"))->rgb == host::candidate_native_palette(true).text);
  content = {};
  content.font = "Sans 16px";
  content.auxiliary = "MMMM";
  const auto auxiliary = host::measure_native_canvas(cr, assets, content);
  host::paint_native_canvas(cr, assets, content, auxiliary, dark);
  int auxiliary_ink = 0;
  for (int y = auxiliary.auxiliary.y; y < auxiliary.auxiliary.y + auxiliary.auxiliary.height; ++y)
    for (int x = auxiliary.auxiliary.x; x < auxiliary.auxiliary.x + auxiliary.auxiliary.width; ++x)
      auxiliary_ink += (pixel(target.get(), x, y) & 0xffffff) == host::candidate_native_palette(true).number;
  assert(auxiliary_ink > 10);
  cairo_destroy(cr);
}

// 非 canvas 包沿用最新 chrome 布局；图内文字、命中和展开网格共用同一份普通页快照。
void chrome_decoration_overlay() {
  const auto target = image(2400, 800, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  assets.chrome = true;
  auto animation = std::make_shared<host::NativeCanvasAnimation>();
  animation->frames = {image(40, 20, 0x123456)};
  animation->delays_ms = {0};
  assets.band.image = animation;
  assets.band.top = 80;
  assets.band.width = 120;
  assets.band.candidate = host::native_canvas_overlay(overlay_metadata());
  host::NativeCanvasContent content;
  content.font = "Sans 16px";
  content.items = {{"1", "alpha"}, {"2", "beta"}};
  content.items[0].decoration_text = "alpha";
  content.items[1].decoration_text = "beta";
  content.selected = 1;
  for (bool horizontal : {false, true}) {
    content.horizontal = horizontal;
    auto layout = host::measure_native_canvas(cr, assets, content);
    assert(layout.chrome && layout.overlay && layout.overlay->text == "beta");
    host::paint_native_canvas(cr, assets, content, layout, {{"surface", "#000000"}});
    int ink = 0;
    for (int y = 0; y < layout.height; ++y) for (int x = 0; x < layout.width; ++x)
      ink += (pixel(target.get(), x, y) & 0xffffff) == 0xff00ff;
    assert(ink > 10);
    // 文字超出声明框后整体平移，不让候选命中留在原位置。
    assets.band.candidate->offsetXDip = -500;
    assets.band.candidate->offsetYDip = -100;
    layout = host::measure_native_canvas(cr, assets, content);
    assert(layout.overlay->ink.x >= 0 && layout.overlay->ink.y >= 0);
    assert(layout.overlay->ink.x + layout.overlay->ink.width <= layout.width);
    assert(layout.overlay->ink.y + layout.overlay->ink.height <= layout.height);
    assert(host::native_canvas_hit(content, layout, layout.items[0].x + 1, layout.items[0].y + 1) == 0);
    assets.band.candidate->offsetXDip = assets.band.candidate->offsetYDip = 0;
  }
  content.decoration_items = content.items;
  content.decoration_selected = 0;
  content.items[0].decoration_text = "grid-word";
  content.selected = 1;
  assert(host::measure_native_canvas(cr, assets, content).overlay->text == "alpha");
  content.decoration_selected = -1;
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  content.decoration_selected = 0;
  assets.band.image.reset();
  assert(!host::measure_native_canvas(cr, assets, content).overlay);
  cairo_destroy(cr);
}

void expanded_panel() {
  const auto target = image(1200, 1200, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  assets.expand_panel = assets.page_arrows = true;
  assets.geometry = {{"numberInset", {{"value", 5}, {"em", false}}},
      {"itemGap", {{"value", 0}, {"em", false}}}};
  host::NativeCanvasContent content;
  content.horizontal = true;
  content.font = "Sans 12";
  content.preedit = "synthetic";
  content.page_size = 3;
  content.items = {{"1.", "a"}, {"2.", "b"}, {"3.", "c"}};
  content.expandable = true;
  const auto collapsed = host::measure_native_canvas(cr, assets, content, 600);
  assert(!collapsed.pager.visible && collapsed.expand.width > 0 && collapsed.grid.empty());
  assert(host::native_canvas_hit(content, collapsed, collapsed.expand.x + 1, collapsed.expand.y + 1) == host::kNativeCanvasHitExpand);
  content.expanded = true;
  for (int at = 0; at < 400; ++at) content.grid_items.push_back({{}, "synthetic"});
  // 没写 expandRows 时与 chrome 的网格一样看得见首行之后 kExpandedGridVisibleRows - 1 行。
  for (const int rows : {0, 2, 4, 8}) {
    if (rows == 0) assets.expand_rows.reset();
    else assets.expand_rows = static_cast<std::size_t>(rows);
    const double extra = rows == 0 ? static_cast<double>(msime::candidate::kExpandedGridVisibleRows - 1) : rows;
    const auto layout = host::measure_native_canvas(cr, assets, content, 600);
    const double first = collapsed.items.front().height;
    assert(std::abs(layout.viewport.height - first - extra * layout.scroll_step) < 0.001);
    assert(layout.grid.size() == 300 && layout.max_scroll > 0 && layout.thumb.width == 4);
    assert(host::native_canvas_hit(content, layout, layout.grid.front().x + 1, layout.grid.front().y + 1) == 3);
    assert(host::native_canvas_hit(content, layout, layout.scrollbar.x + 1, layout.scrollbar.y + 1) == host::kNativeCanvasHitScrollbar);
    assert(host::native_canvas_hit(content, layout, layout.grid.back().x + 1, layout.grid.back().y + 1) == -1);
    content.scroll_offset = 1e9;
    const auto end = host::measure_native_canvas(cr, assets, content, 600);
    assert(end.scroll_offset == end.max_scroll);
    assert(std::abs(end.thumb.y + end.thumb.height - end.viewport.y - end.viewport.height) < 0.001);
    assert(host::native_canvas_hit(content, end, end.items[0].x + 1, end.items[0].y + 1) == -1);
    const auto &last = end.grid.back();
    assert(host::native_canvas_hit(content, end, last.x + 1, last.y + last.height - 1) == 302);
    assert(end.expand.y == layout.expand.y && end.expand.x == layout.expand.x);
    host::paint_native_canvas(cr, assets, content, end, {{"surface", "#ffffff"}});
    content.scroll_offset = -10;
    assert(host::measure_native_canvas(cr, assets, content, 600).scroll_offset == 0);
    content.scroll_offset = 0;
  }
  content.grid_limit = 600;
  assert(host::measure_native_canvas(cr, assets, content, 600).grid.size() == 400);
  const auto short_screen = host::measure_native_canvas(cr, assets, content, 600, 180);
  assert(short_screen.panel.height <= 180 && short_screen.max_scroll > 0);
  content.horizontal = false;
  const auto vertical = host::measure_native_canvas(cr, assets, content, 600);
  assert(vertical.grid.empty() && vertical.expand.width == 0 && vertical.pager.visible);
  content.horizontal = true;
  content.expandable = false;
  assert(host::measure_native_canvas(cr, assets, content, 600).pager.visible);
  content.menu = true;
  assert(host::measure_native_canvas(cr, assets, content, 600).expand.width == 0);
  cairo_destroy(cr);
}

void expanded_identity() {
  Json snapshot = {{"session", 7}, {"generation", 11}, {"candidates", Json::array()}};
  for (int at : {0, 4, 8}) snapshot["candidates"].push_back({{"text", "synthetic"},
      {"id", {{"session", 7}, {"generation", 11}, {"index", at}}}});
  assert(host::native_canvas_snapshot_current(snapshot, 7, 11));
  assert(!host::native_canvas_snapshot_current(snapshot, 7, 10));
  assert(!host::native_canvas_snapshot_current(snapshot, 8, 11));
  for (const auto &value : {Json(-1), Json(1.5), Json("11"), Json(nullptr)}) {
    auto malformed = snapshot;
    malformed["candidates"][1]["id"]["index"] = value;
    assert(!host::native_canvas_snapshot_current(malformed, 7, 11));
  }
  for (const auto *key : {"session", "generation"}) {
    auto malformed = snapshot;
    malformed["candidates"][1]["id"][key] = 12;
    assert(!host::native_canvas_snapshot_current(malformed, 7, 11));
  }
  snapshot["candidates"][1]["id"]["index"] = 0;
  assert(!host::native_canvas_snapshot_current(snapshot, 7, 11));
}

// 「仅当前高亮」：canvas 包按有没有释义变高，没有统一预留，所以竖排横排都按高亮行的标记（视图的 translation_reserved）给它一直留出释义的行数；释义到了、高亮换了一行，卡片都一样高。
void reserved_highlight_gloss() {
  const auto target = image(1000, 600, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  for (const bool horizontal : {false, true}) {
    host::NativeCanvasContent content;
    content.font = "Sans 16px";
    content.horizontal = horizontal;
    content.items = {{"1. ", "alpha"}, {"2. ", "beta"}, {"3. ", "gamma"}};
    const auto plain = host::measure_native_canvas(cr, assets, content);
    content.items[0].reserved_gloss_lines = 1;
    const auto waiting = host::measure_native_canvas(cr, assets, content);
    assert(waiting.items[0].height > plain.items[0].height + 4);
    assert(waiting.height > plain.height + 4);
    if (!horizontal) assert(std::abs(waiting.items[1].height - plain.items[1].height) < 0.5);
    content.items[0].translation = "gloss";
    const auto answered = host::measure_native_canvas(cr, assets, content);
    assert(std::abs(answered.height - waiting.height) < 0.5);
    content.items[0].translation.clear();
    content.items[0].reserved_gloss_lines = 0;
    content.items[1].reserved_gloss_lines = 1;
    content.selected = 1;
    const auto moved = host::measure_native_canvas(cr, assets, content);
    assert(std::abs(moved.height - waiting.height) < 0.5);
    content.items[1].reserved_gloss_lines = 2;
    assert(host::measure_native_canvas(cr, assets, content).height > moved.height + 4);
  }
  cairo_destroy(cr);
}
int main() {
  decoration_overlay();
  reserved_highlight_gloss();
  chrome_decoration_overlay();
  expanded_panel();
  expanded_identity();
  pager_layout();
  skin_layout();
  skin_appearance();
  fixed_position_appearance();
  caret_edit_dim_appearance();
  translation_appearance();
  pronunciation_appearance();
  constrained_text_layout();
  // 完整次数、精确帧边界、跨循环和有限末帧停止唤醒。
  host::NativeCanvasAnimation animation;
  animation.frames = {image(1, 1, 0xff0000), image(1, 1, 0x0000ff)};
  animation.delays_ms = {100, 40};
  animation.play_count = 2;
  const auto sample = [&](std::uint64_t elapsed, std::size_t index, std::optional<std::uint64_t> next) {
    const auto result = host::sample_native_canvas_animation(animation, elapsed);
    assert(result.index == index && result.next_ms == next);
  };
  sample(0, 0, 100); sample(99, 0, 1); sample(100, 1, 40);
  sample(139, 1, 1); sample(140, 0, 100); sample(240, 1, {});
  sample(280, 1, {}); sample(UINT64_MAX, 1, {});
  animation.play_count = 0;
  sample(280, 0, 100); sample(1000000000, 0, 80);
  animation.frames.resize(1); animation.delays_ms.resize(1);
  sample(0, 0, {});
  // 解码回调按整幅画布逐帧扣包预算；失败不能提交部分帧或消耗预算。
  std::string gif("GIF89a");
  const unsigned char header[] = {1, 0, 1, 0, 128, 0, 0, 255, 0, 0, 0, 0, 255};
  gif.append(reinterpret_cast<const char *>(header), sizeof(header));
  for (const unsigned char bits : {0x44, 0x4c}) {
    const unsigned char block[] = {0x21, 0xf9, 4, 4, 1, 0, 0, 0,
        0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, bits, 1, 0};
    gif.append(reinterpret_cast<const char *>(block), sizeof(block));
  }
  gif.push_back(0x3b);
  std::uint64_t budget = 3;
  const auto decoded = host::decode_native_canvas_animation(gif, budget);
  assert(decoded && decoded->frames.size() == 2 && budget == 1);
  assert((pixel(decoded->frames[0].get(), 0, 0) & 0xffffff) == 0xff0000);
  assert((pixel(decoded->frames[1].get(), 0, 0) & 0xffffff) == 0x0000ff);
  assert(!host::decode_native_canvas_animation(gif, budget) && budget == 1);

  // 同一路径只占一份帧缓存；不同路径不能分别绕过整个包的像素预算。
  std::string root_template = (std::filesystem::temp_directory_path() / "msime-canvas-animation-XXXXXX").string();
  const auto *created = ::mkdtemp(root_template.data());
  assert(created);
  const std::filesystem::path root(created);
  struct Cleanup { std::filesystem::path root; ~Cleanup() { std::filesystem::remove_all(root); } } cleanup{root};
  std::filesystem::create_directory(root / "fixture");
  const auto large = image(2048, 2048, 0x123456);
  for (const auto *name : {"a.png", "b.png"})
    assert(cairo_surface_write_to_png(large.get(), (root / "fixture" / name).c_str()) == CAIRO_STATUS_SUCCESS);
  Json package = {{"id", "fixture"}, {"canvas", true},
      {"borderImage", {{"source", "a.png"}, {"sliceDip", {2}}}},
      {"decorations", Json::array({{{"image", "a.png"}, {"widthDip", 20}, {"heightDip", 20}}})}};
  package["fontFamily"] = "monospace";
  package["minWidthDip"] = 320;
  package["geometry"] = {{"padding", {{"value", 3}, {"em", false}}}};
  const auto cached = host::load_native_canvas(package, root, "fixture", true);
  assert(cached && cached->font_family == "monospace" && cached->min_width == 320);
  assert(cached->geometry == package["geometry"]);
  assert(!cached->page_arrows);
  package["pageArrows"] = true;
  assert(host::load_native_canvas(package, root, "fixture", true)->page_arrows);
  package["pageArrows"] = false;
  assert(!host::load_native_canvas(package, root, "fixture", true)->page_arrows);
  package["pageArrows"] = true;
  package["expandPanel"] = true;
  package["expandRows"] = 2;
  const auto expanding = host::load_native_canvas(package, root, "fixture", true);
  assert(expanding->page_arrows && expanding->expand_panel && expanding->expand_rows == std::size_t{2});
  assert(cached && cached->panel_animation == cached->decorations[0].animation);
  // 共享解析结果独立于编辑器声明；默认高亮圆角和两种排列的几何都进入真实测量。
  package["resolvedLayout"] = {{"cornerRadiusDip", 0}, {"itemCornerRadiusDip", 4},
      {"borderWidthDip", 0}, {"fitContent", false}, {"pageArrows", false}, {"expandPanel", false},
      {"horizontal", {{"padding", {{"value", 5}, {"em", false}}},
                      {"itemExtraHeight", {{"value", 2}, {"em", false}}}}},
      {"vertical", {{"padding", {{"value", 7}, {"em", false}}},
                    {"itemExtraHeight", {{"value", 2}, {"em", false}}}}}};
  const auto inherited = host::load_native_canvas(package, root, "fixture", true);
  assert(inherited && inherited->geometry.empty() && inherited->item_radius == 4);
  // 排列默认与显式关闭都读取共享结果，不被编辑器原始声明反向覆盖。
  assert(!inherited->page_arrows && !inherited->expand_panel && !inherited->fit_content);
  package["resolvedLayout"]["pageArrows"] = true;
  package["resolvedLayout"]["expandPanel"] = true;
  package["resolvedLayout"]["expandRows"] = 8;
  package["resolvedLayout"]["fitContent"] = true;
  package["pageArrows"] = false;
  package["expandPanel"] = false;
  const auto resolved_switches = host::load_native_canvas(package, root, "fixture", true);
  assert(resolved_switches->page_arrows && resolved_switches->expand_panel && resolved_switches->fit_content);
  assert(resolved_switches->expand_rows == std::size_t{8});
  // expandRows 夹在 2–8，不是整数时当作没写。
  package["resolvedLayout"]["expandRows"] = 20;
  assert(host::load_native_canvas(package, root, "fixture", true)->expand_rows == std::size_t{8});
  package["resolvedLayout"]["expandRows"] = "4";
  assert(!host::load_native_canvas(package, root, "fixture", true)->expand_rows);
  package["resolvedLayout"].erase("expandRows");
  const auto canvas = image(700, 700, 0);
  auto *context = cairo_create(canvas.get());
  host::NativeCanvasContent factory_content;
  factory_content.font = "Sans 20px";
  factory_content.items = {{"1. ", "synthetic"}};
  auto metrics = host::measure_native_canvas(context, *inherited, factory_content);
  assert(metrics.metrics.content[0] == 9 && metrics.metrics.item_radius == 4);
  factory_content.horizontal = true;
  metrics = host::measure_native_canvas(context, *inherited, factory_content);
  assert(metrics.metrics.content[0] == 7);
  cairo_destroy(context);
  package["decorations"][0]["image"] = "b.png";
  assert(!host::load_native_canvas(package, root, "fixture", true));

  Json banner_package = {{"id", "fixture"}, {"canvas", false}, {"decorationImage", "a.png"},
      {"decorationWidthDip", 120}, {"decorationTopDip", 80}, {"decorationCandidate", overlay_metadata()}};
  const auto traditional = host::load_native_canvas(banner_package, root, "fixture", false);
  assert(traditional && traditional->banner && !traditional->panel && traditional->decorations.empty());
  banner_package.erase("decorationCandidate");
  assert(!host::load_native_canvas(banner_package, root, "fixture", false));
  banner_package["decorationCandidate"] = overlay_metadata();
  banner_package["decorationImage"] = "missing.png";
  assert(!host::load_native_canvas(banner_package, root, "fixture", false));

  const auto target = image(700, 700, 0);
  auto *cr = cairo_create(target.get());
  host::NativeCanvasAssets assets;
  assets.panel = image(30, 30, 0x112233);
  assets.slice = {10, 10, 10, 10};
  assets.width = {8, 6, 4, 2};
  assets.content = {12, 12, 12, 12};
  host::NativeCanvasContent content;
  content.font = "Sans 12";
  content.preedit = "synthetic";
  content.auxiliary = "1/2";
  content.previous = content.next = true;
  content.items = {{"1. ", "alpha"}, {"2. ", "beta"}};
  const auto zero_gap = host::measure_native_canvas(cr, assets, content);
  assets.line_gap = 19;
  const auto gap = host::measure_native_canvas(cr, assets, content);
  assert(gap.items[0].y - zero_gap.items[0].y == 19);
  assert(gap.height - zero_gap.height == 19);
  assert(gap.items[1].y - gap.items[0].y == zero_gap.items[1].y - zero_gap.items[0].y);
  content.preedit.clear();
  const auto hidden_gap = host::measure_native_canvas(cr, assets, content);
  assets.line_gap = 0;
  const auto hidden_zero = host::measure_native_canvas(cr, assets, content);
  assert(hidden_gap.height == hidden_zero.height);
  content.horizontal = true;
  const auto horizontal = host::measure_native_canvas(cr, assets, content);
  assert(horizontal.items[0].y == horizontal.items[1].y);
  assert(horizontal.items[1].x > horizontal.items[0].x + horizontal.items[0].width);
  assert(!horizontal.items[0].contains(horizontal.items[0].x + horizontal.items[0].width, horizontal.items[0].y));

  // 挂件的包含框由面板锚定；等比图片居中，前后层与外留白分别计算。
  const host::NativeCanvasRect panel{20, 30, 200, 100};
  for (const auto &anchor : {"top-left", "top-right", "bottom-left", "bottom-right", "left", "right", "top", "bottom", "center"}) {
    host::NativeCanvasDecoration decoration{image(10, 20, 0xff0000), anchor, 0, 0, 40, 60, 0};
    const auto box = host::native_canvas_decoration_box(decoration, panel);
    assert(box.x >= panel.x && box.y >= panel.y && box.x + box.width <= panel.x + panel.width && box.y + box.height <= panel.y + panel.height);
  }
  assets.decorations.push_back({image(10, 20, 0xff0000), "top-left", -30, -40, 20, 40, 0});
  assets.decorations.push_back({image(10, 20, 0x00ff00), "top-left", 0, 0, 20, 40, 2});
  auto decorated = host::measure_native_canvas(cr, assets, content);
  assert(decorated.panel.x == 30 && decorated.panel.y == 40);
  assert(decorated.width == horizontal.width + 30 && decorated.height == horizontal.height + 40);
  host::paint_native_canvas(cr, assets, content, decorated, Json::object());
  assert((pixel(target.get(), 5, 5) & 0xffffff) == 0xff0000);
  assert((pixel(target.get(), 35, 45) & 0xffffff) == 0x00ff00);
  assets.decorations[1].z = 0;
  host::paint_native_canvas(cr, assets, content, decorated, Json::object());
  assert((pixel(target.get(), 35, 45) & 0xffffff) == 0x112233);

  // 不填中心时保留透明中心；源切片和显示边宽分别参与换算。
  content = {};
  assets.decorations.clear(); assets.fill = false;
  decorated = host::measure_native_canvas(cr, assets, content);
  const auto transparent = host::NativeCanvasImage(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 80, 80), cairo_surface_destroy);
  auto *empty_cr = cairo_create(transparent.get());
  host::paint_native_canvas(empty_cr, assets, content, decorated, Json::object());
  assert((pixel(transparent.get(), 1, 1) & 0xffffff) == 0x112233);
  assert(pixel(transparent.get(), 12, 12) == 0);
  cairo_destroy(empty_cr); cairo_destroy(cr);
  std::cout << "原生 canvas 的横竖排、独立行距、锚点、层级、透明中心与命中边界通过\n";
}
