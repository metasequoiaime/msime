#pragma once

#include "WaveOverlayModel.h"

#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace msime::linux_host {

// 浮层一次呈现的内容，单位是逻辑像素：布局及其尺寸、是否画操作按钮和状态标签、标签文字，以及放得下的那部分转写文字。
struct WaveOverlayFrame {
  WaveOverlayLayout layout = WaveOverlayLayout::Compact;
  int width = 0;
  int height = 0;
  bool actions = false;
  bool label = false;
  std::string label_text;
  std::string transcript;
};

// 用 Cairo 和 Pango 绘制语音浮层，X11 和 Wayland 浮层共用，两种会话因此不会各自走样；它们只在像素送往何处上不同。绘制器以逻辑像素工作，由调用方把上下文缩放到设备像素。几何、字号和交互跟随 macOS 浮层；颜色仍取解析出的主题（内置主题用 Adwaita 配色，皮肤则用皮肤自己的颜色）。
class WaveOverlayPainter {
 public:
  WaveOverlayPainter() = default;
  WaveOverlayPainter(const WaveOverlayPainter &) = delete;
  WaveOverlayPainter &operator=(const WaveOverlayPainter &) = delete;
  ~WaveOverlayPainter() {
    if (status_font_) pango_font_description_free(status_font_);
    if (transcript_font_) pango_font_description_free(transcript_font_);
    if (context_) g_object_unref(context_);
  }

  // 失败提示胶囊为容纳消息最多能加宽到的长度；更长的消息用「…」截断。
  static constexpr int kMaxFailureWidth = 720;

  WaveOverlayFrame frame(const WaveOverlayModel &model) {
    WaveOverlayFrame frame;
    frame.actions = model.actions_visible();
    frame.label = model.label_visible();
    if (model.failure) {
      // 固定消息占用处理中尺寸胶囊的标签行，并加宽到能放下它，与 macOS 一样。
      frame.layout = WaveOverlayLayout::Processing;
      frame.label_text = model.status;
      auto *layout = text_layout(status_font(), model.status, -1);
      PangoRectangle logical{};
      pango_layout_get_extents(layout, nullptr, &logical);
      g_object_unref(layout);
      const int fitted = static_cast<int>(std::ceil(static_cast<double>(logical.width) / PANGO_SCALE +
                                                    2.0 * kWaveOverlayTranscriptPadding));
      frame.width = std::clamp(fitted, kWaveOverlayProcessingWidth, kMaxFailureWidth);
      frame.height = kWaveOverlayProcessingHeight;
      return frame;
    }
    frame.layout = model.layout();
    const auto size = wave_overlay_size(frame.layout);
    frame.width = size.width;
    frame.height = size.height;
    if (frame.label) frame.label_text = model.label();
    if (frame.layout == WaveOverlayLayout::Transcript)
      frame.transcript = visible_transcript(model, frame.width - 2.0 * kWaveOverlayTranscriptPadding);
    return frame;
  }

  // 把 `frame` 画进 `cairo`，后者的用户空间就是这一帧的逻辑像素。在没有 alpha 通道的浮层上 `translucent` 为 false，面板画成不透明，而不是 macOS 的 0.90。
  void paint(cairo_t *cairo, const WaveOverlayFrame &frame, const WaveOverlayModel &model, bool translucent) {
    const auto colors = palette(model);
    const double w = frame.width, h = frame.height;
    const bool transcript = !frame.transcript.empty();

    cairo_save(cairo);
    cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_rgba(cairo, 0, 0, 0, 0);
    cairo_paint(cairo);
    cairo_restore(cairo);

    // 面板向内缩进半根细线的宽度，描边因此留在浮层之内。
    const double radius = std::min(wave_overlay_corner_radius(h, frame.actions, frame.label, transcript),
                                   std::min(w, h) / 2.0 - 0.5);
    rounded_rectangle(cairo, 0.5, 0.5, w - 1.0, h - 1.0, radius);
    set_color(cairo, colors.surface, translucent ? kWaveOverlayPanelOpacity : 1.0);
    if (colors.border) {
      cairo_fill_preserve(cairo);
      set_color(cairo, colors.border->rgb, colors.border->alpha);
      cairo_set_line_width(cairo, 1.0);
      cairo_stroke(cairo);
    } else {
      cairo_fill(cairo);
    }

    const double content_left = frame.actions ? kWaveOverlayActionContentInset : kWaveOverlayTranscriptPadding;
    const double content_width = w - 2.0 * content_left;
    const double center_y = transcript ? 18.0 : h * 0.5;
    if (frame.label) {
      auto *layout = text_layout(status_font(), frame.label_text, static_cast<int>(content_width * PANGO_SCALE));
      pango_layout_set_alignment(layout, PANGO_ALIGN_CENTER);
      pango_layout_set_ellipsize(layout, PANGO_ELLIPSIZE_END);
      PangoRectangle logical{};
      pango_layout_get_extents(layout, nullptr, &logical);
      cairo_move_to(cairo, content_left, center_y - static_cast<double>(logical.height) / PANGO_SCALE / 2.0);
      set_color(cairo, colors.text, 1.0);
      pango_cairo_show_layout(cairo, layout);
      g_object_unref(layout);
    } else {
      const auto wave = wave_overlay_wave(model.levels, w, h, frame.actions, transcript);
      // 波形条用文字颜色，与 macOS 用前景色填充它们一样。
      set_color(cairo, colors.text, 1.0);
      for (const auto &bar : wave.bars) {
        if (bar.dot) {
          cairo_new_sub_path(cairo);
          cairo_arc(cairo, bar.x, wave.center_y, kWaveOverlayDotRadius, 0, 2 * M_PI);
        } else {
          rounded_rectangle(cairo, bar.x - wave.bar_width * 0.5, wave.center_y - bar.half_height, wave.bar_width,
                            bar.half_height * 2.0, wave.bar_width * 0.5);
        }
      }
      cairo_fill(cairo);
    }

    if (transcript) {
      // 最新的三行，按 macOS 的 22 px 行距排列，裁剪到文字区域内。
      const double left = kWaveOverlayTranscriptPadding;
      const double top = kWaveOverlayTranscriptTop;
      const double text_width = w - 2.0 * kWaveOverlayTranscriptPadding;
      cairo_save(cairo);
      cairo_rectangle(cairo, left, top, text_width, h - top - kWaveOverlayTranscriptBottom);
      cairo_clip(cairo);
      set_color(cairo, colors.text, 1.0);
      auto *layout = transcript_layout(frame.transcript, text_width);
      auto *iter = pango_layout_get_iter(layout);
      int index = 0;
      do {
        PangoRectangle logical{};
        pango_layout_iter_get_line_extents(iter, nullptr, &logical);
        const double line_top = static_cast<double>(logical.y) / PANGO_SCALE;
        const double line_height = static_cast<double>(logical.height) / PANGO_SCALE;
        const double baseline = static_cast<double>(pango_layout_iter_get_baseline(iter)) / PANGO_SCALE - line_top;
        cairo_move_to(cairo, left + static_cast<double>(logical.x) / PANGO_SCALE,
                      top + index * kWaveOverlayTranscriptLineHeight +
                          (kWaveOverlayTranscriptLineHeight - line_height) / 2.0 + baseline);
        pango_cairo_show_layout_line(cairo, pango_layout_iter_get_line_readonly(iter));
        ++index;
      } while (pango_layout_iter_next_line(iter));
      pango_layout_iter_free(iter);
      g_object_unref(layout);
      cairo_restore(cairo);
    }

    if (frame.actions) {
      const double y = wave_overlay_action_center_y(h);
      const double left_x = wave_overlay_cancel_center_x();
      const double right_x = wave_overlay_confirm_center_x(w);
      // 略偏向文字颜色的中性圆，就像 macOS 的灰色介于面板和字形之间。
      set_color(cairo, mix(colors.surface, colors.text, 0.12), 1.0);
      for (const double x : {left_x, right_x}) {
        cairo_new_sub_path(cairo);
        cairo_arc(cairo, x, y, kWaveOverlayActionRadius, 0, 2 * M_PI);
      }
      cairo_fill(cairo);
      set_color(cairo, colors.text, 1.0);
      cairo_set_line_width(cairo, 1.7);
      constexpr double x_half = 3.0;
      cairo_move_to(cairo, left_x - x_half, y - x_half);
      cairo_line_to(cairo, left_x + x_half, y + x_half);
      cairo_move_to(cairo, left_x + x_half, y - x_half);
      cairo_line_to(cairo, left_x - x_half, y + x_half);
      cairo_move_to(cairo, right_x - 3.8, y);
      cairo_line_to(cairo, right_x - 0.8, y + 3.0);
      cairo_line_to(cairo, right_x + 4.2, y - 3.5);
      cairo_stroke(cairo);
    }
  }

  // `text` 以转写字体在 `width` 逻辑像素宽度下折成的行数。
  std::size_t transcript_line_count(const std::string &text, double width) {
    auto *layout = transcript_layout(text, width);
    const auto lines = static_cast<std::size_t>(pango_layout_get_line_count(layout));
    g_object_unref(layout);
    return lines;
  }

 private:
  struct Rgb {
    double r = 0, g = 0, b = 0;
  };
  struct Border {
    Rgb rgb;
    double alpha = 1.0;
  };
  struct Colors {
    Rgb surface, text;
    std::optional<Border> border;
  };

  static Rgb rgb(std::uint32_t value) {
    return {((value >> 16) & 0xff) / 255.0, ((value >> 8) & 0xff) / 255.0, (value & 0xff) / 255.0};
  }
  static Rgb mix(Rgb from, Rgb to, double amount) {
    return {from.r + (to.r - from.r) * amount, from.g + (to.g - from.g) * amount, from.b + (to.b - from.b) * amount};
  }
  static void set_color(cairo_t *cairo, Rgb color, double alpha) {
    cairo_set_source_rgba(cairo, color.r, color.g, color.b, alpha);
  }
  static void rounded_rectangle(cairo_t *cairo, double x, double y, double w, double h, double r) {
    r = std::max(0.0, std::min(r, std::min(w, h) / 2.0));
    cairo_new_sub_path(cairo);
    cairo_arc(cairo, x + w - r, y + r, r, -M_PI / 2.0, 0);
    cairo_arc(cairo, x + w - r, y + h - r, r, 0, M_PI / 2.0);
    cairo_arc(cairo, x + r, y + h - r, r, M_PI / 2.0, M_PI);
    cairo_arc(cairo, x + r, y + r, r, M_PI, 3.0 * M_PI / 2.0);
    cairo_close_path(cairo);
  }

  // 解析出的主题的表面色和文字色，主题画描边时带上描边。没有主题配色时用内置的浅色或深色，并像 macOS 给面板描边那样，用文字颜色画一条淡淡的细线。
  static Colors palette(const WaveOverlayModel &model) {
    Colors colors;
    if (model.palette) {
      colors.surface = rgb(model.palette->surface);
      colors.text = rgb(model.palette->text);
      if (model.palette->border) colors.border = Border{rgb(*model.palette->border), 1.0};
      return colors;
    }
    colors.surface = rgb(model.light_theme ? 0xF5F7FAu : 0x202124u);
    colors.text = rgb(model.light_theme ? 0x202124u : 0xF5F7FAu);
    colors.border = Border{colors.text, model.light_theme ? 0.22 : 0.20};
    return colors;
  }

  // 测量和绘制共用一个 Pango 上下文，并关闭度量微调，无论浮层以什么缩放绘制，排版的折行都相同。
  PangoContext *context() {
    if (!context_) {
      context_ = pango_font_map_create_context(pango_cairo_font_map_get_default());
      auto *options = cairo_font_options_create();
      cairo_font_options_set_hint_metrics(options, CAIRO_HINT_METRICS_OFF);
      cairo_font_options_set_antialias(options, CAIRO_ANTIALIAS_GRAY);
      pango_cairo_context_set_font_options(context_, options);
      cairo_font_options_destroy(options);
    }
    return context_;
  }
  static PangoFontDescription *ui_font(double pixels) {
    auto *font = pango_font_description_from_string("Sans");
    pango_font_description_set_absolute_size(font, pixels * PANGO_SCALE);
    return font;
  }
  const PangoFontDescription *status_font() {
    if (!status_font_) status_font_ = ui_font(kWaveOverlayStatusFontSize);
    return status_font_;
  }
  const PangoFontDescription *transcript_font() {
    if (!transcript_font_) transcript_font_ = ui_font(kWaveOverlayTranscriptFontSize);
    return transcript_font_;
  }
  PangoLayout *text_layout(const PangoFontDescription *font, const std::string &text, int width) {
    auto *layout = pango_layout_new(context());
    pango_layout_set_font_description(layout, font);
    pango_layout_set_width(layout, width);
    pango_layout_set_text(layout, text.c_str(), static_cast<int>(text.size()));
    return layout;
  }
  PangoLayout *transcript_layout(const std::string &text, double width) {
    auto *layout = text_layout(transcript_font(), text, static_cast<int>(width * PANGO_SCALE));
    pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
    return layout;
  }

  // 放得进三行的最新文字，只在光标可以停留的位置（字素边界）切开，emoji 或带附加符号的基字母因此整体保留或整体丢弃。结果有缓存：文字每秒只变几次，而帧每 16 ms 一帧。
  std::string visible_transcript(const WaveOverlayModel &model, double width) {
    if (model.transcript == cached_source_ && model.transcript_truncated == cached_truncated_ &&
        width == cached_width_)
      return cached_visible_;
    const auto &text = model.transcript;
    // `pango_get_log_attrs` 按整个字节长度为每个字符写一项，所以字符数也用同样的方法按首字节数；`g_utf8_strlen` 遇到 NUL 就会停下，数组于是不够长。
    const auto characters = static_cast<glong>(
        std::count_if(text.begin(), text.end(), [](char byte) { return (static_cast<unsigned char>(byte) & 0xc0) != 0x80; }));
    std::vector<PangoLogAttr> attributes(static_cast<std::size_t>(characters) + 1);
    pango_get_log_attrs(text.c_str(), static_cast<int>(text.size()), -1, pango_language_get_default(),
                        attributes.data(), static_cast<int>(attributes.size()));
    std::vector<std::size_t> cuts{0};
    const char *position = text.c_str();
    for (glong index = 1; index <= characters; ++index) {
      position = g_utf8_next_char(position);
      if (attributes[static_cast<std::size_t>(index)].is_cursor_position || index == characters)
        cuts.push_back(static_cast<std::size_t>(position - text.c_str()));
    }
    cached_visible_ = wave_overlay_visible_transcript(
        text, cuts, model.transcript_truncated, kWaveOverlayTranscriptLines,
        [this, width](const std::string &candidate) { return transcript_line_count(candidate, width); });
    cached_source_ = text;
    cached_truncated_ = model.transcript_truncated;
    cached_width_ = width;
    return cached_visible_;
  }

  PangoContext *context_ = nullptr;
  PangoFontDescription *status_font_ = nullptr;
  PangoFontDescription *transcript_font_ = nullptr;
  std::string cached_source_;
  bool cached_truncated_ = false;
  double cached_width_ = -1;
  std::string cached_visible_;
};

}  // namespace msime::linux_host
