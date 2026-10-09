#pragma once

#include <algorithm>
#include <cmath>
#include <cstring>
#include <optional>
#include <string>

#include <cairo.h>

#include "CandidateFcitxTheme.h"

namespace msime::linux_host {

// 用 classicui 同样的 cairo 编解码 PNG，保持比例缩放到 `width` 宽；超过可用高度 `room` 时继续缩小，`room <= 0` 只约束宽度，解码失败返回 `nullopt`。
inline std::optional<std::string> scale_fcitx_overlay_png(const std::string &png, int width, int room) {
  if (width <= 0) return std::nullopt;
  struct Reader {
    const std::string *bytes;
    std::size_t offset = 0;
  } reader{&png};
  cairo_surface_t *source = cairo_image_surface_create_from_png_stream(
      [](void *closure, unsigned char *data, unsigned int length) {
        auto *in = static_cast<Reader *>(closure);
        if (in->bytes->size() - in->offset < length) return CAIRO_STATUS_READ_ERROR;
        std::memcpy(data, in->bytes->data() + in->offset, length);
        in->offset += length;
        return CAIRO_STATUS_SUCCESS;
      },
      &reader);
  std::optional<std::string> result;
  const int source_width = cairo_surface_status(source) == CAIRO_STATUS_SUCCESS ? cairo_image_surface_get_width(source) : 0;
  const int source_height = source_width > 0 ? cairo_image_surface_get_height(source) : 0;
  if (source_width > 0 && source_height > 0) {
    double factor = static_cast<double>(width) / source_width;
    if (room > 0) factor = std::min(factor, static_cast<double>(room) / source_height);
    const int scaled_width = std::max(1, static_cast<int>(std::lround(source_width * factor)));
    const int height = std::max(1, static_cast<int>(std::lround(source_height * factor)));
    cairo_surface_t *target = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, scaled_width, height);
    cairo_t *cr = cairo_create(target);
    cairo_scale(cr, static_cast<double>(scaled_width) / source_width, static_cast<double>(height) / source_height);
    cairo_set_source_surface(cr, source, 0, 0);
    // A decoration is shrunk several times over; GOOD filters across the source pixels a destination pixel covers instead of sampling a few.
    cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
    cairo_paint(cr);
    cairo_destroy(cr);
    std::string out;
    if (cairo_surface_status(target) == CAIRO_STATUS_SUCCESS &&
        cairo_surface_write_to_png_stream(
            target,
            [](void *closure, const unsigned char *data, unsigned int length) {
              static_cast<std::string *>(closure)->append(reinterpret_cast<const char *>(data), length);
              return CAIRO_STATUS_SUCCESS;
            },
            &out) == CAIRO_STATUS_SUCCESS)
      result = std::move(out);
    cairo_surface_destroy(target);
  }
  cairo_surface_destroy(source);
  return result;
}

}  // namespace msime::linux_host
