#pragma once

#include <algorithm>
#include <array>
#include <memory>
#include <vector>
#include <cstring>

#include <cairo.h>
#include "msime_client.h"

#include "CandidateFcitxTheme.h"

namespace msime::linux_host {

// 仅目录顶层的新字段指向共享扫描根；旧条目的严格配色协议保持原样。
inline std::optional<std::filesystem::path> fcitx_canvas_directory(const nlohmann::json &catalog) {
  if (!catalog.is_object()) return std::nullopt;
  const auto value = catalog.find("directory");
  if (value == catalog.end() || !value->is_string()) return std::nullopt;
  const auto text = value->get<std::string>();
  if (text.empty() || text.size() > 4096 || text.find('\0') != std::string::npos || text.front() != '/')
    return std::nullopt;
  return std::filesystem::path(text);
}

inline std::string fcitx_canvas_file_stamp(const std::filesystem::path &path) {
  struct stat metadata {};
  if (::stat(path.c_str(), &metadata) != 0) return path.string() + ":missing";
  std::ostringstream stamp;
  stamp << path.string() << ':' << metadata.st_dev << ':' << metadata.st_ino << ':' << metadata.st_size << ':';
#ifdef __APPLE__
  stamp << metadata.st_mtimespec.tv_sec << ':' << metadata.st_mtimespec.tv_nsec;
#else
  stamp << metadata.st_mtim.tv_sec << ':' << metadata.st_mtim.tv_nsec;
#endif
  return stamp.str();
}

// 包 API 的相对路径仍按不可信输入复核；不跟随包内任一级符号链接。
inline std::optional<std::filesystem::path> fcitx_canvas_resource(const std::filesystem::path &directory,
                                                                std::string_view id, const nlohmann::json &name) {
  if (!safe_skin_id(id) || !directory.is_absolute() || !name.is_string()) return std::nullopt;
  const auto text = name.get<std::string>();
  if (text.empty() || text.size() > 256 || text.find('\0') != std::string::npos || text.find('\\') != std::string::npos)
    return std::nullopt;
  const std::filesystem::path relative(text);
  if (relative.is_absolute()) return std::nullopt;
  auto path = directory;
  std::error_code error;
  for (const auto &part : std::filesystem::path(id) / relative) {
    if (part == "." || part == ".." || part.empty()) return std::nullopt;
    path /= part;
    if (std::filesystem::is_symlink(std::filesystem::symlink_status(path, error)) || error) return std::nullopt;
  }
  return path;
}

inline std::optional<std::array<double, 4>> fcitx_canvas_box(const nlohmann::json &value) {
  if (!value.is_array() || value.empty() || value.size() > 4) return std::nullopt;
  std::array<double, 4> values{};
  for (std::size_t at = 0; at < value.size(); ++at) {
    if (!value[at].is_number()) return std::nullopt;
    values[at] = value[at].get<double>();
    if (!std::isfinite(values[at]) || values[at] < 0 || values[at] > 500) return std::nullopt;
  }
  if (value.size() == 1) return std::array<double, 4>{values[0], values[0], values[0], values[0]};
  if (value.size() == 2) return std::array<double, 4>{values[0], values[1], values[0], values[1]};
  if (value.size() == 3) values[3] = values[1];
  return values;
}

inline std::optional<std::array<int, 4>> fcitx_canvas_insets(const nlohmann::json &value) {
  if (!value.is_object()) return std::nullopt;
  constexpr const char *keys[] = {"top", "right", "bottom", "left"};
  std::array<int, 4> result{};
  for (std::size_t at = 0; at < result.size(); ++at) {
    const auto field = value.find(keys[at]);
    if (field == value.end() || !field->is_number()) return std::nullopt;
    const auto number = field->get<double>();
    if (!std::isfinite(number) || number < 0 || number > 500) return std::nullopt;
    result[at] = static_cast<int>(std::ceil(number));
  }
  return result;
}

inline std::optional<std::string> read_fcitx_canvas_image(const std::filesystem::path &path) {
  const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return std::nullopt;
  struct Close { int fd; ~Close() { ::close(fd); } } close{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) || metadata.st_nlink != 1 ||
      metadata.st_size <= 0 || static_cast<std::uintmax_t>(metadata.st_size) > kFcitxOverlayMaxBytes)
    return std::nullopt;
  std::string bytes;
  std::array<char, 64 * 1024> buffer{};
  for (;;) {
    const auto count = ::read(descriptor, buffer.data(), buffer.size());
    if (count == 0) break;
    if (count < 0) {
      if (errno == EINTR) continue;
      return std::nullopt;
    }
    if (bytes.size() > kFcitxOverlayMaxBytes - static_cast<std::size_t>(count)) return std::nullopt;
    bytes.append(buffer.data(), static_cast<std::size_t>(count));
  }
  return bytes.empty() ? std::nullopt : std::optional<std::string>(std::move(bytes));
}

// 宿主复制借用的 RGBA 为 Cairo 原生字节序的预乘像素。
inline cairo_surface_t *copy_fcitx_canvas_rgba(const std::uint8_t *rgba, std::size_t length,
                                             std::uint32_t width, std::uint32_t height) {
  if (!rgba || width == 0 || height == 0 || width > 2048 || height > 2048 ||
      length != static_cast<std::size_t>(width) * height * 4) return nullptr;
  auto *image = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, static_cast<int>(width), static_cast<int>(height));
  if (cairo_surface_status(image) != CAIRO_STATUS_SUCCESS) { cairo_surface_destroy(image); return nullptr; }
  auto *data = cairo_image_surface_get_data(image);
  const auto stride = cairo_image_surface_get_stride(image);
  for (std::uint32_t y = 0; y < height; ++y) for (std::uint32_t x = 0; x < width; ++x) {
    const auto *pixel = rgba + (static_cast<std::size_t>(y) * width + x) * 4;
    const auto alpha = static_cast<std::uint32_t>(pixel[3]);
    const auto premultiply = [alpha](std::uint8_t channel) { return (channel * alpha + 127) / 255; };
    const std::uint32_t word = alpha << 24 | premultiply(pixel[0]) << 16 |
                               premultiply(pixel[1]) << 8 | premultiply(pixel[2]);
    std::memcpy(data + y * stride + x * 4, &word, 4);
  }
  cairo_surface_mark_dirty(image);
  return image;
}

// 回调只借用共享解码器的 RGBA；当场复制为 Cairo 的本机字节序、预乘 ARGB32。
inline cairo_surface_t *decode_fcitx_canvas_image(const std::string &bytes) {
  cairo_surface_t *surface = nullptr;
  const auto received = msime_client_skin_image_decode(
      reinterpret_cast<const std::uint8_t *>(bytes.data()), bytes.size(),
      [](void *context, const std::uint8_t *rgba, std::size_t length, std::uint32_t width, std::uint32_t height) -> std::int32_t {
        auto *image = copy_fcitx_canvas_rgba(rgba, length, width, height);
        if (!image) return 0;
        *static_cast<cairo_surface_t **>(context) = image;
        return 1;
      }, &surface);
  if (received != 1) {
    if (surface) cairo_surface_destroy(surface);
    return nullptr;
  }
  return surface;
}

// canvas 包的自由挂件（`decorations[]`）在 classicui 面板图里的位置：按与原生画布相同的锚点规则（native_canvas_decoration_box）放在最小尺寸的面板图上，`card` 是面板图里卡片（九宫格内框）的矩形。classicui 会把面板图按九宫格拉伸，只有整个落在某个角的切片里（左或右边距列、上或下边距行）的挂件在拉伸后仍保持原样，其余的会被拉变形，所以不放进来，返回空。
inline std::optional<std::array<double, 4>> fcitx_canvas_widget_box(const nlohmann::json &item, std::array<double, 4> card,
                                                                     double width, double height, const std::array<int, 4> &margin) {
  if (!item.is_object() || !item.contains("widthDip") || !item.contains("heightDip") ||
      !item["widthDip"].is_number() || !item["heightDip"].is_number()) return std::nullopt;
  const double w = item["widthDip"].get<double>(), h = item["heightDip"].get<double>();
  if (!(w > 0) || !(h > 0)) return std::nullopt;
  const auto anchor = item.value("anchor", std::string("right"));
  const double x = anchor.find("left") != std::string::npos ? card[0]
      : anchor.find("right") != std::string::npos ? card[0] + card[2] - w : card[0] + (card[2] - w) / 2;
  const double y = anchor.find("top") != std::string::npos ? card[1]
      : anchor.find("bottom") != std::string::npos ? card[1] + card[3] - h : card[1] + (card[3] - h) / 2;
  const std::array<double, 4> box{x + item.value("offsetXDip", 0.0), y + item.value("offsetYDip", 0.0), w, h};
  const bool left = box[0] >= 0 && box[0] + w <= margin[3];
  const bool right = box[0] >= width - margin[1] && box[0] + w <= width;
  const bool top = box[1] >= 0 && box[1] + h <= margin[0];
  const bool bottom = box[1] >= height - margin[2] && box[1] + h <= height;
  if (!(left || right) || !(top || bottom)) return std::nullopt;
  return box;
}

// classicui 的源切片与显示边宽共用 Margin；先分别缩放九块，使两者不同的清单仍保留边框比例。
// 这里只准备静态画布，动画能力不能由生成了贴图推断：动图（面板和挂件）都只取第一帧。Wayland 上输入法不能自己摆窗口，classicui 是唯一的画法，所以落在角上的自由挂件按 zIndex 画进面板图（z < 1 在面板下，其余在面板上），别处的挂件和帧动画只有 X11 的原生画布画得出来。
inline std::optional<FcitxCanvasPanel> prepare_fcitx_canvas_panel(const nlohmann::json &summary,
                                                                const std::filesystem::path &directory,
                                                                std::string_view id, bool dark,
                                                                std::filesystem::path *selected_source = nullptr) {
  if (!summary.is_object() || !summary.contains("id") || summary["id"] != id ||
      !summary.contains("canvas") || summary["canvas"] != true ||
      !summary.contains("schemaVersion") || summary["schemaVersion"] != 2)
    return std::nullopt;
  const auto border = summary.find("borderImage");
  if (border == summary.end() || !border->is_object()) return std::nullopt;
  const auto source = border->find(!dark && border->contains("lightSource") ? "lightSource" : "source");
  if (source == border->end()) return std::nullopt;
  const auto path = fcitx_canvas_resource(directory, id, *source);
  if (!path) return std::nullopt;
  if (selected_source) *selected_source = *path;
  const auto bytes = read_fcitx_canvas_image(*path);
  if (!bytes) return std::nullopt;
  const auto slice = border->find(border->contains("stretchDip") ? "stretchDip" : "sliceDip");
  if (slice == border->end()) return std::nullopt;
  const auto slices = fcitx_canvas_box(*slice);
  if (!slices) return std::nullopt;
  const auto widths = border->contains("widthDip") ? fcitx_canvas_box((*border)["widthDip"]) : slices;
  if (!widths || (border->contains("fill") && !(*border)["fill"].is_boolean())) return std::nullopt;
  const bool fill = border->value("fill", true);
  FcitxCanvasPanel panel;
  std::array<int, 4> edge{};
  for (std::size_t at = 0; at < edge.size(); ++at) edge[at] = static_cast<int>(std::ceil((*widths)[at]));
  // 没写 content_insets_dip 的 canvas 包仍在贴图边宽内留 7 px（原先 Linux 卡片的 1 px 发丝线加 6 px 内边距）：贴图是包自己的外观，不跟着原生卡片改成 macOS 的 4 px。
  constexpr int kCanvasContentPadding = 7;
  for (std::size_t at = 0; at < edge.size(); ++at)
    panel.content[at] = edge[at] + kCanvasContentPadding;
  if (summary.contains("contentInsetsDip")) {
    const auto insets = fcitx_canvas_insets(summary["contentInsetsDip"]);
    if (!insets) return std::nullopt;
    for (std::size_t at = 0; at < edge.size(); ++at) panel.content[at] = std::max(edge[at], (*insets)[at]);
  }
  if (summary.contains("outerInsetsDip")) {
    const auto insets = fcitx_canvas_insets(summary["outerInsetsDip"]);
    if (!insets) return std::nullopt;
    panel.outer = *insets;
  }
  for (std::size_t at = 0; at < edge.size(); ++at) panel.margin[at] = edge[at] + panel.outer[at];
  using Surface = std::unique_ptr<cairo_surface_t, decltype(&cairo_surface_destroy)>;
  Surface image(decode_fcitx_canvas_image(*bytes), cairo_surface_destroy);
  if (!image || cairo_surface_status(image.get()) != CAIRO_STATUS_SUCCESS) return std::nullopt;
  const auto axis = [](double length, double before, double after) {
    const auto factor = before + after > length ? length / (before + after) : 1.0;
    return std::array<double, 4>{0, before * factor, length - after * factor, length};
  };
  const auto x = axis(cairo_image_surface_get_width(image.get()), (*slices)[3], (*slices)[1]);
  const auto y = axis(cairo_image_surface_get_height(image.get()), (*slices)[0], (*slices)[2]);
  const int width = panel.margin[3] + std::max(1, static_cast<int>(std::ceil(x[2] - x[1]))) + panel.margin[1];
  const int height = panel.margin[0] + std::max(1, static_cast<int>(std::ceil(y[2] - y[1]))) + panel.margin[2];
  if (width > 2048 || height > 2048) return std::nullopt;
  const double dx[] = {static_cast<double>(panel.outer[3]), static_cast<double>(panel.margin[3]),
                       static_cast<double>(width - panel.margin[1]), static_cast<double>(width - panel.outer[1])};
  const double dy[] = {static_cast<double>(panel.outer[0]), static_cast<double>(panel.margin[0]),
                       static_cast<double>(height - panel.margin[2]), static_cast<double>(height - panel.outer[2])};
  // 落在角上的挂件，按 zIndex 排好；读不出或解不开的挂件只是少画，面板照样生成。
  struct Widget { std::array<double, 4> box; int z; std::shared_ptr<cairo_surface_t> image; };
  std::vector<Widget> widgets;
  const std::array<double, 4> card{static_cast<double>(panel.outer[3]), static_cast<double>(panel.outer[0]),
      static_cast<double>(width - panel.outer[3] - panel.outer[1]), static_cast<double>(height - panel.outer[0] - panel.outer[2])};
  for (const auto &item : summary.value("decorations", nlohmann::json::array())) {
    const auto box = fcitx_canvas_widget_box(item, card, width, height, panel.margin);
    if (!box || !item.contains("image")) continue;
    const auto widget_path = fcitx_canvas_resource(directory, id, item["image"]);
    const auto widget_bytes = widget_path ? read_fcitx_canvas_image(*widget_path) : std::nullopt;
    std::shared_ptr<cairo_surface_t> widget(widget_bytes ? decode_fcitx_canvas_image(*widget_bytes) : nullptr, cairo_surface_destroy);
    if (!widget || cairo_surface_status(widget.get()) != CAIRO_STATUS_SUCCESS) continue;
    widgets.push_back({*box, item.value("zIndex", 2), std::move(widget)});
  }
  std::stable_sort(widgets.begin(), widgets.end(), [](const auto &left, const auto &right) { return left.z < right.z; });
  const auto paint_widgets = [&](cairo_t *cr, bool front) {
    for (const auto &widget : widgets) {
      if ((widget.z >= 1) != front) continue;
      const double image_width = cairo_image_surface_get_width(widget.image.get());
      const double image_height = cairo_image_surface_get_height(widget.image.get());
      if (image_width <= 0 || image_height <= 0) continue;
      // 与原生画布相同：等比放进挂件框并居中。
      const double ratio = std::min(widget.box[2] / image_width, widget.box[3] / image_height);
      cairo_save(cr);
      cairo_translate(cr, widget.box[0] + (widget.box[2] - image_width * ratio) / 2, widget.box[1] + (widget.box[3] - image_height * ratio) / 2);
      cairo_scale(cr, ratio, ratio);
      cairo_set_source_surface(cr, widget.image.get(), 0, 0);
      cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
      cairo_paint(cr);
      cairo_restore(cr);
    }
  };
  std::array<std::string, 2> rendered;
  for (int scale = 1; scale <= 2; ++scale) {
    Surface target(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width * scale, height * scale), cairo_surface_destroy);
    std::unique_ptr<cairo_t, decltype(&cairo_destroy)> cr(cairo_create(target.get()), cairo_destroy);
    cairo_scale(cr.get(), scale, scale);
    paint_widgets(cr.get(), false);
    for (int row = 0; row < 3; ++row) for (int col = 0; col < 3; ++col) {
      if (!fill && row == 1 && col == 1) continue;
      const double sw = x[col + 1] - x[col], sh = y[row + 1] - y[row];
      const double dw = dx[col + 1] - dx[col], dh = dy[row + 1] - dy[row];
      if (sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0) continue;
      Surface tile(cairo_surface_create_for_rectangle(image.get(), x[col], y[row], sw, sh), cairo_surface_destroy);
      cairo_save(cr.get());
      cairo_rectangle(cr.get(), dx[col], dy[row], dw, dh);
      cairo_clip(cr.get());
      cairo_translate(cr.get(), dx[col], dy[row]);
      cairo_scale(cr.get(), dw / sw, dh / sh);
      cairo_set_source_surface(cr.get(), tile.get(), 0, 0);
      cairo_pattern_set_extend(cairo_get_source(cr.get()), CAIRO_EXTEND_PAD);
      cairo_paint(cr.get());
      cairo_restore(cr.get());
    }
    paint_widgets(cr.get(), true);
    if (cairo_status(cr.get()) != CAIRO_STATUS_SUCCESS || cairo_surface_status(target.get()) != CAIRO_STATUS_SUCCESS)
      return std::nullopt;
    if (cairo_surface_write_to_png_stream(target.get(), [](void *closure, const unsigned char *data, unsigned int length) {
      static_cast<std::string *>(closure)->append(reinterpret_cast<const char *>(data), length);
      return CAIRO_STATUS_SUCCESS;
    }, &rendered[scale - 1]) != CAIRO_STATUS_SUCCESS) return std::nullopt;
  }
  panel.file = std::string(kFcitxCanvasPrefix) + fcitx_content_hash(rendered[0] + rendered[1]) + ".png";
  panel.images = {{panel.file, std::move(rendered[0])}, {fcitx_overlay_2x_name(panel.file), std::move(rendered[1])}};
  return panel;
}

// 皮肤背景图画成主题的叠图（见 CandidateFcitxTheme.h 的 FcitxBackgroundRenderer）：用共享层的解码器读包里的图片（PNG、JPEG、GIF、WebP 都行，动图取第一帧），按 fit 放进 width×height 的框里，cover 铺满后居中裁切、contain 完整放进并居中、stretch 两个方向各自拉伸，再乘上不透明度，各画一张 1x 和 2x 的 PNG。读不出、解不开或画不出时返回 `nullopt`，主题就不带背景图。
inline std::optional<std::pair<std::string, std::string>> render_fcitx_background(const CandidateSkinBackground &background,
                                                                                 int width, int height, double opacity) {
  if (width <= 0 || height <= 0 || width > 2048 || height > 2048) return std::nullopt;
  const auto bytes = read_fcitx_canvas_image(background.image);
  if (!bytes) return std::nullopt;
  using Surface = std::unique_ptr<cairo_surface_t, decltype(&cairo_surface_destroy)>;
  Surface image(decode_fcitx_canvas_image(*bytes), cairo_surface_destroy);
  if (!image || cairo_surface_status(image.get()) != CAIRO_STATUS_SUCCESS) return std::nullopt;
  const double source_width = cairo_image_surface_get_width(image.get());
  const double source_height = cairo_image_surface_get_height(image.get());
  if (source_width <= 0 || source_height <= 0) return std::nullopt;
  double scale_x = width / source_width;
  double scale_y = height / source_height;
  if (background.fit == CandidateSkinBackgroundFit::cover) scale_x = scale_y = std::max(scale_x, scale_y);
  else if (background.fit == CandidateSkinBackgroundFit::contain) scale_x = scale_y = std::min(scale_x, scale_y);
  std::array<std::string, 2> rendered;
  for (int scale = 1; scale <= 2; ++scale) {
    Surface target(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width * scale, height * scale), cairo_surface_destroy);
    std::unique_ptr<cairo_t, decltype(&cairo_destroy)> cr(cairo_create(target.get()), cairo_destroy);
    cairo_scale(cr.get(), scale, scale);
    cairo_translate(cr.get(), (width - source_width * scale_x) / 2, (height - source_height * scale_y) / 2);
    cairo_scale(cr.get(), scale_x, scale_y);
    // 只画图片本身的范围；PAD 让放大时边缘像素一直延伸到范围边上，边缘不会被插值成半透明。
    cairo_rectangle(cr.get(), 0, 0, source_width, source_height);
    cairo_clip(cr.get());
    cairo_set_source_surface(cr.get(), image.get(), 0, 0);
    cairo_pattern_set_filter(cairo_get_source(cr.get()), CAIRO_FILTER_GOOD);
    cairo_pattern_set_extend(cairo_get_source(cr.get()), CAIRO_EXTEND_PAD);
    cairo_paint_with_alpha(cr.get(), std::clamp(opacity, 0.0, 1.0));
    if (cairo_status(cr.get()) != CAIRO_STATUS_SUCCESS || cairo_surface_status(target.get()) != CAIRO_STATUS_SUCCESS)
      return std::nullopt;
    if (cairo_surface_write_to_png_stream(target.get(), [](void *closure, const unsigned char *data, unsigned int length) {
      static_cast<std::string *>(closure)->append(reinterpret_cast<const char *>(data), length);
      return CAIRO_STATUS_SUCCESS;
    }, &rendered[scale - 1]) != CAIRO_STATUS_SUCCESS) return std::nullopt;
  }
  return std::pair{std::move(rendered[0]), std::move(rendered[1])};
}

} // namespace msime::linux_host
