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
  // 画出来的波形条；`advance()` 每次一帧地把它们推向 `input_level`。
  std::array<float, kLevelCount> levels{};
  // 最新的麦克风音量，0...1。
  float input_level = 0.0f;
  bool listening = false;
  bool show_transcript = true;
  // 失败提示：`status` 存放它的固定消息，单独画在加宽到能放下它的胶囊里。
  bool failure = false;
  bool light_theme = false;
  // The resolved theme's colours for the surfaces MSIME draws itself (X11, Wayland); unset keeps their built-in light or dark colours. The IBus fallback shows only text and ignores it.
  std::optional<FloatingSurfaceColors> palette;
  std::string transcript;
  // 为了让转写文字不超过 `kWaveOverlayTranscriptWindow`，更早的文字已被丢弃，所以它的每一次预览都以「…」开头。
  bool transcript_truncated = false;
  std::string status;
  // 按住录音时按了空格锁定：松开快捷键后录音继续。
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
      // 换行和制表符以外的 C0 控制字符也丢弃：NUL 会让之后读取它的每个 C 字符串接口（GLib、IBus、Pango 的校验）提前结束文本，而字节长度仍然算到它后面。
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

  // 波形要趋向的音量；波形条本身只在 `advance()` 里改变。
  void set_input_level(float value) {
    input_level = std::isfinite(value) ? std::clamp(value, 0.0f, 1.0f) : 0.0f;
  }
  // 单调时间 `seconds` 处的一帧动画。
  void advance(double seconds) {
    wave_overlay_advance_levels(levels, input_level, listening, seconds);
  }

  // 录音结束后正在识别或润色。
  bool processing() const { return compact_status != CompactStatus::None; }
  // macOS 的规则：锁定的录音正在收音，或者正在识别或润色时，显示圆形的取消和确认按钮；普通按住录音和失败提示时都不显示。
  bool actions_visible() const {
    return !failure && (processing() || (locked && listening));
  }
  // 识别或润色进行中以及失败时，状态标签代替波形；录音时只显示波形。
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
  // 只要画了波形就播放动画，也就是没有标签代替它的时候。
  bool animates() const { return !label_visible(); }
};

// `width` x `height` 浮层上的逻辑点 (x, y) 落在哪个操作按钮上（如果有）；只有两个圆形操作按钮接收指针。
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
