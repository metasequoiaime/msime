#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <string>
#include <vector>

namespace msime::linux_host {

// 语音浮层的几何尺寸、时序、波形运动和文案，单位是缩放为 1 时的逻辑像素。数值和规则取自 macOS 浮层（`platforms/macos/src/voice/VoiceWaveOverlay.mm`），它又沿用 Windows 的语音条；那里改了这里也要跟着改。写成纯函数，X11 和 Wayland 浮层、两个宿主和测试共用同一份。

// 四种呈现，按 macOS 的优先顺序：圆形操作按钮优先于状态标签，状态标签优先于转写文字。
enum class WaveOverlayLayout { Compact, Processing, Action, Transcript };

struct WaveOverlaySize {
  int width = 0;
  int height = 0;
};

inline constexpr std::size_t kWaveOverlayBarCount = 12;
// 波形显示在屏幕上时按这个节奏播放动画。
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
// 浮条离工作区底边的距离，与 macOS 和 Windows 相同。
inline constexpr int kWaveOverlayWorkAreaGap = 10;
// 失败提示在屏幕上停留的时长，除非 Esc、焦点变化或下一次录音先把它收起。
inline constexpr unsigned kWaveOverlayFailureMilliseconds = 6000;
// 转写文字最多保留这么多个码位；392 px 宽、15 px 字号的三行远远放不下这么多，所以查找最新文字时排版的量不会超过它。
inline constexpr std::size_t kWaveOverlayTranscriptWindow = 2048;

// 两个 Linux 宿主在菜单、浮层和辅助文本回退里共用的状态文案。录音状态只用于朗读（macOS 只显示波形）；收尾的两个状态用 macOS 的标签。
inline constexpr const char *kVoiceStatusRecording = "正在录音…";
inline constexpr const char *kVoiceStatusRecognizing = "识别中...";
inline constexpr const char *kVoiceStatusProcessing = "处理中...";
// 只有辅助文本回退显示这句话；画出来的浮层改为显示圆形操作按钮，macOS 对锁定的录音也只显示这些。
inline constexpr const char *kVoiceStatusLockedHint =
    "录音已锁定 · 可松开快捷键 · 再按快捷键或点击语音菜单结束 · Esc 取消";

inline constexpr WaveOverlayLayout wave_overlay_layout(bool actions_visible, bool has_label,
                                                       bool has_transcript) {
  if (actions_visible) return WaveOverlayLayout::Action;
  if (has_label) return WaveOverlayLayout::Processing;
  return has_transcript ? WaveOverlayLayout::Transcript : WaveOverlayLayout::Compact;
}

// 紧凑 78x32，处理中 112x40，操作 142x40，转写 420x112。
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

// 带操作按钮或只有一个标签时是胶囊形，包着转写文字时是 16 px 圆角的卡片，其他情况是紧凑的 10 px 圆角。
inline constexpr double wave_overlay_corner_radius(double height, bool actions, bool label,
                                                   bool transcript) {
  return actions || (label && !transcript) ? height * 0.5 : (transcript ? 16.0 : 10.0);
}

// 按 macOS 的多谐波运动把波形条推进一帧。`seconds` 是单调时钟，`input_level` 是麦克风音量，限制在 0...1。各条高度保持在 0...1 之内，`listening` 为 false 或输入静音后回落到 0（圆点）。
inline void wave_overlay_advance_levels(std::array<float, kWaveOverlayBarCount> &levels,
                                        float input_level, bool listening, double seconds) {
  const float level = listening && input_level > 0.0f ? std::min(1.0f, input_level) : 0.0f;
  const float center = 0.5f * static_cast<float>(kWaveOverlayBarCount - 1);
  for (std::size_t i = 0; i < kWaveOverlayBarCount; ++i) {
    // 每根条有自己的振幅、频率和相位，运动看起来自然，结果仍然确定。
    const float fi = static_cast<float>(i);
    const float amplitude = 0.55f + 0.45f * std::fabs(std::sin(0.73f * fi + 0.19f));
    const float p = 0.41f * fi + 0.37f * std::sin(0.29f * fi + 1.11f);
    const float f = 4.2f + std::fmod(1.7f * fi, 3.6f);
    const float dist = std::fabs(fi - center) / std::max(1.0f, center);
    const float center_boost = 1.0f + 0.75f * (1.0f - dist);
    // 时钟保持 double：开机时长用 float 表示，几天后就分辨不出 16 ms 的步长。
    const float n1 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * f + p));
    const float n2 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (0.57f * f) + 1.7f * p + 0.9f));
    const float n3 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (1.23f * f) + 0.6f * p + 2.1f));
    const float irregular = std::min(1.0f, 0.60f * n1 + 0.30f * n2 + 0.10f * n3);
    // 信号弱时下限也随之降低，波形条平滑地回落成圆点。
    const float floor = level * (0.06f + 0.12f * n3);
    float target = level * amplitude * center_boost * std::max(floor, irregular);
    // 上升沿上的瞬时冲击让起音更有力。
    if (target > levels[i]) target = std::min(1.0f, target + 0.22f * level * (1.0f - levels[i]));
    const float smooth = target > levels[i] ? 0.52f : (listening ? 0.07f : 0.045f);
    levels[i] = levels[i] * (1.0f - smooth) + target * smooth;
  }
}

// 波形中的一根条，单位是逻辑像素：中心 x，以及高到足以画成胶囊时的半高；不够高时是半径为 `kWaveOverlayDotRadius` 的圆点。
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

// `width` x `height` 面板上波形的几何布局，与 macOS 视图的排法相同：铺满整个紧凑胶囊、在两个操作按钮之间占 78 px，或在转写文字上方占 78 px；间距收紧四分之一，胶囊至少有一个圆点宽，低于 0.06 时变成圆点。
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

// `width` x `height` 面板上取消（左）和确认（右）操作按钮的圆心；半径都是 `kWaveOverlayActionRadius`。
inline double wave_overlay_action_center_y(double height) { return height * 0.5; }
inline double wave_overlay_cancel_center_x() { return kWaveOverlayActionCenterInset; }
inline double wave_overlay_confirm_center_x(double width) { return width - kWaveOverlayActionCenterInset; }

// 保留放得进 `max_lines` 行的最新文字：整段转写放不下，或更早的文字已经丢弃（`truncated`）时，最旧的部分换成开头的「…」。`cuts` 是允许切开的字节偏移，升序，从 0 开始到 `text.size()` 结束，组合字符因此整体保留或整体丢弃；`line_count` 测量一个候选字符串占几行。
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
