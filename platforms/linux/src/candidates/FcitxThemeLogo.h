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

// The brand mark is the application icon the package installs into the hicolor theme (data/icons), the picture the launcher and the input method list show. The PNGs are decoded with cairo, the library the classic UI itself reads theme images with, so it is on every system that draws this theme.
inline std::optional<FcitxPixels> read_fcitx_logo_pixels(const std::filesystem::path &file, int side) {
  cairo_surface_t *surface = cairo_image_surface_create_from_png(file.c_str());
  std::optional<FcitxPixels> pixels;
  if (cairo_surface_status(surface) == CAIRO_STATUS_SUCCESS &&
      cairo_image_surface_get_format(surface) == CAIRO_FORMAT_ARGB32 &&
      cairo_image_surface_get_width(surface) == side && cairo_image_surface_get_height(surface) == side) {
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
    pixels = std::move(image);
  }
  cairo_surface_destroy(surface);
  return pixels;
}

// The mark at 1x and 2x from <icons>/<size>x<size>/apps/msime-linux.png, `icons` being the hicolor directory. Without either size the theme is drawn without the mark, as it was before there was one.
// 图标名按版本取（LinuxEdition.h 的 MSIME_EDITION_ICON）：各版本的图标内容相同，文件名不同，几个版本的包才能同时装。
inline std::optional<FcitxThemeLogo> load_fcitx_theme_logo(const std::filesystem::path &icons) {
  constexpr int side = FcitxPanelGeometry::logo_side;
  const auto file = [&](int size) {
    const auto name = std::to_string(size) + "x" + std::to_string(size);
    return icons / name / "apps" / (MSIME_EDITION_ICON ".png");
  };
  auto one = read_fcitx_logo_pixels(file(side), side);
  auto two = read_fcitx_logo_pixels(file(2 * side), 2 * side);
  if (!one || !two) return std::nullopt;
  return FcitxThemeLogo{std::move(*one), std::move(*two)};
}

}  // namespace msime::linux_host
