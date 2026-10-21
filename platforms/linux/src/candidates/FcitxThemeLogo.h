#pragma once

#include <cstdint>
#include <cstring>
#include <filesystem>
#include <optional>
#include <string>

#include <cairo.h>

#include "CandidateFcitxTheme.h"
#include "../core/LinuxEdition.h"

namespace msime::linux_host {

// 把一张 side×side 的 CAIRO_FORMAT_ARGB32 表面拷成主题画布用的像素；不是这个格式或尺寸时返回 `nullopt`。
inline std::optional<FcitxPixels> fcitx_logo_pixels_from_surface(cairo_surface_t *surface, int side) {
  if (cairo_surface_status(surface) != CAIRO_STATUS_SUCCESS ||
      cairo_image_surface_get_format(surface) != CAIRO_FORMAT_ARGB32 ||
      cairo_image_surface_get_width(surface) != side || cairo_image_surface_get_height(surface) != side)
    return std::nullopt;
  cairo_surface_flush(surface);
  const auto *data = cairo_image_surface_get_data(surface);
  const int stride = cairo_image_surface_get_stride(surface);
  FcitxPixels image{side, side, std::vector<float>(static_cast<std::size_t>(side) * static_cast<std::size_t>(side) * 4u)};
  for (int y = 0; y < side; ++y)
    for (int x = 0; x < side; ++x) {
      // CAIRO_FORMAT_ARGB32 is premultiplied, one native-endian 32-bit word per pixel, as the canvas keeps its own.
      std::uint32_t word;
      std::memcpy(&word, data + static_cast<std::size_t>(y) * static_cast<std::size_t>(stride) + static_cast<std::size_t>(x) * 4u, sizeof word);
      auto *out = &image.rgba[(static_cast<std::size_t>(y) * static_cast<std::size_t>(side) + static_cast<std::size_t>(x)) * 4u];
      out[0] = static_cast<float>((word >> 16) & 0xffu) / 255.0f;
      out[1] = static_cast<float>((word >> 8) & 0xffu) / 255.0f;
      out[2] = static_cast<float>(word & 0xffu) / 255.0f;
      out[3] = static_cast<float>(word >> 24) / 255.0f;
    }
  return image;
}

// The brand mark is the application icon the package installs into the hicolor theme (data/icons), the picture the launcher and the input method list show. The PNGs are decoded with cairo, the library the classic UI itself reads theme images with, so it is on every system that draws this theme.
inline std::optional<FcitxPixels> read_fcitx_logo_pixels(const std::filesystem::path &file, int side) {
  cairo_surface_t *surface = cairo_image_surface_create_from_png(file.c_str());
  auto pixels = fcitx_logo_pixels_from_surface(surface, side);
  cairo_surface_destroy(surface);
  return pixels;
}

// 把任意大小的方形图标缩放成 side×side：cairo 的 GOOD 过滤器在缩小时按面积取样，画出来与图标主题里同尺寸的 PNG 接近。
inline std::optional<FcitxPixels> read_fcitx_logo_pixels_scaled(const std::filesystem::path &file, int side) {
  cairo_surface_t *source = cairo_image_surface_create_from_png(file.c_str());
  std::optional<FcitxPixels> pixels;
  const int width = cairo_image_surface_get_width(source);
  const int height = cairo_image_surface_get_height(source);
  if (cairo_surface_status(source) == CAIRO_STATUS_SUCCESS && width > 0 && height > 0 && side > 0) {
    cairo_surface_t *target = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, side, side);
    cairo_t *cr = cairo_create(target);
    cairo_scale(cr, static_cast<double>(side) / width, static_cast<double>(side) / height);
    cairo_set_source_surface(cr, source, 0, 0);
    cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
    cairo_paint(cr);
    cairo_destroy(cr);
    pixels = fcitx_logo_pixels_from_surface(target, side);
    cairo_surface_destroy(target);
  }
  cairo_surface_destroy(source);
  return pixels;
}

// The mark at 1x and 2x from <icons>/<size>x<size>/apps/msime-linux.png, `icons` being the hicolor directory. Without either size the theme is drawn without the mark, as it was before there was one.
// 图标名按版本取（LinuxEdition.h 的 MSIME_EDITION_ICON）：各版本的图标内容相同，文件名不同，几个版本的包才能同时装。
// `scale` 是 candidate_scale_percent：100% 时直接读 16 和 32 像素的图标；其他大小从不小于目标的最小一档图标（没有就用最大的一档）缩放出来，logo 才和卡片的其余部分一起变大变小。
inline std::optional<FcitxThemeLogo> load_fcitx_theme_logo(const std::filesystem::path &icons, double scale = 1.0) {
  FcitxCandidateStyle style;
  style.scale = scale;
  const int side = fcitx_panel_metrics(style).logo_side;
  const auto file = [&](int size) {
    const auto name = std::to_string(size) + "x" + std::to_string(size);
    return icons / name / "apps" / (MSIME_EDITION_ICON ".png");
  };
  if (side == FcitxPanelGeometry::logo_side) {
    auto one = read_fcitx_logo_pixels(file(side), side);
    auto two = read_fcitx_logo_pixels(file(2 * side), 2 * side);
    if (!one || !two) return std::nullopt;
    return FcitxThemeLogo{std::move(*one), std::move(*two)};
  }
  const auto source = [&](int wanted) {
    std::filesystem::path largest;
    for (const int size : {16, 32, 48, 64, 128, 256}) {
      std::error_code error;
      if (!std::filesystem::is_regular_file(file(size), error)) continue;
      largest = file(size);
      if (size >= wanted) return largest;
    }
    return largest;
  };
  auto one = read_fcitx_logo_pixels_scaled(source(side), side);
  auto two = read_fcitx_logo_pixels_scaled(source(2 * side), 2 * side);
  if (!one || !two) return std::nullopt;
  return FcitxThemeLogo{std::move(*one), std::move(*two)};
}

}  // namespace msime::linux_host
