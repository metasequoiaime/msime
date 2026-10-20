#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <string>
#include <vector>

namespace msime::linux_host {

// The voice overlay's geometry, timing, wave motion and wording, in logical pixels at a scale of 1. The numbers and rules are those of the macOS overlay (platforms/macos/src/voice/VoiceWaveOverlay.mm), which follows the Windows voice bar; a change there belongs here too. Pure so the X11 and Wayland surfaces, both hosts and the tests share one copy.

// The four presentations in the macOS precedence order: the round actions win over a status label, which wins over the transcript.
enum class WaveOverlayLayout { Compact, Processing, Action, Transcript };

struct WaveOverlaySize {
  int width = 0;
  int height = 0;
};

inline constexpr std::size_t kWaveOverlayBarCount = 12;
// The wave animates on this cadence while it is on screen.
inline constexpr int kWaveOverlayFrameMilliseconds = 16;
inline constexpr int kWaveOverlayProcessingWidth = 112;
inline constexpr int kWaveOverlayProcessingHeight = 40;
inline constexpr double kWaveOverlayActionCenterInset = 18;
inline constexpr double kWaveOverlayActionRadius = 12;
inline constexpr double kWaveOverlayActionContentInset = 35;
inline constexpr double kWaveOverlayTranscriptPadding = 14;
inline constexpr double kWaveOverlayTranscriptTop = 33;
inline constexpr double kWaveOverlayTranscriptBottom = 8;
inline constexpr double kWaveOverlayTranscriptLineHeight = 22;
inline constexpr double kWaveOverlayTranscriptFontSize = 15;
inline constexpr double kWaveOverlayStatusFontSize = 14;
inline constexpr std::size_t kWaveOverlayTranscriptLines = 3;
inline constexpr double kWaveOverlayPanelOpacity = 0.90;
inline constexpr double kWaveOverlayDotRadius = 1.15;
inline constexpr double kWaveOverlayMaxHalfHeight = 14;
// The bar sits this far above the bottom of the work area, as on macOS and Windows.
inline constexpr int kWaveOverlayWorkAreaGap = 10;
// A failure stays on screen this long unless Esc, a focus change or the next recording takes it down first.
inline constexpr unsigned kWaveOverlayFailureMilliseconds = 6000;
// The transcript keeps at most this many code points; three lines of 15 px text in 392 px hold far fewer, so the newest-text search never lays out more.
inline constexpr std::size_t kWaveOverlayTranscriptWindow = 2048;

// The status wording both Linux hosts use, in their menus, the overlay and the auxiliary-text fallback. The recording status is spoken only (macOS shows a bare wave); the two closing states are the macOS labels.
inline constexpr const char *kVoiceStatusRecording = "正在录音…";
inline constexpr const char *kVoiceStatusRecognizing = "识别中...";
inline constexpr const char *kVoiceStatusProcessing = "处理中...";
// Only the auxiliary-text fallback says this; the drawn overlay shows the round actions instead, which is all macOS shows for a locked recording.
inline constexpr const char *kVoiceStatusLockedHint =
    "录音已锁定 · 可松开快捷键 · 再按快捷键或点击语音菜单结束 · Esc 取消";

inline constexpr WaveOverlayLayout wave_overlay_layout(bool actions_visible, bool has_label,
                                                       bool has_transcript) {
  if (actions_visible) return WaveOverlayLayout::Action;
  if (has_label) return WaveOverlayLayout::Processing;
  return has_transcript ? WaveOverlayLayout::Transcript : WaveOverlayLayout::Compact;
}

// Compact 78x32, processing 112x40, action 142x40, transcript 420x112.
inline constexpr WaveOverlaySize wave_overlay_size(WaveOverlayLayout layout) {
  switch (layout) {
    case WaveOverlayLayout::Processing:
      return {kWaveOverlayProcessingWidth, kWaveOverlayProcessingHeight};
    case WaveOverlayLayout::Action:
      return {142, 40};
    case WaveOverlayLayout::Transcript:
      return {420, 112};
    case WaveOverlayLayout::Compact:
      break;
  }
  return {78, 32};
}

// A pill while it carries the actions or a lone label, a 16 px card around a transcript, otherwise the compact 10 px rounding.
inline constexpr double wave_overlay_corner_radius(double height, bool actions, bool label,
                                                   bool transcript) {
  return actions || (label && !transcript) ? height * 0.5 : (transcript ? 16.0 : 10.0);
}

// Advance the bar levels by one frame with the macOS multi-harmonic motion. `seconds` is a monotonic clock and `input_level` the microphone level, clamped to 0...1. Levels stay within 0...1 and settle back to 0 (dots) once `listening` is false or the input is silent.
inline void wave_overlay_advance_levels(std::array<float, kWaveOverlayBarCount> &levels,
                                        float input_level, bool listening, double seconds) {
  const float level = listening && input_level > 0.0f ? std::min(1.0f, input_level) : 0.0f;
  const float center = 0.5f * static_cast<float>(kWaveOverlayBarCount - 1);
  for (std::size_t i = 0; i < kWaveOverlayBarCount; ++i) {
    // Each bar has its own amplitude, frequency and phase, so the movement looks natural yet stays deterministic.
    const float fi = static_cast<float>(i);
    const float amplitude = 0.55f + 0.45f * std::fabs(std::sin(0.73f * fi + 0.19f));
    const float p = 0.41f * fi + 0.37f * std::sin(0.29f * fi + 1.11f);
    const float f = 4.2f + std::fmod(1.7f * fi, 3.6f);
    const float dist = std::fabs(fi - center) / std::max(1.0f, center);
    const float center_boost = 1.0f + 0.75f * (1.0f - dist);
    // The clock stays in double: a float of the uptime loses the 16 ms step after a few days.
    const float n1 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * f + p));
    const float n2 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (0.57f * f) + 1.7f * p + 0.9f));
    const float n3 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (1.23f * f) + 0.6f * p + 2.1f));
    const float irregular = std::min(1.0f, 0.60f * n1 + 0.30f * n2 + 0.10f * n3);
    // When the signal is weak the floor drops too, so the bars settle back to dots smoothly.
    const float floor = level * (0.06f + 0.12f * n3);
    float target = level * amplitude * center_boost * std::max(floor, irregular);
    // A transient punch on the rising edge makes the start feel more energetic.
    if (target > levels[i]) target = std::min(1.0f, target + 0.22f * level * (1.0f - levels[i]));
    const float smooth = target > levels[i] ? 0.52f : (listening ? 0.07f : 0.045f);
    levels[i] = levels[i] * (1.0f - smooth) + target * smooth;
  }
}

// One bar of the wave in logical pixels: its centre x and, when it is tall enough to be a capsule, its half height; below that it is a dot of kWaveOverlayDotRadius.
struct WaveOverlayBar {
  double x = 0;
  double half_height = 0;
  bool dot = true;
};

struct WaveOverlayWave {
  double center_y = 0;
  double bar_width = 0;
  std::array<WaveOverlayBar, kWaveOverlayBarCount> bars{};
};

// The wave's geometry for a panel of `width` x `height`, as the macOS view lays it out: across the whole compact pill, 78 px between the actions, or 78 px above a transcript, on a step tightened by a quarter, with capsules of at least a dot's width that turn into dots below 0.06.
inline WaveOverlayWave wave_overlay_wave(const std::array<float, kWaveOverlayBarCount> &levels,
                                         double width, double height, bool actions,
                                         bool transcript) {
  WaveOverlayWave wave;
  const double content_left = actions ? kWaveOverlayActionContentInset : 0.0;
  const double content_width = (actions ? width - kWaveOverlayActionContentInset : width) - content_left;
  wave.center_y = transcript ? 18.0 : height * 0.5;
  const double wave_width = transcript ? std::min(78.0, width) : (actions ? std::min(78.0, content_width) : width);
  const double wave_left = actions ? content_left + (content_width - wave_width) * 0.5
                                   : (transcript ? (width - wave_width) * 0.5 : 0.0);
  const double side_margin = kWaveOverlayDotRadius + 0.25;
  const double track_width = std::max(1.0, wave_width - 2.0 * side_margin);
  const double base_step = track_width / static_cast<double>(kWaveOverlayBarCount - 1);
  const double step = base_step * 0.75;
  const double start_x = wave_left + (wave_width - step * static_cast<double>(kWaveOverlayBarCount - 1)) * 0.5;
  wave.bar_width = std::max(kWaveOverlayDotRadius * 2.0, base_step * 0.26);
  for (std::size_t i = 0; i < kWaveOverlayBarCount; ++i) {
    auto &bar = wave.bars[i];
    bar.x = start_x + static_cast<double>(i) * step;
    const double level = transcript ? std::min(1.0, static_cast<double>(levels[i]) * 1.35) : levels[i];
    bar.dot = level < 0.06;
    bar.half_height = bar.dot ? kWaveOverlayDotRadius
                              : kWaveOverlayDotRadius + level * (kWaveOverlayMaxHalfHeight - kWaveOverlayDotRadius);
  }
  return wave;
}

// The cancel (left) and confirm (right) action centres for a panel of `width` x `height`; both have radius kWaveOverlayActionRadius.
inline double wave_overlay_action_center_y(double height) { return height * 0.5; }
inline double wave_overlay_cancel_center_x() { return kWaveOverlayActionCenterInset; }
inline double wave_overlay_confirm_center_x(double width) { return width - kWaveOverlayActionCenterInset; }

// Keep the newest text that fits in `max_lines`: when the whole transcript does not fit, or older text was already dropped (`truncated`), the oldest text goes behind a leading "…". `cuts` are the byte offsets where the text may be cut, ascending, starting at 0 and ending at text.size(), so a composed character is kept or dropped whole; `line_count` measures a candidate string.
template <typename LineCount>
std::string wave_overlay_visible_transcript(const std::string &text, const std::vector<std::size_t> &cuts,
                                            bool truncated, std::size_t max_lines, LineCount line_count) {
  if (text.empty()) return {};
  if (!truncated && line_count(text) <= max_lines) return text;
  if (cuts.empty()) return "…";
  const auto tail = [&](std::size_t index) { return "…" + text.substr(cuts[index]); };
  std::size_t low = 0;
  std::size_t high = cuts.size() - 1;
  while (low < high) {
    const std::size_t middle = low + (high - low) / 2;
    if (line_count(tail(middle)) <= max_lines)
      high = middle;
    else
      low = middle + 1;
  }
  return tail(low);
}

}  // namespace msime::linux_host
