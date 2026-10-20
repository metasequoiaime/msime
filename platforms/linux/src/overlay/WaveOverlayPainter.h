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

// What one presentation of the overlay shows, in logical pixels: the layout and its size, whether the actions and a status label are drawn, the label's text and the part of the transcript that fits.
struct WaveOverlayFrame {
  WaveOverlayLayout layout = WaveOverlayLayout::Compact;
  int width = 0;
  int height = 0;
  bool actions = false;
  bool label = false;
  std::string label_text;
  std::string transcript;
};

// Drawing the voice overlay with Cairo and Pango, shared by the X11 and Wayland surfaces so the two session types cannot drift apart; they differ only in where the pixels go. The painter works in logical pixels and the caller scales the context to the device. It follows the macOS overlay's geometry, type sizes and interaction; the colours stay the resolved theme's (the Adwaita palette for the built-in theme, a skin's own colours otherwise).
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

  // The longest a failure pill grows to fit its message; a longer message is cut with "…".
  static constexpr int kMaxFailureWidth = 720;

  WaveOverlayFrame frame(const WaveOverlayModel &model) {
    WaveOverlayFrame frame;
    frame.actions = model.actions_visible();
    frame.label = model.label_visible();
    if (model.failure) {
      // The fixed message takes the label line of a processing-sized pill, widened to fit it, as macOS does.
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

  // Paint `frame` into `cairo`, whose user space is the frame's logical pixels. `translucent` is false on a surface without an alpha channel, where the panel is drawn opaque instead of at the macOS 0.90.
  void paint(cairo_t *cairo, const WaveOverlayFrame &frame, const WaveOverlayModel &model, bool translucent) {
    const auto colors = palette(model);
    const double w = frame.width, h = frame.height;
    const bool transcript = !frame.transcript.empty();

    cairo_save(cairo);
    cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_rgba(cairo, 0, 0, 0, 0);
    cairo_paint(cairo);
    cairo_restore(cairo);

    // The panel, inset by half the hairline so the outline stays inside the surface.
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
      // The bars take the text colour, as macOS fills them with its foreground.
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
      // The newest three lines at the macOS 22 px pitch, clipped to the text area.
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
      // A neutral circle a little toward the text colour, as the macOS grey sits between its panel and its glyphs.
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

  // Lines `text` wraps into in the transcript font at `width` logical pixels.
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

  // The resolved theme's surface and text, with its outline when it draws one. Without a theme palette the built-in light or dark colours apply, outlined by a faint hairline of the text colour as macOS outlines its panel.
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

  // One Pango context for measuring and drawing, with hinted metrics off, so a layout wraps the same way whatever scale the surface draws it at.
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

  // The newest text that fits in three lines, cut only where a cursor may stand (a grapheme boundary), so an emoji or a base letter with its marks is kept or dropped whole. Cached: the text changes a few times a second, the frames come every 16 ms.
  std::string visible_transcript(const WaveOverlayModel &model, double width) {
    if (model.transcript == cached_source_ && model.transcript_truncated == cached_truncated_ &&
        width == cached_width_)
      return cached_visible_;
    const auto &text = model.transcript;
    // pango_get_log_attrs writes one entry per character over the whole byte length, so the characters are counted the same way, by their lead bytes; g_utf8_strlen would stop at a NUL and leave the array short.
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
