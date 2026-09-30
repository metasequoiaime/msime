#pragma once

#include <algorithm>
#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>

#include "../candidates/CandidatePalette.h"

namespace msime::linux_host {

struct WaveOverlayModel {
  enum class Action { Cancel, Confirm };
  enum class CompactStatus { None, Recognizing, Processing };
  static constexpr std::size_t kLevelCount = 12;
  std::array<float, kLevelCount> levels{};
  bool listening = false;
  bool show_transcript = true;
  bool actions_visible = false;
  bool light_theme = false;
  // The resolved theme's colours for the surfaces MSIME draws itself (X11, Wayland); unset keeps their built-in light or dark colours. The IBus fallback shows only text and ignores it.
  std::optional<FloatingSurfaceColors> palette;
  std::string transcript;
  std::string status;
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
      if (!sequence_valid) {
        ++offset;
        continue;
      }
      valid.append(value, offset, width);
      offset += width;
    }

    constexpr std::size_t kVisibleCharacters = 160;
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
  }
  Action pressed_action = Action::Confirm;
  CompactStatus compact_status = CompactStatus::None;

  void set_input_level(float value) {
    value = std::clamp(value, 0.0f, 1.0f);
    for (std::size_t i = 0; i < kLevelCount; ++i) {
      const float center = 1.0f - static_cast<float>(i) / kLevelCount;
      levels[i] = std::clamp(value * (0.55f + center * 0.45f), 0.0f, 1.0f);
    }
  }
};

}  // namespace msime::linux_host
