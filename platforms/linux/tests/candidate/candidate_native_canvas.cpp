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
int main() {
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
