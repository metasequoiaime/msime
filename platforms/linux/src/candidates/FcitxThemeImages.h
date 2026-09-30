#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace msime::linux_host {

// The Fcitx5 classic UI draws a panel either as a flat colour rectangle or from a nine-slice image in the theme directory; rounded corners and a drop shadow are only possible through the image. This header draws those images and encodes them as PNG, which cairo reads natively on every Fcitx5 release, without a new dependency: the PNG is written with stored (uncompressed) deflate blocks, which every inflater accepts, and the images are a few kilobytes each. Geometry is given in logical pixels and drawn at an integer scale, so the theme can ship an @2x copy for Fcitx5 releases that load one.

// A bitmap drawn onto a canvas as it is: premultiplied RGBA in [0, 1], row by row, `width` by `height` device pixels.
struct FcitxPixels {
  int width = 0;
  int height = 0;
  std::vector<float> rgba;
};

// A canvas of premultiplied RGBA in [0, 1], composited source-over.
class FcitxCanvas {
public:
  FcitxCanvas(int width, int height, int scale)
      : width_(width * scale), height_(height * scale), scale_(scale),
        pixels_(static_cast<std::size_t>(width_) * static_cast<std::size_t>(height_) * 4u, 0.0f) {}

  int width() const { return width_; }
  int height() const { return height_; }
  int scale() const { return scale_; }

  // Source-over of an opaque colour at `alpha` onto one device pixel.
  void blend(int x, int y, std::uint32_t rgb, float alpha) {
    if (alpha <= 0.0f || x < 0 || y < 0 || x >= width_ || y >= height_) return;
    alpha = std::min(alpha, 1.0f);
    auto *pixel = &pixels_[(static_cast<std::size_t>(y) * static_cast<std::size_t>(width_) + static_cast<std::size_t>(x)) * 4u];
    const float source[3] = {static_cast<float>((rgb >> 16) & 0xffu) / 255.0f,
                             static_cast<float>((rgb >> 8) & 0xffu) / 255.0f,
                             static_cast<float>(rgb & 0xffu) / 255.0f};
    for (int channel = 0; channel < 3; ++channel) pixel[channel] = source[channel] * alpha + pixel[channel] * (1.0f - alpha);
    pixel[3] = alpha + pixel[3] * (1.0f - alpha);
  }

  // Source-over of a premultiplied RGBA image, one image pixel per device pixel, with its top-left corner at a logical position.
  void draw(const FcitxPixels &image, double left, double top) {
    const int origin_x = static_cast<int>(std::lround(left * scale_));
    const int origin_y = static_cast<int>(std::lround(top * scale_));
    for (int y = 0; y < image.height; ++y)
      for (int x = 0; x < image.width; ++x) {
        const int target_x = origin_x + x;
        const int target_y = origin_y + y;
        if (target_x < 0 || target_y < 0 || target_x >= width_ || target_y >= height_) continue;
        const auto *source = &image.rgba[(static_cast<std::size_t>(y) * static_cast<std::size_t>(image.width) + static_cast<std::size_t>(x)) * 4u];
        auto *pixel = &pixels_[(static_cast<std::size_t>(target_y) * static_cast<std::size_t>(width_) + static_cast<std::size_t>(target_x)) * 4u];
        const float keep = 1.0f - source[3];
        for (int channel = 0; channel < 4; ++channel) pixel[channel] = source[channel] + pixel[channel] * keep;
      }
  }

  // Make every pixel above `top` (logical pixels) fully transparent, whatever was drawn there.
  void clear_above(int top) {
    const auto rows = static_cast<std::size_t>(std::clamp(top * scale_, 0, height_));
    std::fill(pixels_.begin(), pixels_.begin() + static_cast<std::ptrdiff_t>(rows * static_cast<std::size_t>(width_) * 4u), 0.0f);
  }

  // Straight (non-premultiplied) 8-bit RGBA rows, as PNG colour type 6 stores them.
  std::vector<std::uint8_t> straight_rgba() const {
    std::vector<std::uint8_t> out(pixels_.size());
    for (std::size_t index = 0; index < pixels_.size(); index += 4) {
      const float alpha = pixels_[index + 3];
      for (std::size_t channel = 0; channel < 3; ++channel) {
        const float value = alpha > 0.0f ? pixels_[index + channel] / alpha : 0.0f;
        out[index + channel] = static_cast<std::uint8_t>(std::lround(std::clamp(value, 0.0f, 1.0f) * 255.0f));
      }
      out[index + 3] = static_cast<std::uint8_t>(std::lround(std::clamp(alpha, 0.0f, 1.0f) * 255.0f));
    }
    return out;
  }

  // Alpha of one device pixel, for tests.
  float alpha_at(int x, int y) const {
    return pixels_[(static_cast<std::size_t>(y) * static_cast<std::size_t>(width_) + static_cast<std::size_t>(x)) * 4u + 3u];
  }

private:
  int width_;
  int height_;
  int scale_;
  std::vector<float> pixels_;
};

// A rectangle in logical pixels.
struct FcitxRect {
  double left;
  double top;
  double right;
  double bottom;
};

// Signed distance from a point to a rounded rectangle: negative inside, in the point's units.
inline double fcitx_rounded_rect_distance(double x, double y, const FcitxRect &rect, double radius) {
  const double half_width = (rect.right - rect.left) / 2.0;
  const double half_height = (rect.bottom - rect.top) / 2.0;
  radius = std::clamp(radius, 0.0, std::min(half_width, half_height));
  const double qx = std::abs(x - (rect.left + half_width)) - (half_width - radius);
  const double qy = std::abs(y - (rect.top + half_height)) - (half_height - radius);
  const double outside = std::hypot(std::max(qx, 0.0), std::max(qy, 0.0));
  return outside + std::min(std::max(qx, qy), 0.0) - radius;
}

// Visit every device pixel with its centre in logical coordinates and the device-to-logical factor.
template <typename Visit>
void fcitx_each_pixel(FcitxCanvas &canvas, Visit visit) {
  const double unit = 1.0 / canvas.scale();
  for (int y = 0; y < canvas.height(); ++y)
    for (int x = 0; x < canvas.width(); ++x) visit(x, y, (x + 0.5) * unit, (y + 0.5) * unit, unit);
}

// Anti-aliased coverage of a rounded rectangle: the pixel's distance to the edge, in device pixels, turned into a one-pixel ramp.
inline float fcitx_coverage(double distance, double unit) {
  return static_cast<float>(std::clamp(0.5 - distance / unit, 0.0, 1.0));
}

inline void fcitx_fill_rounded(FcitxCanvas &canvas, const FcitxRect &rect, double radius, std::uint32_t rgb,
                               float alpha = 1.0f) {
  fcitx_each_pixel(canvas, [&](int x, int y, double px, double py, double unit) {
    canvas.blend(x, y, rgb, alpha * fcitx_coverage(fcitx_rounded_rect_distance(px, py, rect, radius), unit));
  });
}

// A border `width` wide drawn inside the rectangle's edge, the way a CSS border sits inside its box.
inline void fcitx_stroke_rounded(FcitxCanvas &canvas, const FcitxRect &rect, double radius, double width,
                                 std::uint32_t rgb, float alpha = 1.0f) {
  if (width <= 0.0) return;
  const FcitxRect inner{rect.left + width, rect.top + width, rect.right - width, rect.bottom - width};
  const double inner_radius = std::max(0.0, radius - width);
  const bool hollow = inner.right > inner.left && inner.bottom > inner.top;
  fcitx_each_pixel(canvas, [&](int x, int y, double px, double py, double unit) {
    const float outer = fcitx_coverage(fcitx_rounded_rect_distance(px, py, rect, radius), unit);
    const float hole = hollow ? fcitx_coverage(fcitx_rounded_rect_distance(px, py, inner, inner_radius), unit) : 0.0f;
    canvas.blend(x, y, rgb, alpha * std::max(0.0f, outer - hole));
  });
}

// A soft black shadow of a rounded rectangle: the Gaussian falloff of a blurred straight edge, taken along the distance to the rounded outline. It matches a true blur along the sides and is close to it at the corners.
inline void fcitx_drop_shadow(FcitxCanvas &canvas, const FcitxRect &rect, double radius, double sigma, float alpha) {
  const double spread = sigma * std::sqrt(2.0);
  fcitx_each_pixel(canvas, [&](int x, int y, double px, double py, double) {
    const double distance = fcitx_rounded_rect_distance(px, py, rect, radius);
    canvas.blend(x, y, 0x000000u, alpha * static_cast<float>(0.5 * std::erfc(distance / spread)));
  });
}

// An open polyline stroked with round joins and caps.
inline void fcitx_stroke_polyline(FcitxCanvas &canvas, const std::vector<std::pair<double, double>> &points,
                                  double width, std::uint32_t rgb) {
  const double half = width / 2.0;
  fcitx_each_pixel(canvas, [&](int x, int y, double px, double py, double unit) {
    double nearest = 1e9;
    for (std::size_t index = 1; index < points.size(); ++index) {
      const auto [ax, ay] = points[index - 1];
      const auto [bx, by] = points[index];
      const double dx = bx - ax;
      const double dy = by - ay;
      const double length = dx * dx + dy * dy;
      const double t = length > 0.0 ? std::clamp(((px - ax) * dx + (py - ay) * dy) / length, 0.0, 1.0) : 0.0;
      nearest = std::min(nearest, std::hypot(px - (ax + t * dx), py - (ay + t * dy)));
    }
    canvas.blend(x, y, rgb, fcitx_coverage(nearest - half, unit));
  });
}

inline std::uint32_t fcitx_png_crc(const std::string &bytes, std::size_t begin) {
  std::uint32_t crc = 0xffffffffu;
  for (std::size_t index = begin; index < bytes.size(); ++index) {
    crc ^= static_cast<unsigned char>(bytes[index]);
    for (int bit = 0; bit < 8; ++bit) crc = (crc >> 1) ^ (0xedb88320u & (0u - (crc & 1u)));
  }
  return crc ^ 0xffffffffu;
}

inline void fcitx_png_u32(std::string &out, std::uint32_t value) {
  for (const int shift : {24, 16, 8, 0}) out.push_back(static_cast<char>((value >> shift) & 0xffu));
}

inline void fcitx_png_chunk(std::string &out, const char *type, const std::string &data) {
  fcitx_png_u32(out, static_cast<std::uint32_t>(data.size()));
  const auto start = out.size();
  out.append(type, 4);
  out += data;
  fcitx_png_u32(out, fcitx_png_crc(out, start));
}

// Encode straight RGBA rows as an 8-bit colour-type-6 PNG whose zlib stream uses stored blocks.
inline std::string fcitx_png_encode(int width, int height, const std::vector<std::uint8_t> &rgba) {
  std::string raw;
  const auto row = static_cast<std::size_t>(width) * 4u;
  raw.reserve((row + 1u) * static_cast<std::size_t>(height));
  for (int y = 0; y < height; ++y) {
    raw.push_back('\0');  // filter type None
    raw.append(reinterpret_cast<const char *>(rgba.data()) + static_cast<std::size_t>(y) * row, row);
  }
  std::string zlib("\x78\x01", 2);
  std::size_t offset = 0;
  do {
    const auto length = std::min<std::size_t>(raw.size() - offset, 65535u);
    const bool last = offset + length == raw.size();
    zlib.push_back(last ? '\x01' : '\x00');
    zlib.push_back(static_cast<char>(length & 0xffu));
    zlib.push_back(static_cast<char>(length >> 8));
    zlib.push_back(static_cast<char>(~length & 0xffu));
    zlib.push_back(static_cast<char>((~length >> 8) & 0xffu));
    zlib.append(raw, offset, length);
    offset += length;
  } while (offset < raw.size());
  std::uint32_t a = 1;
  std::uint32_t b = 0;
  for (const char value : raw) {
    a = (a + static_cast<unsigned char>(value)) % 65521u;
    b = (b + a) % 65521u;
  }
  fcitx_png_u32(zlib, (b << 16) | a);

  std::string png("\x89PNG\r\n\x1a\n", 8);
  std::string header;
  fcitx_png_u32(header, static_cast<std::uint32_t>(width));
  fcitx_png_u32(header, static_cast<std::uint32_t>(height));
  header += std::string("\x08\x06\x00\x00\x00", 5);  // 8-bit RGBA, deflate, adaptive filtering, no interlace
  fcitx_png_chunk(png, "IHDR", header);
  fcitx_png_chunk(png, "IDAT", zlib);
  fcitx_png_chunk(png, "IEND", {});
  return png;
}

inline std::string fcitx_png_encode(const FcitxCanvas &canvas) {
  return fcitx_png_encode(canvas.width(), canvas.height(), canvas.straight_rgba());
}

}  // namespace msime::linux_host
