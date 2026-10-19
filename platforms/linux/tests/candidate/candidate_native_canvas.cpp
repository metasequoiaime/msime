#include "../../src/candidates/NativeCanvas.h"
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

int main() {
  skin_layout();
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
  assert(cached && cached->panel_animation == cached->decorations[0].animation);
  package["decorations"][0]["image"] = "b.png";
  assert(!host::load_native_canvas(package, root, "fixture", true));

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
