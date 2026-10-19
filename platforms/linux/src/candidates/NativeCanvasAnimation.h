#pragma once

#include "FcitxCanvasPanel.h"
#include <vector>

namespace msime::linux_host {

using NativeCanvasImage = std::shared_ptr<cairo_surface_t>;
struct NativeCanvasAnimation {
  std::vector<NativeCanvasImage> frames;
  std::vector<std::uint32_t> delays_ms;
  std::uint32_t play_count = 1;
};
struct NativeCanvasSample {
  std::size_t index = 0;
  // 相对当前采样时间的下一次变化；单帧和有限末帧没有下一次。
  std::optional<std::uint64_t> next_ms;
};
inline NativeCanvasSample sample_native_canvas_animation(const NativeCanvasAnimation &animation,
                                                         std::uint64_t elapsed_ms) {
  if (animation.frames.size() < 2 || animation.frames.size() != animation.delays_ms.size()) return {};
  std::uint64_t cycle = 0;
  for (const auto delay : animation.delays_ms) cycle += std::max(1u, delay);
  if (animation.play_count && elapsed_ms / cycle >= animation.play_count) return {animation.frames.size() - 1, {}};
  auto phase = elapsed_ms % cycle;
  for (std::size_t at = 0; at < animation.frames.size(); ++at) {
    const auto delay = std::max(1u, animation.delays_ms[at]);
    if (phase < delay) {
      // 最后一轮的末帧已经是最终画面，不必为“播放结束”再唤醒。
      if (animation.play_count && elapsed_ms / cycle + 1 == animation.play_count && at + 1 == animation.frames.size())
        return {at, {}};
      return {at, delay - phase};
    }
    phase -= delay;
  }
  return {};
}

// 包内不同路径累计完整画布像素；同一资源的共享缓存由加载器持有。
inline std::shared_ptr<const NativeCanvasAnimation> decode_native_canvas_animation(
    const std::string &bytes, std::uint64_t &remaining_pixels) {
  struct Receive {
    NativeCanvasAnimation animation;
    std::uint64_t pixels = 0, budget = 0;
  } received;
  received.budget = remaining_pixels;
  const auto ok = msime_client_skin_animation_decode(
      reinterpret_cast<const std::uint8_t *>(bytes.data()), bytes.size(),
      [](void *context, const std::uint8_t *rgba, std::size_t length, std::uint32_t width, std::uint32_t height,
         std::uint32_t index, std::uint32_t count, std::uint32_t delay, std::uint32_t plays) -> std::int32_t {
        // C++ 分配失败不得跨越 Rust 的 C ABI 展开异常。
        try {
          auto &value = *static_cast<Receive *>(context);
          const auto pixels = static_cast<std::uint64_t>(width) * height;
          if (count == 0 || count > 120 || index != value.animation.frames.size() || index >= count ||
              pixels > value.budget - value.pixels) return 0;
          NativeCanvasImage frame(copy_fcitx_canvas_rgba(rgba, length, width, height), cairo_surface_destroy);
          if (!frame) return 0;
          value.animation.frames.push_back(std::move(frame));
          value.animation.delays_ms.push_back(delay);
          value.animation.play_count = plays;
          value.pixels += pixels;
          return 1;
        } catch (...) { return 0; }
      }, &received);
  if (ok != 1 || received.animation.frames.empty()) return {};
  remaining_pixels -= received.pixels;
  return std::make_shared<const NativeCanvasAnimation>(std::move(received.animation));
}

} // msime::linux_host 命名空间
