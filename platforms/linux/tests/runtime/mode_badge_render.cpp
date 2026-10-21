#include "../src/overlay/ModeBadgePainter.h"

#include <cairo/cairo.h>

#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <string>

namespace {

// 预乘 ARGB32 的一个像素：返回 alpha 和按 alpha 还原的 RGB。
struct Pixel {
  int alpha;
  std::uint32_t rgb;
};

Pixel pixel_at(cairo_surface_t *surface, int x, int y) {
  cairo_surface_flush(surface);
  const auto *data = cairo_image_surface_get_data(surface);
  const auto stride = cairo_image_surface_get_stride(surface);
  const auto value = *reinterpret_cast<const std::uint32_t *>(data + y * stride + x * 4);
  const int alpha = static_cast<int>(value >> 24);
  if (alpha == 0) return {0, 0};
  const auto channel = [&](int shift) {
    return static_cast<std::uint32_t>((((value >> shift) & 0xff) * 255 + alpha / 2) / alpha);
  };
  return {alpha, (channel(16) << 16) | (channel(8) << 8) | channel(0)};
}

// 把画好的徽章存下来给人看：设置了 MSIME_TEST_CANVAS_ARTIFACTS 时写成 PNG，与候选画布测试同一个目录约定。
void save(cairo_surface_t *surface, const char *name) {
  const char *directory = std::getenv("MSIME_TEST_CANVAS_ARTIFACTS");
  if (!directory || !*directory) return;
  cairo_surface_write_to_png(surface, (std::string(directory) + "/" + name).c_str());
}

}  // namespace

// 用徽章真正的画法（ModeBadgePainter.h）画进内存缓冲，检查本体、描边、阴影边和淡出，不需要显示服务器。
int main() {
  namespace host = msime::linux_host;
  const host::FloatingSurfaceColors colors{0xffffff, 0x1f1f1f, 0x3584e4, 0xd0d0d0};

  // 不带阴影：表面就是本体，32 px 高，左右内边距 8 px，四角透明，正中是底色或字。
  {
    const auto layout = host::measure_mode_badge(host::ModeBadgeMetrics{}, "");
    assert(layout.height == 32 && layout.plate_height == 32 && layout.margin == 0.0 && !layout.logo);
    assert(layout.width > 2 * 8 && layout.plate_width == layout.width);
    auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, layout.width, layout.height);
    auto *cairo = cairo_create(surface);
    host::paint_mode_badge(cairo, layout, "中", "", colors);
    cairo_destroy(cairo);
    save(surface, "mode-badge-plain.png");
    assert(pixel_at(surface, 0, 0).alpha == 0);
    assert(pixel_at(surface, layout.width - 1, layout.height - 1).alpha == 0);
    // 左侧内边距里、垂直居中处是不透明的底色。
    const auto inset = pixel_at(surface, 4, layout.height / 2);
    assert(inset.alpha == 255 && inset.rgb == colors.surface);
    // 描边在本体边上。
    const auto edge = pixel_at(surface, layout.width / 2, 0);
    assert(edge.alpha > 0 && edge.rgb != colors.surface);
    // 字形取决于容器里装了哪些字体，这里不断言；设置 MSIME_TEST_CANVAS_ARTIFACTS 可以把画出来的图存下来看。
    cairo_surface_destroy(surface);
  }

  // 带阴影：四周各多 8 px，本体下沿外面是半透明的黑色阴影，上方的阴影比下方淡（阴影向下偏 2 px）。
  {
    const auto layout = host::measure_mode_badge(host::ModeBadgeMetrics{1.0, true}, "");
    assert(layout.margin == 8.0 && layout.height == 48 && layout.width == layout.plate_width + 16);
    auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, layout.width, layout.height);
    auto *cairo = cairo_create(surface);
    host::paint_mode_badge(cairo, layout, "英", "", colors);
    cairo_destroy(cairo);
    save(surface, "mode-badge-shadow.png");
    const int middle = layout.width / 2;
    const auto below = pixel_at(surface, middle, 8 + 32 + 2);
    const auto above = pixel_at(surface, middle, 8 - 2);
    assert(below.alpha > 0 && below.alpha < 128 && below.rgb == 0);
    assert(above.alpha < below.alpha);
    assert(pixel_at(surface, 0, 0).alpha == 0);
    assert(pixel_at(surface, 8 + 4, 8 + 16).alpha == 255);

    // 淡出：同一处本体在一半不透明度下 alpha 约为一半，颜色不变；不透明度为 0 时整块透明。
    auto *faded = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, layout.width, layout.height);
    cairo = cairo_create(faded);
    host::paint_mode_badge(cairo, layout, "英", "", colors, 0.5);
    cairo_destroy(cairo);
    save(faded, "mode-badge-fade-half.png");
    const auto half = pixel_at(faded, 8 + 4, 8 + 16);
    assert(half.alpha >= 126 && half.alpha <= 129);
    assert(((half.rgb >> 16) & 0xff) >= 0xfd);
    cairo = cairo_create(faded);
    host::paint_mode_badge(cairo, layout, "英", "", colors, 0.0);
    cairo_destroy(cairo);
    assert(pixel_at(faded, 8 + 4, 8 + 16).alpha == 0);
    cairo_surface_destroy(faded);
    cairo_surface_destroy(surface);
  }
  return 0;
}
