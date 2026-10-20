#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>

#include "../candidates/CandidatePalette.h"
#include "WaveOverlayLayout.h"

namespace msime::linux_host {

struct WaveOverlayModel {
  enum class Action { Cancel, Confirm };
  enum class CompactStatus { None, Recognizing, Processing };
  static constexpr std::size_t kLevelCount = kWaveOverlayBarCount;
  // The bars as drawn; advance() moves them toward input_level one frame at a time.
  std::array<float, kLevelCount> levels{};
  // The latest microphone level, 0...1.
  float input_level = 0.0f;
  bool listening = false;
  bool show_transcript = true;
  // A failure notice: `status` holds its fixed message, drawn alone in a pill widened to fit it.
  bool failure = false;
  bool light_theme = false;
  // The resolved theme's colours for the surfaces MSIME draws itself (X11, Wayland); unset keeps their built-in light or dark colours. The IBus fallback shows only text and ignores it.
  std::optional<FloatingSurfaceColors> palette;
  std::string transcript;
  // Older text was dropped to keep the transcript within kWaveOverlayTranscriptWindow, so every preview of it starts with "…".
  bool transcript_truncated = false;
  std::string status;
  // Space locked a hold-to-record recording: it goes on after the shortcut is released.
  bool locked = false;

  void reset() {
    const auto theme = light_theme;
    auto colors = palette;
    *this = WaveOverlayModel{};
    light_theme = theme;
    palette = colors;
  }

  void set_transcript(std::string value) {
    // Provider responses are external input. Keep the model's invariant that
    // transcript is valid UTF-8 before GLib receives it for display.
    std::string valid;
    valid.reserve(value.size());
    for (std::size_t offset = 0; offset < value.size();) {
      const auto first = static_cast<std::uint8_t>(value[offset]);
      std::size_t width = 0;
      if (first < 0x80)
        width = 1;
      else if (first >= 0xc2 && first <= 0xdf)
        width = 2;
      else if (first >= 0xe0 && first <= 0xef)
        width = 3;
      else if (first >= 0xf0 && first <= 0xf4)
        width = 4;
      bool sequence_valid = width != 0 && offset + width <= value.size();
      if (sequence_valid) {
        const auto second =
            width > 1 ? static_cast<std::uint8_t>(value[offset + 1]) : 0;
        if (width > 1 && (second & 0xc0) != 0x80)
          sequence_valid = false;
        if (width == 3 && ((first == 0xe0 && second < 0xa0) ||
                           (first == 0xed && second > 0x9f)))
          sequence_valid = false;
        if (width == 4 && ((first == 0xf0 && second < 0x90) ||
                           (first == 0xf4 && second > 0x8f)))
          sequence_valid = false;
        for (std::size_t index = 2; sequence_valid && index < width; ++index)
          if ((static_cast<std::uint8_t>(value[offset + index]) & 0xc0) != 0x80)
            sequence_valid = false;
      }
      // C0 controls other than a line break or a tab are dropped too: a NUL would end the text early for every C string API that later reads it (GLib, IBus, Pango's validation) while the byte length still counts past it.
      if (sequence_valid && width == 1 && first < 0x20 && first != '\n' && first != '\t')
        sequence_valid = false;
      if (!sequence_valid) {
        ++offset;
        continue;
      }
      valid.append(value, offset, width);
      offset += width;
    }

    constexpr std::size_t kVisibleCharacters = kWaveOverlayTranscriptWindow;
    std::size_t count = 0;
    for (std::size_t offset = 0; offset < valid.size();) {
      ++count;
      const auto first = static_cast<std::uint8_t>(valid[offset]);
      offset += first < 0x80 ? 1 : first < 0xe0 ? 2 : first < 0xf0 ? 3 : 4;
    }
    std::size_t first_visible = 0;
    for (std::size_t drop = count > kVisibleCharacters
                                ? count - kVisibleCharacters
                                : 0;
         drop > 0; --drop) {
      const auto first = static_cast<std::uint8_t>(valid[first_visible]);
      first_visible +=
          first < 0x80 ? 1 : first < 0xe0 ? 2 : first < 0xf0 ? 3 : 4;
    }
    transcript = valid.substr(first_visible);
    transcript_truncated = first_visible > 0;
  }
  Action pressed_action = Action::Confirm;
  CompactStatus compact_status = CompactStatus::None;

  // The level the wave moves toward; the bars themselves change only in advance().
  void set_input_level(float value) {
    input_level = std::isfinite(value) ? std::clamp(value, 0.0f, 1.0f) : 0.0f;
  }
  // One animation frame at monotonic time `seconds`.
  void advance(double seconds) {
    wave_overlay_advance_levels(levels, input_level, listening, seconds);
  }

  // Recognition or polishing is pending after the recording ended.
  bool processing() const { return compact_status != CompactStatus::None; }
  // The macOS rule: the round cancel and confirm actions appear while a locked recording is listening or while recognition or polishing is pending, never during a plain hold and never on a failure.
  bool actions_visible() const {
    return !failure && (processing() || (locked && listening));
  }
  // A status label replaces the wave while recognition or polishing is pending and on a failure; a recording shows the bare wave.
  bool label_visible() const { return failure || processing(); }
  std::string label() const {
    if (failure) return status;
    return compact_status == CompactStatus::Processing ? kVoiceStatusProcessing
                                                       : kVoiceStatusRecognizing;
  }
  bool has_transcript() const { return show_transcript && !failure && !transcript.empty(); }
  WaveOverlayLayout layout() const {
    return wave_overlay_layout(actions_visible(), label_visible(), has_transcript());
  }
  // The wave animates whenever it is drawn, that is whenever no label stands in its place.
  bool animates() const { return !label_visible(); }
};

// Which action, if any, the logical point (x, y) of a `width` x `height` overlay lands on; only the two round actions take the pointer.
inline bool wave_overlay_hit_action(const WaveOverlayModel &model, double x, double y, double width,
                                    double height, WaveOverlayModel::Action &action) {
  if (!model.actions_visible()) return false;
  const double center_y = wave_overlay_action_center_y(height);
  const auto inside = [x, y, center_y](double center_x) {
    const double dx = x - center_x;
    const double dy = y - center_y;
    return dx * dx + dy * dy <= kWaveOverlayActionRadius * kWaveOverlayActionRadius;
  };
  if (inside(wave_overlay_cancel_center_x())) {
    action = WaveOverlayModel::Action::Cancel;
    return true;
  }
  if (inside(wave_overlay_confirm_center_x(width))) {
    action = WaveOverlayModel::Action::Confirm;
    return true;
  }
  return false;
}

}  // namespace msime::linux_host
